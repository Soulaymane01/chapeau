use crate::core::RelationshipType;
use crate::errors::{ChapeauError, Result};
use crate::storage::Database;
use crate::storage::{relationships, resources};

pub fn run(db: &Database, resource_name: &str) -> Result<()> {
    // Find the resource by native_id
    let resource = resources::find_by_native_id(db.conn(), resource_name)?
        .ok_or_else(|| ChapeauError::ResourceNotFound(resource_name.to_string()))?;

    // Get incoming DependsOn relationships (what depends on this resource)
    let rels = relationships::list_to(db.conn(), &resource.id)?;
    let dependents: Vec<_> = rels
        .iter()
        .filter(|r| r.relationship_type == RelationshipType::DependsOn)
        .collect();

    println!("What depends on '{}':", resource_name);
    println!("====================");

    if dependents.is_empty() {
        println!("  (nothing depends on this resource)");
    } else {
        for rel in &dependents {
            if let Ok(Some(dep)) = resources::get(db.conn(), &rel.source_id) {
                let label = dep.display_name.as_deref().unwrap_or(&dep.native_id);
                println!("  {} ({})", label, dep.resource_type);
            }
        }
    }

    println!();
    println!("Total: {} dependents", dependents.len());

    Ok(())
}
