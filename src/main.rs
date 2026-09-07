mod backends;
mod cli;
mod core;
mod discovery;
mod errors;
mod reconciliation;
mod storage;

use backends::flatpak_backend::FlatpakBackendTrait;
use backends::package_backend::PackageBackend;
use backends::service_backend::ServiceBackend;
use clap::Parser;
use cli::{Cli, Commands};
use storage::Database;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let db_path = Database::default_path();
    let db = Database::open(&db_path)?;

    match &cli.command {
        Commands::Scan => cli::scan::run(&db)?,
        Commands::Status => cli::status::run(&db)?,
        Commands::Why { resource } => cli::why::run(&db, resource)?,
        Commands::Domains { command } => {
            let cmd = command.as_ref().unwrap_or(&cli::domains::Commands::List);
            cli::domains::run(&db, cmd)?
        }
        Commands::Packages => cli::packages::run(&db)?,
        Commands::Services => cli::services::run(&db)?,
        Commands::Flatpaks => cli::flatpaks::run(&db)?,
        Commands::Repositories => cli::repositories::run(&db)?,
        Commands::Dependencies { resource } => cli::dependencies::run(&db, resource)?,
        Commands::Dependents { resource } => cli::dependents::run(&db, resource)?,
        Commands::Backends => {
            let dnf = backends::DnfCliBackend::new();
            let flatpak = backends::FlatpakCliBackend::new();
            let systemd = backends::SystemdDbusBackend::new();
            println!("Backend availability:");
            println!(
                "  DNF5:      {}",
                if dnf.is_available() {
                    "available"
                } else {
                    "not found"
                }
            );
            println!(
                "  Flatpak:   {}",
                if flatpak.is_available() {
                    "available"
                } else {
                    "not found"
                }
            );
            println!(
                "  systemd:   {}",
                if systemd.is_available() {
                    "available"
                } else {
                    "not found"
                }
            );
        }
        Commands::Orphaned => cli::orphaned::run(&db)?,
        Commands::Unused => cli::unused::run(&db)?,
        Commands::Remove { resource } => cli::remove::run(&db, resource)?,
        Commands::Db { command } => cli::db::run(&db, command)?,
    }

    Ok(())
}
