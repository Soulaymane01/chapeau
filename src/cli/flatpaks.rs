use crate::backends::flatpak::FlatpakCliBackend;
use crate::backends::flatpak_backend::FlatpakBackendTrait;
use crate::errors::Result;

pub fn run(_db: &crate::storage::Database) -> Result<()> {
    let backend = FlatpakCliBackend::new();

    if !backend.is_available() {
        println!("Flatpak is not installed on this system.");
        return Ok(());
    }

    let snapshot = backend.discover_snapshot()?;

    println!(
        "Flatpak packages: {} total ({} apps, {} runtimes)",
        snapshot.total_count(),
        snapshot.apps.len(),
        snapshot.runtimes.len()
    );
    println!("  remotes: {}", snapshot.remotes.len());

    // Group apps by origin.
    let mut origins: Vec<&str> = snapshot.apps.iter().map(|a| a.origin.as_str()).collect();
    origins.sort();
    origins.dedup();
    if !origins.is_empty() {
        println!("  origins: {}", origins.join(", "));
    }

    if !snapshot.apps.is_empty() {
        println!();
        println!("Applications ({}):", snapshot.apps.len());
        for app in &snapshot.apps {
            let runtime_suffix = app
                .runtime
                .as_ref()
                .map(|r| format!(" [{}]", r))
                .unwrap_or_default();
            println!(
                "  {:<50} {:<12} {:<10} {}{}",
                app.id, app.version, app.branch, app.origin, runtime_suffix
            );
        }
    }

    if !snapshot.runtimes.is_empty() {
        println!();
        println!("Runtimes ({}):", snapshot.runtimes.len());
        for rt in &snapshot.runtimes {
            println!("  {:<50} {:<12} {}", rt.id, rt.version, rt.branch);
        }
    }

    if !snapshot.remotes.is_empty() {
        println!();
        println!("Remotes ({}):", snapshot.remotes.len());
        for remote in &snapshot.remotes {
            println!("  {:<20} {:<40} {}", remote.name, remote.url, remote.title);
        }
    }

    Ok(())
}
