use crate::core::ResourceType;
use crate::errors::Result;
use crate::storage::Database;
use crate::storage::{domains, observations, relationships, resources};

pub fn run(db: &Database) -> Result<()> {
    let resources_list = resources::list(db.conn())?;
    let domains_list = domains::list(db.conn())?;
    let relationships_list = relationships::list(db.conn())?;

    // Count by type
    let mut by_type: std::collections::HashMap<ResourceType, usize> =
        std::collections::HashMap::new();
    for r in &resources_list {
        *by_type.entry(r.resource_type).or_insert(0) += 1;
    }

    let packages = by_type.get(&ResourceType::Package).copied().unwrap_or(0);
    let services = by_type.get(&ResourceType::Service).copied().unwrap_or(0);
    let flatpaks = by_type.get(&ResourceType::Flatpak).copied().unwrap_or(0);
    let repos = by_type.get(&ResourceType::Repository).copied().unwrap_or(0);

    // Count observations with installed=true
    let all_obs = observations::count(db.conn()).unwrap_or(0);

    // Resources recorded as absent at the last scan.
    let missing_count = observations::count_absent(db.conn()).unwrap_or(0);

    // Untracked = resources with no DependsOn incoming and no ComesFrom relationship
    // (simpler: count resources that have zero relationships at all)
    let mut related_ids = std::collections::HashSet::new();
    for rel in &relationships_list {
        related_ids.insert(rel.source_id.clone());
        related_ids.insert(rel.target_id.clone());
    }
    let untracked = resources_list
        .iter()
        .filter(|r| !related_ids.contains(&r.id))
        .count();

    // Services stats from observations
    let services_list = resources::list_by_type(db.conn(), ResourceType::Service)?;
    let mut active_count = 0;
    let mut failed_count = 0;
    let mut enabled_count = 0;
    for svc in &services_list {
        if let Ok(Some(obs)) = observations::get(db.conn(), &svc.id) {
            if obs.active == Some(true) {
                active_count += 1;
            }
            if obs.failed == Some(true) {
                failed_count += 1;
            }
            if obs.enabled == Some(true) {
                enabled_count += 1;
            }
        }
    }

    // Repository list
    let repo_list = resources::list_by_type(db.conn(), ResourceType::Repository)?;

    println!("Chapeau Status");
    println!("==============");
    println!();
    println!("Resources:");
    println!("  Total:       {}", resources_list.len());
    println!("  Packages:    {}", packages);
    println!("  Services:    {}", services);
    println!("  Flatpaks:    {}", flatpaks);
    println!("  Repositories: {}", repos);
    println!();
    println!("Domains:       {}", domains_list.len());
    println!("Untracked:     {}", untracked);
    println!("Missing:       {}", missing_count);
    println!("Relationships: {}", relationships_list.len());
    println!();
    println!("Services:");
    println!("  Active:      {}", active_count);
    println!("  Enabled:     {}", enabled_count);
    println!("  Failed:      {}", failed_count);
    println!();
    println!("Repositories ({}):", repo_list.len());
    for repo in &repo_list {
        println!("  {}", repo.native_id);
    }
    println!();
    println!("Observations:  {}", all_obs);

    Ok(())
}
