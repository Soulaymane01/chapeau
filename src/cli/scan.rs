use crate::errors::Result;
use crate::reconciliation::scanner;
use crate::storage::Database;

pub fn run(db: &Database) -> Result<()> {
    println!("Discovering system state...");

    let snapshot = scanner::scan(db)?;

    println!();
    println!("Discovered:");
    println!("  Packages:      {}", snapshot.packages.len());
    println!("  Repositories:  {}", snapshot.repositories.len());
    println!("  Services:      {}", snapshot.units.len());
    println!("  Flatpak apps:  {}", snapshot.flatpak_apps.len());
    println!("  Flatpak runtimes: {}", snapshot.flatpak_runtimes.len());
    println!("  Flatpak remotes:  {}", snapshot.flatpak_remotes.len());

    // Show per-repo package counts
    let mut repo_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for pkg in &snapshot.packages {
        *repo_counts
            .entry(pkg.from_repo.as_deref().unwrap_or(&pkg.repository))
            .or_insert(0) += 1;
    }
    if !repo_counts.is_empty() {
        println!();
        println!("Packages by origin repo:");
        let mut repos: Vec<_> = repo_counts.iter().collect();
        repos.sort_by(|a, b| b.1.cmp(a.1));
        for (repo, count) in repos {
            println!("  {:<24} {}", repo, count);
        }
    }

    Ok(())
}
