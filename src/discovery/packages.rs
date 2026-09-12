use serde::{Deserialize, Serialize};

/// Why a package was installed (from dnf5 `reason` field).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum InstallReason {
    /// Installed as a dependency of another package.
    Dependency,
    /// Installed explicitly by the user.
    User,
    /// Installed as part of a group.
    Group,
    /// Unknown reason.
    Unknown,
}

impl InstallReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            InstallReason::Dependency => "Dependency",
            InstallReason::User => "User",
            InstallReason::Group => "Group",
            InstallReason::Unknown => "Unknown",
        }
    }

    pub fn from_dnf5(s: &str) -> Self {
        match s {
            "Dependency" => InstallReason::Dependency,
            "User" => InstallReason::User,
            "Group" => InstallReason::Group,
            _ => InstallReason::Unknown,
        }
    }
}

impl std::fmt::Display for InstallReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// File-payload facts derived from a package's file list.
///
/// Chapeau does not store the full file list — only the small set of
/// structural signals needed to classify a package's semantic role.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PackageFileFacts {
    /// Whether the package owns any files at all.
    pub has_files: bool,
    /// Whether the package installs a binary in a standard executable dir
    /// (`/usr/bin`, `/usr/sbin`, `/bin`, `/sbin`).
    pub has_executable: bool,
    /// Whether the package installs a `.desktop` entry.
    pub has_desktop_entry: bool,
    /// Whether the package installs an application payload under `/opt`.
    pub has_app_bundle: bool,
}

/// A single installed package discovered from DNF5.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageRecord {
    /// Package name (e.g. "bash").
    pub name: String,
    /// EVR string (e.g. "5.3.9-3.fc44").
    pub version: String,
    /// Architecture (e.g. "x86_64", "noarch").
    pub arch: String,
    /// Repository ID the package came from (e.g. "fedora", "updates").
    pub repository: String,
    /// Why the package was installed.
    pub reason: InstallReason,
    /// The repository the package was originally installed from.
    pub from_repo: Option<String>,
    /// Unix timestamp of installation.
    pub install_time: Option<i64>,
    /// RPM summary (short description), used for semantic role classification.
    #[serde(default)]
    pub summary: Option<String>,
    /// Source RPM the package was built from, used to group subpackages by
    /// project.
    #[serde(default)]
    pub source_rpm: Option<String>,
    /// Structural facts derived from the package file list.
    #[serde(default)]
    pub file_facts: PackageFileFacts,
    /// systemd unit files shipped by the package (e.g. `postgresql.service`),
    /// used to link packages to the services they provide.
    #[serde(default)]
    pub service_units: Vec<String>,
}

/// A dependency relationship discovered from DNF5.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependencyRecord {
    /// The package that has the dependency.
    pub source_package: String,
    /// The raw dependency string from DNF5 (e.g. "libc.so.6()(64bit)").
    pub dependency_string: String,
    /// Type of dependency.
    pub dep_type: DependencyType,
}

/// The type of dependency relationship.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DependencyType {
    Requires,
    Recommends,
    Suggests,
}

impl DependencyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DependencyType::Requires => "requires",
            DependencyType::Recommends => "recommends",
            DependencyType::Suggests => "suggests",
        }
    }
}

/// A repository discovered from DNF5.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoRecord {
    /// Repository ID (e.g. "fedora").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Whether the repository is enabled.
    pub is_enabled: bool,
}

/// A snapshot of system package state discovered from DNF5.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSnapshot {
    /// All installed packages.
    pub packages: Vec<PackageRecord>,
    /// All known repositories.
    pub repositories: Vec<RepoRecord>,
    /// ISO-8601 timestamp of when the snapshot was taken.
    pub snapshot_time: String,
}

impl SystemSnapshot {
    /// Create an empty snapshot.
    pub fn empty() -> Self {
        Self {
            packages: Vec::new(),
            repositories: Vec::new(),
            snapshot_time: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Total number of installed packages.
    pub fn package_count(&self) -> usize {
        self.packages.len()
    }

    /// Number of user-installed packages.
    pub fn user_installed_count(&self) -> usize {
        self.packages
            .iter()
            .filter(|p| p.reason == InstallReason::User)
            .count()
    }

    /// Number of dependency-installed packages.
    pub fn dependency_count(&self) -> usize {
        self.packages
            .iter()
            .filter(|p| p.reason == InstallReason::Dependency)
            .count()
    }

    /// Find a package by name.
    pub fn find_package(&self, name: &str) -> Option<&PackageRecord> {
        self.packages.iter().find(|p| p.name == name)
    }

    /// List packages from a specific repository.
    pub fn packages_from_repo(&self, repo_id: &str) -> Vec<&PackageRecord> {
        self.packages
            .iter()
            .filter(|p| p.repository == repo_id)
            .collect()
    }

    /// List enabled repositories.
    pub fn enabled_repos(&self) -> Vec<&RepoRecord> {
        self.repositories.iter().filter(|r| r.is_enabled).collect()
    }
}
