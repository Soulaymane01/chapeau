use crate::backends::package_backend::PackageBackend;
use crate::backends::DnfCliBackend;
use crate::core::ResourceType;
use crate::errors::Result;
use crate::storage::Database;
use crate::storage::{observations, resources};

pub fn run(db: &Database, all: bool) -> Result<()> {
    if all {
        // Show all packages from DNF (existing behavior)
        run_all(db)
    } else {
        // Show root packages only (progressive disclosure)
        run_roots_only(db)
    }
}

/// Show only root (intentional) packages.
fn run_roots_only(db: &Database) -> Result<()> {
    let root_pkgs = resources::list_roots_by_type(db.conn(), ResourceType::Package)?;

    if root_pkgs.is_empty() {
        println!("No intentional packages (roots) found.");
        println!();
        println!("Run 'chapeau scan' to classify installed software into intentional resources.");
        println!("Or add a root manually: chapeau root add <package>");
        return Ok(());
    }

    println!("Intentional Packages ({} roots):", root_pkgs.len());
    println!();

    for pkg in &root_pkgs {
        let label = pkg.display_name.as_deref().unwrap_or(&pkg.native_id);
        let obs = observations::get(db.conn(), &pkg.id).ok().flatten();
        let version = obs
            .as_ref()
            .and_then(|o| o.version.as_deref())
            .unwrap_or("unknown");
        println!("  {:<30} {}", label, version);
    }

    println!();
    println!(
        "Use 'chapeau packages --all' to see all {} installed packages.",
        crate::storage::resources::count_by_type(db.conn(), ResourceType::Package)?
    );

    Ok(())
}

/// Show all packages from DNF (existing behavior).
fn run_all(_db: &Database) -> Result<()> {
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
