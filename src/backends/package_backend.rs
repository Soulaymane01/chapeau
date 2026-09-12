use crate::discovery::packages::{DependencyRecord, PackageRecord, RepoRecord};
use crate::errors::Result;

/// Abstraction for discovering system packages from a package manager.
pub trait PackageBackend {
    /// Check if this backend is available on the current system.
    fn is_available(&self) -> bool;

    /// Discover all installed packages.
    fn discover_installed(&self) -> Result<Vec<PackageRecord>>;

    /// Discover all configured repositories.
    fn discover_repositories(&self) -> Result<Vec<RepoRecord>>;

    /// Query the dependencies of a specific installed package.
    fn query_dependencies(&self, package_name: &str) -> Result<Vec<DependencyRecord>>;

    /// Query which installed packages depend on the given package (reverse dependencies).
    /// Returns a list of package names that depend on `package_name`.
    fn query_reverse_dependencies(&self, package_name: &str) -> Result<Vec<String>>;

    /// The argv (program + arguments) that removes a package.
    ///
    /// Package-manager knowledge stays in the backend; callers only prepend a
    /// privilege launcher (`sudo`, `pkexec`).
    fn removal_argv(&self, package_name: &str) -> Vec<String>;
}
