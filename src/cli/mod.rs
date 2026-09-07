pub mod db;
pub mod dependencies;
pub mod dependents;
pub mod domains;
pub mod flatpaks;
pub mod orphaned;
pub mod packages;
pub mod remove;
pub mod repositories;
pub mod scan;
pub mod services;
pub mod status;
pub mod unused;
pub mod why;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "chapeau",
    version,
    about = "Fedora-native system state and relationship manager",
    long_about = "Chapeau answers: What is on my Fedora, why is it there, what does it depend on, and can I safely remove it?"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Scan the system and update Chapeau's knowledge base
    Scan,

    /// Show system status summary
    Status,

    /// Explain why a resource is installed
    Why {
        /// Resource name (package, service, etc.)
        resource: String,
    },

    /// List or manage domains
    Domains {
        #[command(subcommand)]
        command: Option<domains::Commands>,
    },

    /// List packages tracked by Chapeau
    Packages,

    /// List services tracked by Chapeau
    Services,

    /// List Flatpak applications and runtimes
    Flatpaks,

    /// List repositories tracked by Chapeau
    Repositories,

    /// Show dependencies of a resource
    Dependencies {
        /// Resource name
        resource: String,
    },

    /// Show what depends on a resource
    Dependents {
        /// Resource name
        resource: String,
    },

    /// Show backend availability
    Backends,

    /// Analyze potentially orphaned resources (analysis only, no removal)
    Orphaned,

    /// Analyze potentially unused resources (analysis only, no removal)
    Unused,

    /// Plan and execute removal of a resource
    Remove {
        /// Resource name (native_id)
        resource: String,
    },

    /// Database management
    Db {
        #[command(subcommand)]
        command: db::Commands,
    },
}
