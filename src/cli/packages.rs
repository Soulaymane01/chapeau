use crate::backends::package_backend::PackageBackend;
use crate::backends::DnfCliBackend;
use crate::errors::Result;

pub fn run(_db: &crate::storage::Database) -> Result<()> {
    let backend = DnfCliBackend::new();

    if !backend.is_available() {
        println!("DNF5 is not installed or not available on this system.");
        println!("Install it with: sudo dnf install dnf5");
        return Ok(());
    }

    println!("Discovering packages from DNF5...");
    println!();

    let packages = backend.discover_installed()?;
    let repos = backend.discover_repositories()?;

    // Summary
    let user_count = packages
        .iter()
        .filter(|p| p.reason.as_str() == "User")
        .count();
    let dep_count = packages
        .iter()
        .filter(|p| p.reason.as_str() == "Dependency")
        .count();
    let group_count = packages
        .iter()
        .filter(|p| p.reason.as_str() == "Group")
        .count();

    println!("Installed packages: {}", packages.len());
    println!("  User-installed:  {}", user_count);
    println!("  Dependencies:    {}", dep_count);
    println!("  Group-installed: {}", group_count);
    println!();

    // Repository summary
    let enabled_repos: Vec<_> = repos.iter().filter(|r| r.is_enabled).collect();
    println!(
        "Repositories: {} total ({} enabled)",
        repos.len(),
        enabled_repos.len()
    );
    for repo in &enabled_repos {
        let from_count = packages
            .iter()
            .filter(|p| p.from_repo.as_deref() == Some(&*repo.id))
            .count();
        println!(
            "  {:<30} {:<40} ({} packages installed)",
            repo.id, repo.name, from_count
        );
    }
    println!();

    // Top 20 user-installed packages
    let mut user_pkgs: Vec<_> = packages
        .iter()
        .filter(|p| p.reason.as_str() == "User")
        .collect();
    user_pkgs.sort_by(|a, b| a.name.cmp(&b.name));

    println!("User-installed packages ({} total):", user_pkgs.len());
    for p in user_pkgs.iter().take(20) {
        let origin = p.from_repo.as_deref().unwrap_or("unknown");
        println!(
            "  {:<30} {:<25} {:<15} {}",
            p.name, p.version, p.arch, origin
        );
    }
    if user_pkgs.len() > 20 {
        println!("  ... and {} more", user_pkgs.len() - 20);
    }

    Ok(())
}
