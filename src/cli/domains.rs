use crate::core::RelationshipType;
use crate::errors::{ChapeauError, Result};
use crate::storage::Database;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// List all domains
    List,

    /// Create a new domain
    Create {
        /// Domain name
        name: String,

        /// Optional description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Add a resource to a domain
    Add {
        /// Domain name
        domain: String,

        /// Resource name (native_id)
        resource: String,

        /// Use 'owns' relationship (default: uses)
        #[arg(long)]
        owns: bool,

        /// Use 'uses' relationship (default if neither flag set)
        #[arg(long)]
        uses: bool,

        /// Optional reason for this relationship
        #[arg(short, long)]
        reason: Option<String>,
    },

    /// Remove a resource from a domain
    Remove {
        /// Domain name
        domain: String,

        /// Resource name (native_id)
        resource: String,
    },
}

pub fn run(db: &Database, command: &Commands) -> Result<()> {
    match command {
        Commands::List => {
            let domains = crate::storage::domains::list(db.conn())?;
            if domains.is_empty() {
                println!("No domains configured.");
            } else {
                println!("Domains:");
                for d in &domains {
                    let resources = crate::storage::domains::list_resources(db.conn(), &d.id)?;
                    println!(
                        "  {} ({}) - {}",
                        d.name,
                        &d.id[..8],
                        d.description.as_deref().unwrap_or("no description")
                    );
                    for (res, rel_type) in &resources {
                        println!(
                            "    {} {} ({})",
                            rel_type,
                            res.display_name.as_deref().unwrap_or(&res.native_id),
                            res.resource_type,
                        );
                    }
                }
            }
        }
        Commands::Create { name, description } => {
            if crate::storage::domains::get_by_name(db.conn(), name)?.is_some() {
                println!("Domain '{}' already exists.", name);
                return Ok(());
            }
            let domain = crate::storage::domains::create(db.conn(), name, description.as_deref())?;
            println!("Created domain: {} ({})", domain.name, domain.id);
        }
        Commands::Add {
            domain,
            resource,
            owns,
            uses: _,
            reason,
        } => {
            let domain_obj = crate::storage::domains::get_by_name(db.conn(), domain)?
                .ok_or_else(|| ChapeauError::DomainNotFound(domain.clone()))?;

            let res = crate::storage::resources::find_by_native_id(db.conn(), resource)?
                .ok_or_else(|| ChapeauError::ResourceNotFound(resource.clone()))?;

            let rel_type = if *owns {
                RelationshipType::Owns
            } else {
                RelationshipType::Uses
            };

            if crate::storage::domains::has_resource(db.conn(), &domain_obj.id, &res.id, rel_type)?
            {
                println!(
                    "Relationship '{} {} {}' already exists.",
                    domain, rel_type, resource
                );
                return Ok(());
            }

            crate::storage::domains::add_resource(
                db.conn(),
                &domain_obj.id,
                &res.id,
                rel_type,
                reason.as_deref(),
            )?;

            let reason_str = reason
                .as_deref()
                .map(|r| format!(" ({})", r))
                .unwrap_or_default();
            println!("Added: {} {} {}{}", domain, rel_type, resource, reason_str);
        }
        Commands::Remove { domain, resource } => {
            let domain_obj = crate::storage::domains::get_by_name(db.conn(), domain)?
                .ok_or_else(|| ChapeauError::DomainNotFound(domain.clone()))?;

            let res = crate::storage::resources::find_by_native_id(db.conn(), resource)?
                .ok_or_else(|| ChapeauError::ResourceNotFound(resource.clone()))?;

            let present: Vec<RelationshipType> =
                crate::storage::domains::list_for_resource(db.conn(), &res.id)?
                    .into_iter()
                    .filter(|(member_domain, _)| member_domain.id == domain_obj.id)
                    .map(|(_, rel_type)| rel_type)
                    .collect();

            if present.is_empty() {
                println!(
                    "No relationship found between '{}' and '{}'.",
                    domain, resource
                );
                return Ok(());
            }

            crate::storage::domains::remove_resource(db.conn(), &domain_obj.id, &res.id)?;
            for rel_type in present {
                println!("Removed: {} {} {}", domain, rel_type, resource);
            }
        }
    }
    Ok(())
}
