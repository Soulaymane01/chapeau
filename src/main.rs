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
        None | Some(Commands::Overview) => cli::overview::run(&db)?,
        Some(Commands::Scan) => cli::scan::run(&db)?,
        Some(Commands::Status) => cli::status::run(&db)?,
        Some(Commands::Show { resource }) => cli::show::run(&db, resource)?,
        Some(Commands::Graph { resource, depth }) => cli::graph::run(&db, resource, *depth)?,
        Some(Commands::Drift) => cli::drift::run(&db)?,
        Some(Commands::Why { resource }) => cli::why::run(&db, resource)?,
        Some(Commands::Domains { command }) => {
            let cmd = command.as_ref().unwrap_or(&cli::domains::Commands::List);
            cli::domains::run(&db, cmd)?
        }
        Some(Commands::Roots { command, all }) => {
            let cmd = command
                .clone()
                .map(|cmd| match cmd {
                    cli::roots::Commands::List { .. } if *all => {
                        cli::roots::Commands::List { all: true }
                    }
                    other => other,
                })
                .unwrap_or(cli::roots::Commands::List { all: *all });
            cli::roots::run(&db, &cmd)?
        }
        Some(Commands::Packages { all }) => cli::packages::run(&db, *all)?,
        Some(Commands::Services { all }) => cli::services::run(&db, *all)?,
        Some(Commands::Flatpaks { all }) => cli::flatpaks::run(&db, *all)?,
        Some(Commands::Repositories) => cli::repositories::run(&db)?,
        Some(Commands::Dependencies { resource }) => cli::dependencies::run(&db, resource)?,
        Some(Commands::Dependents { resource }) => cli::dependents::run(&db, resource)?,
        Some(Commands::Backends) => {
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
        Some(Commands::Orphaned) => cli::orphaned::run(&db)?,
        Some(Commands::Unused) => cli::unused::run(&db)?,
        Some(Commands::Remove { resource }) => cli::remove::run(&db, resource)?,
        Some(Commands::Db { command }) => cli::db::run(&db, command)?,
    }

    Ok(())
}
