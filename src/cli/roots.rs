use crate::errors::{ChapeauError, Result};
use crate::storage::Database;
use crate::storage::{history, resources, roots};
use clap::Subcommand;

#[derive(Subcommand, Clone)]
pub enum Commands {
    /// List intentional resources (roots)
    List {
        /// Include resources hidden from the My System view
        #[arg(long)]
        all: bool,
    },

    /// Add a resource as an intentional root
    Add {
        /// Resource name (package, service, flatpak, etc.)
        resource: String,
        /// Optional reason for marking as root
        #[arg(short, long)]
        reason: Option<String>,
    },

    /// Remove a resource's root status (does not uninstall)
    Remove {
        /// Resource name
        resource: String,
    },

    /// Hide a resource from the default My System view
    Hide {
        /// Resource name
        resource: String,
    },

    /// Restore a hidden resource to the default My System view
    Unhide {
        /// Resource name
        resource: String,
    },
}

pub fn run(db: &Database, command: &Commands) -> Result<()> {
    match command {
        Commands::List { all } => run_list(db, *all),
        Commands::Add { resource, reason } => run_add(db, resource, reason.as_deref()),
        Commands::Remove { resource } => run_remove(db, resource),
        Commands::Hide { resource } => run_hide(db, resource),
        Commands::Unhide { resource } => run_unhide(db, resource),
    }
}

fn run_list(db: &Database, include_hidden: bool) -> Result<()> {
    let all_roots = roots::list(db.conn())?;
    let hidden_count = all_roots
        .iter()
        .filter(|root| root.source == crate::core::root::RootSource::Ignored)
        .count();
    let root_list: Vec<_> = all_roots
        .iter()
        .filter(|root| include_hidden || root.source != crate::core::root::RootSource::Ignored)
        .collect();

    if root_list.is_empty() {
        println!("No intentional resources (roots) found.");
        if hidden_count > 0 {
            println!();
            println!(
                "{} resource(s) are hidden (use 'chapeau roots --all' to include them).",
                hidden_count
            );
        }
        println!();
        println!("Run 'chapeau scan' to detect candidate roots from DNF and Flatpak.");
        println!("Or add a root manually: chapeau root add <resource>");
        return Ok(());
    }

    let missing_ids: std::collections::HashSet<String> = roots::list_missing(db.conn())?
        .into_iter()
        .map(|root| root.resource_id)
        .collect();

    if missing_ids.is_empty() {
        println!("Intentional Resources ({} roots):", root_list.len());
    } else {
        println!(
            "Intentional Resources ({} roots, {} missing):",
            root_list.len(),
            missing_ids.len()
        );
    }
    println!();

    // Group by resource type
    let mut packages = Vec::new();
    let mut flatpaks = Vec::new();
    let mut services = Vec::new();
    let mut other = Vec::new();
    let mut missing_entries: Vec<(&crate::core::root::Root, crate::core::Resource)> = Vec::new();

    for root in root_list.iter().copied() {
        if let Some(res) = resources::get(db.conn(), &root.resource_id)? {
            if missing_ids.contains(&root.resource_id) {
                missing_entries.push((root, res.clone()));
            }
            match res.resource_type {
                crate::core::ResourceType::Package => packages.push((root, res)),
                crate::core::ResourceType::Flatpak => flatpaks.push((root, res)),
                crate::core::ResourceType::Service => services.push((root, res)),
                _ => other.push((root, res)),
            }
        }
    }

    if !packages.is_empty() {
        println!("Packages ({}):", packages.len());
        for (root, res) in &packages {
            let label = res.display_name.as_deref().unwrap_or(&res.native_id);
            let source = &root.source;
            println!(
                "  {:<30} {}",
                label,
                root_tag(source, missing_ids.contains(&res.id))
            );
        }
        println!();
    }

    if !flatpaks.is_empty() {
        println!("Flatpak ({}):", flatpaks.len());
        for (root, res) in &flatpaks {
            let label = res.display_name.as_deref().unwrap_or(&res.native_id);
            let source = &root.source;
            println!(
                "  {:<50} {}",
                label,
                root_tag(source, missing_ids.contains(&res.id))
            );
        }
        println!();
    }

    if !services.is_empty() {
        println!("Services ({}):", services.len());
        for (root, res) in &services {
            let label = res.display_name.as_deref().unwrap_or(&res.native_id);
            let source = &root.source;
            println!(
                "  {:<40} {}",
                label,
                root_tag(source, missing_ids.contains(&res.id))
            );
        }
        println!();
    }

    if !other.is_empty() {
        println!("Other ({}):", other.len());
        for (root, res) in &other {
            let label = res.display_name.as_deref().unwrap_or(&res.native_id);
            let source = &root.source;
            println!(
                "  {:<30} {} {}",
                label,
                res.resource_type,
                root_tag(source, missing_ids.contains(&res.id))
            );
        }
        println!();
    }

    if !missing_entries.is_empty() {
        println!("Missing ({}):", missing_entries.len());
        for (root, res) in &missing_entries {
            let label = res.display_name.as_deref().unwrap_or(&res.native_id);
            if let Some(reason) = &root.reason {
                println!("  {} — recorded intentionally ({})", label, reason);
            } else {
                println!("  {} — recorded intentionally", label);
            }
        }
        println!();
        println!("Missing resources are recorded as intentional but are not currently present.");
        println!("Reinstall them, or use 'chapeau root remove <resource>' if no longer wanted.");
        println!();
    }

    if hidden_count > 0 {
        println!(
            "{} hidden resource(s) — use 'chapeau roots --all' to include them.",
            hidden_count
        );
        println!();
    }

    println!("[user]     explicitly declared with 'chapeau root add'");
    println!("[detected] automatically classified; recomputed on every scan");
    println!("[missing]  recorded as intentional, but currently absent from the system");
    println!();
    println!("Use 'chapeau root add <resource>' to mark additional resources as intentional.");
    println!("Use 'chapeau root remove <resource>' to remove root status (does not uninstall).");
    println!("Use 'chapeau root hide <resource>' to hide a detected root from My System.");

    Ok(())
}

/// Format a root's tags, e.g. `[user, missing]` or `[detected]`.
fn root_tag(source: &crate::core::root::RootSource, missing: bool) -> String {
    if missing {
        format!("[{}, missing]", source)
    } else {
        format!("[{}]", source)
    }
}

fn run_add(db: &Database, resource_name: &str, reason: Option<&str>) -> Result<()> {
    use crate::core::root::RootSource;

    let resource = resources::find_by_native_id(db.conn(), resource_name)?
        .ok_or_else(|| ChapeauError::ResourceNotFound(resource_name.to_string()))?;

    let label = resource.display_name.as_deref().unwrap_or(resource_name);

    // An explicit user declaration always wins over automatic classification:
    // a previously detected root is promoted to source=user, and a previously
    // explicit root is left as-is.
    match roots::get(db.conn(), &resource.id)? {
        Some(existing) if existing.source == RootSource::User => {
            println!("'{}' is already an intentional resource (root).", label);
            return Ok(());
        }
        Some(existing) => {
            roots::update(db.conn(), &resource.id, RootSource::User, reason)?;
            history::insert(
                db.conn(),
                "root.add",
                Some(&resource.resource_type.to_string()),
                Some(&resource.id),
                None,
                Some(&format!(
                    "promoted '{}' from source={} to explicit user root",
                    resource_name, existing.source
                )),
            )?;
            println!("Marked '{}' as an intentional resource (explicit).", label);
            if let Some(r) = reason {
                println!("Reason: {}", r);
            }
            return Ok(());
        }
        None => {}
    }

    let root = roots::create(db.conn(), &resource.id, RootSource::User, reason)?;

    // Record in history
    history::insert(
        db.conn(),
        "root.add",
        Some(&resource.resource_type.to_string()),
        Some(&resource.id),
        None,
        Some(&format!(
            "marked '{}' as intentional root (source: {})",
            resource_name, root.source
        )),
    )?;

    println!("Marked '{}' as an intentional resource.", label);
    if let Some(r) = reason {
        println!("Reason: {}", r);
    }

    Ok(())
}

fn run_remove(db: &Database, resource_name: &str) -> Result<()> {
    let resource = resources::find_by_native_id(db.conn(), resource_name)?
        .ok_or_else(|| ChapeauError::ResourceNotFound(resource_name.to_string()))?;

    let Some(root) = roots::get(db.conn(), &resource.id)? else {
        println!("'{}' is not an intentional resource (root).", resource_name);
        return Ok(());
    };

    roots::delete(db.conn(), &resource.id)?;

    // Record in history
    history::insert(
        db.conn(),
        "root.remove",
        Some(&resource.resource_type.to_string()),
        Some(&resource.id),
        None,
        Some(&format!(
            "removed '{}' from intentional roots (source was {}, resource preserved)",
            resource_name, root.source
        )),
    )?;

    let label = resource.display_name.as_deref().unwrap_or(resource_name);
    println!("Removed '{}' from intentional resources.", label);
    println!("The underlying resource is preserved.");
    if root.source == crate::core::root::RootSource::Detected {
        println!(
            "Note: '{}' was automatically detected and may be re-detected by a future scan.",
            resource_name
        );
    }
    println!(
        "Use 'chapeau root add {}' to mark it as an explicit intentional resource.",
        resource_name
    );

    Ok(())
}

fn run_hide(db: &Database, resource_name: &str) -> Result<()> {
    if roots::get(
        db.conn(),
        &resources::find_by_native_id(db.conn(), resource_name)?
            .ok_or_else(|| ChapeauError::ResourceNotFound(resource_name.to_string()))?
            .id,
    )?
    .is_some_and(|root| root.source == crate::core::root::RootSource::Ignored)
    {
        println!("'{}' is already hidden from My System.", resource_name);
        return Ok(());
    }

    crate::services::roots::hide(db, resource_name)?;
    println!(
        "Hid '{}' from My System. It stays tracked and visible in Explore.",
        resource_name
    );
    println!(
        "Use 'chapeau root unhide {}' to show it again.",
        resource_name
    );
    Ok(())
}

fn run_unhide(db: &Database, resource_name: &str) -> Result<()> {
    match crate::services::roots::unhide(db, resource_name)? {
        Some(_) => {
            println!("'{}' is visible in My System again.", resource_name);
            println!("A detected root may reappear after the next scan.");
        }
        None => {
            println!("'{}' is not hidden.", resource_name);
        }
    }
    Ok(())
}
