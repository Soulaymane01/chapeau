use crate::discovery::flatpaks::{FlatpakRecord, FlatpakRemote, FlatpakSnapshot};
use crate::errors::Result;

/// Abstraction for discovering system Flatpak state.
pub trait FlatpakBackendTrait {
    /// Check if this backend is available on the current system.
    fn is_available(&self) -> bool;

    /// Discover all installed Flatpak applications.
    fn discover_installed(&self) -> Result<Vec<FlatpakRecord>>;

    /// Discover all installed Flatpak runtimes.
    fn discover_runtimes(&self) -> Result<Vec<FlatpakRecord>>;

    /// Discover all configured Flatpak remotes.
    fn discover_remotes(&self) -> Result<Vec<FlatpakRemote>>;

    /// Build a full snapshot (apps + runtimes + remotes).
    fn discover_snapshot(&self) -> Result<FlatpakSnapshot> {
        let mut apps = self.discover_installed()?;
        let mut runtimes = self.discover_runtimes()?;
        let remotes = self.discover_remotes()?;
        // Sort by id then branch for deterministic output.
        apps.sort_by(|a, b| a.id.cmp(&b.id).then(a.branch.cmp(&b.branch)));
        runtimes.sort_by(|a, b| a.id.cmp(&b.id).then(a.branch.cmp(&b.branch)));

        Ok(FlatpakSnapshot {
            apps,
            runtimes,
            remotes,
            snapshot_time: chrono::Utc::now().to_rfc3339(),
        })
    }
}
