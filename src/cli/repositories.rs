use crate::core::{RelationshipType, ResourceType};
use crate::errors::Result;
use crate::storage::Database;
use crate::storage::{relationships, resources};

pub fn run(db: &Database) -> Result<()> {
    let repos = resources::list_by_type(db.conn(), ResourceType::Repository)?;

    if repos.is_empty() {
        println!("No repositories in the Chapeau model.");
        println!("Run 'chapeau scan' to discover system repositories.");
        return Ok(());
    }

    println!("Repositories ({} total):", repos.len());

    for repo in &repos {
        // Count packages that come from this repo
        let rels = relationships::list_to(db.conn(), &repo.id)?;
        let package_count = rels
            .iter()
            .filter(|r| r.relationship_type == RelationshipType::ComesFrom)
            .count();

        let label = repo.display_name.as_deref().unwrap_or(&repo.native_id);
        println!();
        println!("  {} ({} packages)", repo.native_id, package_count);
        if label != repo.native_id {
            println!("    {}", label);
        }
    }

    Ok(())
}
