use crate::errors::Result;
use crate::services::status;
use crate::storage::Database;

pub fn run(db: &Database) -> Result<()> {
    let summary = status::summary(db)?;

    println!("Chapeau Status");
    println!("==============");
    println!();
    println!("Resources:");
    println!("  Total:       {}", summary.total_resources);
    println!("  Packages:    {}", summary.packages);
    println!("  Services:    {}", summary.services);
    println!("  Flatpaks:    {}", summary.flatpaks);
    println!("  Repositories: {}", summary.repositories);
    println!();
    println!("Domains:       {}", summary.domains);
    println!("Untracked:     {}", summary.untracked);
    println!("Missing:       {}", summary.missing);
    println!("Relationships: {}", summary.relationships);
    println!();
    println!("Services:");
    println!("  Active:      {}", summary.active_services);
    println!("  Enabled:     {}", summary.enabled_services);
    println!("  Failed:      {}", summary.failed_services);
    println!();
    println!("Repositories ({}):", summary.repository_names.len());
    for repo in &summary.repository_names {
        println!("  {}", repo);
    }
    println!();
    println!("Observations:  {}", summary.observations);

    Ok(())
}
