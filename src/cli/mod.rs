pub mod db;
pub mod dependencies;
pub mod dependents;
pub mod domains;
pub mod drift;
pub mod flatpaks;
pub mod orphaned;
pub mod overview;
pub mod packages;
pub mod remove;
pub mod repositories;
pub mod roots;
pub mod scan;
pub mod services;
pub mod show;
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
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show the My System overview (default when no command is given)
    Overview,

    /// Scan the system and update Chapeau's knowledge base
    Scan,

    /// Show system status summary
    Status,

    /// Show detailed information about a resource
    Show {
        /// Resource name (package, service, flatpak, repository)
        resource: String,
    },

    /// Show drift between Chapeau's recorded state and the live system (read-only)
    Drift,

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

    /// List or manage intentional resources (roots)
    #[command(visible_alias = "root")]
    Roots {
        #[command(subcommand)]
        command: Option<roots::Commands>,
    },

    /// List packages (roots by default, --all for everything)
    Packages {
        /// Show all packages, not just roots
        #[arg(long)]
        all: bool,
    },

    /// List services (roots by default, --all for everything)
    Services {
        /// Show all services, not just roots
        #[arg(long)]
        all: bool,
    },

    /// List Flatpak applications and runtimes
    Flatpaks {
        /// Show all Flatpaks, not just root apps
        #[arg(long)]
        all: bool,
    },

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
