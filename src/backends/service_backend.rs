use crate::discovery::services::SystemSnapshot;
use crate::errors::Result;

/// Backend trait for discovering systemd units and services.
///
/// Backends provide raw unit facts. Chapeau provides semantics —
/// no domain logic belongs in backends.
pub trait ServiceBackend {
    /// Check if this backend is available on the current system.
    fn is_available(&self) -> bool;

    /// Discover all loaded systemd units and return a snapshot.
    fn discover_units(&self) -> Result<SystemSnapshot>;
}
