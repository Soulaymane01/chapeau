use chapeau::backends::flatpak_backend::FlatpakBackendTrait;
use chapeau::backends::package_backend::PackageBackend;
use chapeau::backends::service_backend::ServiceBackend;
use chapeau::cli::{self, Cli, Commands};
use chapeau::storage::Database;
use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let db_path = Database::default_path();
    let db = Database::open(&db_path)?;

    match &cli.command {
        Commands::Scan => cli::scan::run(&db)?,
        Commands::Status => cli::status::run(&db)?,
        Commands::Drift => cli::drift::run(&db)?,
        Commands::Why { resource } => cli::why::run(&db, resource)?,
        Commands::Domains { command } => {
            let cmd = command.as_ref().unwrap_or(&cli::domains::Commands::List);
            cli::domains::run(&db, cmd)?
        }
        Commands::Roots { command } => {
            let cmd = command.as_ref().unwrap_or(&cli::roots::Commands::List);
            cli::roots::run(&db, cmd)?
        }
        Commands::Packages { all } => cli::packages::run(&db, *all)?,
        Commands::Services { all } => cli::services::run(&db, *all)?,
        Commands::Flatpaks { all } => cli::flatpaks::run(&db, *all)?,
        Commands::Repositories => cli::repositories::run(&db)?,
        Commands::Dependencies { resource } => cli::dependencies::run(&db, resource)?,
        Commands::Dependents { resource } => cli::dependents::run(&db, resource)?,
        Commands::Backends => {
            let dnf = chapeau::backends::DnfCliBackend::new();
            let flatpak = chapeau::backends::FlatpakCliBackend::new();
            let systemd = chapeau::backends::SystemdDbusBackend::new();
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
