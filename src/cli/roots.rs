use crate::errors::{ChapeauError, Result};
use crate::storage::Database;
use crate::storage::{history, resources, roots};
use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// List all intentional resources (roots)
    List,

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
}

pub fn run(db: &Database, command: &Commands) -> Result<()> {
    match command {
        Commands::List => run_list(db),
        Commands::Add { resource, reason } => run_add(db, resource, reason.as_deref()),
        Commands::Remove { resource } => run_remove(db, resource),
    }
}

fn run_list(db: &Database) -> Result<()> {
    let root_list = roots::list(db.conn())?;

    if root_list.is_empty() {
        println!("No intentional resources (roots) found.");
        println!();
        println!("Run 'chapeau scan' to detect candidate roots from DNF and Flatpak.");
        println!("Or add a root manually: chapeau root add <resource>");
        return Ok(());
    }

    println!("Intentional Resources ({} roots):", root_list.len());
    println!();

    // Group by resource type
    let mut packages = Vec::new();
    let mut flatpaks = Vec::new();
    let mut services = Vec::new();
    let mut other = Vec::new();

    for root in &root_list {
        if let Some(res) = resources::get(db.conn(), &root.resource_id)? {
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
            println!("  {:<30} [{}]", label, source);
        }
        println!();
    }

    if !flatpaks.is_empty() {
        println!("Flatpak ({}):", flatpaks.len());
        for (root, res) in &flatpaks {
            let label = res.display_name.as_deref().unwrap_or(&res.native_id);
            let source = &root.source;
            println!("  {:<50} [{}]", label, source);
        }
        println!();
    }

    if !services.is_empty() {
        println!("Services ({}):", services.len());
        for (root, res) in &services {
            let label = res.display_name.as_deref().unwrap_or(&res.native_id);
            let source = &root.source;
            println!("  {:<40} [{}]", label, source);
        }
        println!();
    }

    if !other.is_empty() {
        println!("Other ({}):", other.len());
        for (root, res) in &other {
            let label = res.display_name.as_deref().unwrap_or(&res.native_id);
            let source = &root.source;
            println!("  {:<30} {} [{}]", label, res.resource_type, source);
        }
        println!();
    }

    println!("[user]     explicitly declared with 'chapeau root add'");
    println!("[detected] automatically classified; recomputed on every scan");
    println!();
    println!("Use 'chapeau root add <resource>' to mark additional resources as intentional.");
    println!("Use 'chapeau root remove <resource>' to remove root status (does not uninstall).");

    Ok(())
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
