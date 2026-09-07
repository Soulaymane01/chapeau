use crate::errors::Result;
use crate::storage::Database;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// Show database status and diagnostics
    Status,
}

pub fn run(db: &Database, command: &Commands) -> Result<()> {
    match command {
        Commands::Status => run_status(db),
    }
}

fn run_status(db: &Database) -> Result<()> {
    let status = db.status()?;

    println!("Chapeau Database Status");
    println!("=======================");
    println!("Path:           {}", status.db_path);
    println!("WAL mode:       {}", status.wal_mode);
    println!(
        "Foreign keys:   {}",
        if status.fk_enabled { "ON" } else { "OFF" }
    );
    println!();
    println!("Contents:");
    println!("  Resources:      {}", status.resource_count);
    println!("  Relationships:  {}", status.relationship_count);
    println!("  Domains:        {}", status.domain_count);

    Ok(())
}
