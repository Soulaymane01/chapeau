pub mod flatpaks;
pub mod packages;
pub mod repositories;
pub mod services;

use flatpaks::{FlatpakRecord, FlatpakRemote};
use packages::{PackageRecord, RepoRecord};
use services::UnitRecord;

/// A unified snapshot of all system state discovered from all backends.
///
/// This is the intermediate representation between discovery and reconciliation.
/// It contains raw facts from each backend, without any Chapeau semantics applied.
#[derive(Debug, Clone)]
pub struct SystemSnapshot {
    /// All installed packages discovered from DNF.
    pub packages: Vec<PackageRecord>,
    /// All known repositories discovered from DNF.
    pub repositories: Vec<RepoRecord>,
    /// All loaded systemd units.
    pub units: Vec<UnitRecord>,
    /// All installed Flatpak applications.
    pub flatpak_apps: Vec<FlatpakRecord>,
    /// All installed Flatpak runtimes.
    pub flatpak_runtimes: Vec<FlatpakRecord>,
    /// All configured Flatpak remotes.
    pub flatpak_remotes: Vec<FlatpakRemote>,
    /// ISO-8601 timestamp of when the snapshot was taken.
    pub snapshot_time: String,
}

impl SystemSnapshot {
    /// Create an empty snapshot.
    pub fn empty() -> Self {
        Self {
            packages: Vec::new(),
            repositories: Vec::new(),
            units: Vec::new(),
            flatpak_apps: Vec::new(),
            flatpak_runtimes: Vec::new(),
            flatpak_remotes: Vec::new(),
            snapshot_time: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Total number of resources discovered (packages + units + flatpaks).
    pub fn total_count(&self) -> usize {
        self.packages.len()
            + self.units.len()
            + self.flatpak_apps.len()
            + self.flatpak_runtimes.len()
    }

    /// Number of distinct repositories.
    pub fn repository_count(&self) -> usize {
        self.repositories.len()
    }

    /// Number of distinct Flatpak remotes.
    pub fn flatpak_remote_count(&self) -> usize {
        self.flatpak_remotes.len()
    }
}

/// Summary of what the scan changed in the database.
#[derive(Debug, Clone, Default)]
pub struct ScanSummary {
    /// Number of new resources created.
    pub created: usize,
    /// Number of existing resources updated.
    pub updated: usize,
    /// Number of observations written.
    pub observations: usize,
    /// Number of relationships created.
    pub relationships_created: usize,
    /// Number of resources discovered this scan.
    pub total_discovered: usize,
    /// Per-type breakdown of discovered resources.
    pub by_type: std::collections::HashMap<String, usize>,
}

impl std::fmt::Display for ScanSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Scan complete:")?;
        writeln!(f, "  Discovered: {} resources", self.total_discovered)?;
        writeln!(f, "  Created:    {} new", self.created)?;
        writeln!(f, "  Updated:    {} existing", self.updated)?;
        writeln!(f, "  Observations: {}", self.observations)?;
        writeln!(f, "  Relationships: {}", self.relationships_created)?;
        if !self.by_type.is_empty() {
            writeln!(f, "  By type:")?;
            let mut types: Vec<_> = self.by_type.iter().collect();
            types.sort_by_key(|(k, _)| (*k).clone());
            for (rtype, count) in types {
                writeln!(f, "    {}: {}", rtype, count)?;
            }
        }
        Ok(())
    }
}
