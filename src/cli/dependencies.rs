use crate::core::RelationshipType;
use crate::errors::{ChapeauError, Result};
use crate::storage::Database;
use crate::storage::{relationships, resources};

pub fn run(db: &Database, resource_name: &str) -> Result<()> {
    // Find the resource by native_id
    let resource = resources::find_by_native_id(db.conn(), resource_name)?
        .ok_or_else(|| ChapeauError::ResourceNotFound(resource_name.to_string()))?;

    // Get outgoing DependsOn relationships
    let rels = relationships::list_from(db.conn(), &resource.id)?;
    let deps: Vec<_> = rels
        .iter()
        .filter(|r| r.relationship_type == RelationshipType::DependsOn)
        .collect();

    println!("Dependencies of '{}':", resource_name);
    println!("====================");

    if deps.is_empty() {
        println!("  (none)");
    } else {
        for rel in &deps {
            if let Ok(Some(dep)) = resources::get(db.conn(), &rel.target_id) {
                let label = dep.display_name.as_deref().unwrap_or(&dep.native_id);
                println!("  {} ({})", label, dep.resource_type);
            }
        }
    }

    println!();
    println!("Total: {} dependencies", deps.len());

    Ok(())
}
