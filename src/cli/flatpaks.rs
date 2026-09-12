use crate::backends::flatpak::FlatpakCliBackend;
use crate::backends::flatpak_backend::FlatpakBackendTrait;
use crate::core::ResourceType;
use crate::errors::Result;
use crate::storage::Database;
use crate::storage::{observations, resources};

pub fn run(db: &Database, all: bool) -> Result<()> {
    if all {
        run_all(db)
    } else {
        run_roots_only(db)
    }
}

/// Show only root (intentional) Flatpak applications.
fn run_roots_only(db: &Database) -> Result<()> {
    let root_flatpaks = resources::list_roots_by_type(db.conn(), ResourceType::Flatpak)?;

    if root_flatpaks.is_empty() {
        println!("No intentional Flatpak applications (roots) found.");
        println!();
        println!("Run 'chapeau scan' to detect Flatpak apps as candidate roots.");
        println!("Or add a root manually: chapeau root add <flatpak-id>");
        return Ok(());
    }

    println!("Intentional Flatpak ({} roots):", root_flatpaks.len());
    println!();

    let missing_ids: std::collections::HashSet<String> =
        crate::storage::roots::list_missing(db.conn())?
            .into_iter()
            .map(|root| root.resource_id)
            .collect();

    for app in &root_flatpaks {
        let label = app.display_name.as_deref().unwrap_or(&app.native_id);
        let obs = observations::get(db.conn(), &app.id).ok().flatten();
        let version = obs
            .as_ref()
            .and_then(|o| o.version.as_deref())
            .unwrap_or("unknown");
        let tag = if missing_ids.contains(&app.id) {
            " [missing]"
        } else {
            ""
        };
        println!("  {:<50} {}{}", label, version, tag);
    }

    if !missing_ids.is_empty() {
        println!();
        println!("[missing] recorded as intentional, but currently absent from the system.");
    }

    println!();
    println!("Use 'chapeau flatpaks --all' to see all Flatpak apps and runtimes.");

    Ok(())
}

/// Show all Flatpak apps and runtimes (existing behavior).
fn run_all(db: &Database) -> Result<()> {
    let _ = db; // unused in this path, kept for consistency

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
