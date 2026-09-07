use serde::{Deserialize, Serialize};

use crate::errors::{ChapeauError, Result};

/// Type of a Flatpak ref.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FlatpakRefKind {
    Application,
    Runtime,
}

impl FlatpakRefKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            FlatpakRefKind::Application => "application",
            FlatpakRefKind::Runtime => "runtime",
        }
    }
}

impl std::fmt::Display for FlatpakRefKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single installed Flatpak ref (application or runtime).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FlatpakRecord {
    /// Application ID (e.g. "org.mozilla.firefox").
    pub id: String,
    /// Human-readable name (e.g. "Firefox Web Browser").
    pub name: String,
    /// Version string (e.g. "138.0.1").
    pub version: String,
    /// Architecture (e.g. "x86_64").
    pub arch: String,
    /// Origin remote name (e.g. "flathub").
    pub origin: String,
    /// Branch name (e.g. "stable", "24.08").
    pub branch: String,
    /// Active commit hash.
    pub active_commit: String,
    /// Runtime used by this app (only set for applications).
    pub runtime: Option<String>,
    /// Installation location ("system" or "user").
    pub installation: String,
    /// Whether this is an application or runtime.
    pub kind: FlatpakRefKind,
}

/// A Flatpak remote repository.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FlatpakRemote {
    /// Remote name (e.g. "flathub").
    pub name: String,
    /// Remote URL.
    pub url: String,
    /// Human-readable title.
    pub title: String,
    /// Options string (e.g. "system,oci").
    pub options: String,
}

/// JSON shape returned by `flatpak list --json`.
#[derive(Debug, Deserialize)]
pub struct FlatpakListItem {
    pub application_id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub arch: Option<String>,
    pub origin: Option<String>,
    pub branch: Option<String>,
    pub active_commit: Option<String>,
    pub runtime: Option<String>,
    pub installation: Option<String>,
}

/// JSON shape returned by `flatpak remotes --json`.
#[derive(Debug, Deserialize)]
pub struct FlatpakRemoteItem {
    pub name: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub options: Option<String>,
}

/// A snapshot of all Flatpak state on the system.
#[derive(Debug, Clone)]
pub struct FlatpakSnapshot {
    /// Installed applications.
    pub apps: Vec<FlatpakRecord>,
    /// Installed runtimes.
    pub runtimes: Vec<FlatpakRecord>,
    /// Configured remotes.
    pub remotes: Vec<FlatpakRemote>,
    /// ISO-8601 timestamp of when the snapshot was taken.
    pub snapshot_time: String,
}

impl FlatpakSnapshot {
    /// Create an empty snapshot.
    pub fn empty() -> Self {
        Self {
            apps: Vec::new(),
            runtimes: Vec::new(),
            remotes: Vec::new(),
            snapshot_time: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Total installed refs (apps + runtimes).
    pub fn total_count(&self) -> usize {
        self.apps.len() + self.runtimes.len()
    }

    /// Find an app by ID.
    pub fn find_app(&self, id: &str) -> Option<&FlatpakRecord> {
        self.apps.iter().find(|r| r.id == id)
    }

    /// Find a runtime by ID.
    pub fn find_runtime(&self, id: &str) -> Option<&FlatpakRecord> {
        self.runtimes.iter().find(|r| r.id == id)
    }

    /// Apps from a specific origin.
    pub fn apps_from_origin(&self, origin: &str) -> Vec<&FlatpakRecord> {
        self.apps.iter().filter(|r| r.origin == origin).collect()
    }

    /// Remotes by name.
    pub fn find_remote(&self, name: &str) -> Option<&FlatpakRemote> {
        self.remotes.iter().find(|r| r.name == name)
    }
}

/// Parse the JSON output of `flatpak list --json --app`.
pub fn parse_flatpak_list_json(json: &str, kind: FlatpakRefKind) -> Result<Vec<FlatpakRecord>> {
    let items: Vec<FlatpakListItem> = serde_json::from_str(json)
        .map_err(|e| ChapeauError::Backend(format!("invalid Flatpak list JSON: {}", e)))?;

    Ok(items
        .into_iter()
        .filter_map(|item| {
            Some(FlatpakRecord {
                id: item.application_id?,
                name: item.name.unwrap_or_default(),
                version: item.version.unwrap_or_default(),
                arch: item.arch.unwrap_or_default(),
                origin: item.origin.unwrap_or_default(),
                branch: item.branch.unwrap_or_default(),
                active_commit: item.active_commit.unwrap_or_default(),
                runtime: item.runtime,
                installation: item.installation.unwrap_or_default(),
                kind,
            })
        })
        .collect())
}

/// Parse the JSON output of `flatpak remotes --json`.
pub fn parse_flatpak_remotes_json(json: &str) -> Result<Vec<FlatpakRemote>> {
    let items: Vec<FlatpakRemoteItem> = serde_json::from_str(json)
        .map_err(|e| ChapeauError::Backend(format!("invalid Flatpak remotes JSON: {}", e)))?;

    Ok(items
        .into_iter()
        .filter_map(|item| {
            Some(FlatpakRemote {
                name: item.name?,
                url: item.url.unwrap_or_default(),
                title: item.title.unwrap_or_default(),
                options: item.options.unwrap_or_default(),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_flatpak_list_json tests ---

    #[test]
    fn parse_app_list_single() {
        let json = r#"[
            {
                "application_id": "org.mozilla.firefox",
                "name": "Firefox Web Browser",
                "version": "138.0.1",
                "arch": "x86_64",
                "origin": "flathub",
                "active_commit": "abc123def456",
                "branch": "stable",
                "runtime": "org.freedesktop.Platform/x86_64/24.08",
                "installation": "system"
            }
        ]"#;

        let apps = parse_flatpak_list_json(json, FlatpakRefKind::Application).unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].id, "org.mozilla.firefox");
        assert_eq!(apps[0].name, "Firefox Web Browser");
        assert_eq!(apps[0].version, "138.0.1");
        assert_eq!(apps[0].arch, "x86_64");
        assert_eq!(apps[0].origin, "flathub");
        assert_eq!(apps[0].branch, "stable");
        assert_eq!(apps[0].active_commit, "abc123def456");
        assert_eq!(
            apps[0].runtime.as_deref(),
            Some("org.freedesktop.Platform/x86_64/24.08")
        );
        assert_eq!(apps[0].installation, "system");
        assert_eq!(apps[0].kind, FlatpakRefKind::Application);
    }

    #[test]
    fn parse_runtime_list_single() {
        let json = r#"[
            {
                "application_id": "org.freedesktop.Platform",
                "name": "Freedesktop Platform",
                "version": "freedesktop-sdk-24.08.30",
                "arch": "x86_64",
                "origin": "flathub",
                "active_commit": "8e2d609ad6f1",
                "branch": "24.08"
            }
        ]"#;

        let runtimes = parse_flatpak_list_json(json, FlatpakRefKind::Runtime).unwrap();
        assert_eq!(runtimes.len(), 1);
        assert_eq!(runtimes[0].id, "org.freedesktop.Platform");
        assert_eq!(runtimes[0].kind, FlatpakRefKind::Runtime);
        assert!(runtimes[0].runtime.is_none());
    }

    #[test]
    fn parse_app_list_multiple() {
        let json = r#"[
            {
                "application_id": "org.mozilla.firefox",
                "name": "Firefox",
                "version": "138.0.1",
                "arch": "x86_64",
                "origin": "flathub",
                "active_commit": "aaa",
                "branch": "stable",
                "runtime": "org.freedesktop.Platform/x86_64/24.08",
                "installation": "system"
            },
            {
                "application_id": "com.visualstudio.code",
                "name": "Visual Studio Code",
                "version": "1.100.0",
                "arch": "x86_64",
                "origin": "flathub",
                "active_commit": "bbb",
                "branch": "stable",
                "runtime": "org.freedesktop.Platform/x86_64/24.08",
                "installation": "system"
            },
            {
                "application_id": "com.slack.Slack",
                "name": "Slack",
                "version": "4.45.0",
                "arch": "x86_64",
                "origin": "flathub",
                "active_commit": "ccc",
                "branch": "stable",
                "runtime": "org.freedesktop.Platform/x86_64/24.08",
                "installation": "user"
            }
        ]"#;

        let apps = parse_flatpak_list_json(json, FlatpakRefKind::Application).unwrap();
        assert_eq!(apps.len(), 3);
        assert_eq!(apps[2].id, "com.slack.Slack");
        assert_eq!(apps[2].installation, "user");
    }

    #[test]
    fn parse_app_list_empty() {
        let json = "[]";
        let apps = parse_flatpak_list_json(json, FlatpakRefKind::Application).unwrap();
        assert!(apps.is_empty());
    }

    #[test]
    fn parse_app_list_missing_optional_fields() {
        let json = r#"[
            {
                "application_id": "org.example.App",
                "arch": "x86_64",
                "origin": "flathub",
                "branch": "stable",
                "active_commit": "abc123"
            }
        ]"#;

        let apps = parse_flatpak_list_json(json, FlatpakRefKind::Application).unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].id, "org.example.App");
        assert_eq!(apps[0].name, "");
        assert_eq!(apps[0].version, "");
        assert!(apps[0].runtime.is_none());
        assert_eq!(apps[0].installation, "");
    }

    #[test]
    fn parse_app_list_missing_application_id_filters_out() {
        let json = r#"[
            {
                "name": "No ID App",
                "version": "1.0",
                "arch": "x86_64",
                "origin": "flathub",
                "branch": "stable",
                "active_commit": "abc"
            }
        ]"#;

        let apps = parse_flatpak_list_json(json, FlatpakRefKind::Application).unwrap();
        assert!(apps.is_empty());
    }

    #[test]
    fn parse_app_list_invalid_json() {
        let json = "not valid json";
        let result = parse_flatpak_list_json(json, FlatpakRefKind::Application);
        assert!(result.is_err());
    }

    // --- parse_flatpak_remotes_json tests ---

    #[test]
    fn parse_remotes_single() {
        let json = r#"[
            {
                "name": "flathub",
                "url": "https://dl.flathub.org/repo/",
                "title": "Flathub",
                "options": "system"
            }
        ]"#;

        let remotes = parse_flatpak_remotes_json(json).unwrap();
        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0].name, "flathub");
        assert_eq!(remotes[0].url, "https://dl.flathub.org/repo/");
        assert_eq!(remotes[0].title, "Flathub");
        assert_eq!(remotes[0].options, "system");
    }

    #[test]
    fn parse_remotes_multiple() {
        let json = r#"[
            {
                "name": "fedora",
                "url": "oci+https://registry.fedoraproject.org",
                "title": "Fedora Flatpaks",
                "options": "system,oci"
            },
            {
                "name": "flathub",
                "url": "https://dl.flathub.org/repo/",
                "title": "Flathub",
                "options": "system"
            }
        ]"#;

        let remotes = parse_flatpak_remotes_json(json).unwrap();
        assert_eq!(remotes.len(), 2);
        assert_eq!(remotes[0].name, "fedora");
        assert_eq!(remotes[1].name, "flathub");
    }

    #[test]
    fn parse_remotes_empty() {
        let json = "[]";
        let remotes = parse_flatpak_remotes_json(json).unwrap();
        assert!(remotes.is_empty());
    }

    #[test]
    fn parse_remotes_missing_optional_fields() {
        let json = r#"[
            {
                "name": "test-remote"
            }
        ]"#;

        let remotes = parse_flatpak_remotes_json(json).unwrap();
        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0].name, "test-remote");
        assert_eq!(remotes[0].url, "");
        assert_eq!(remotes[0].title, "");
        assert_eq!(remotes[0].options, "");
    }

    #[test]
    fn parse_remotes_missing_name_filters_out() {
        let json = r#"[
            {
                "url": "https://example.com",
                "title": "No Name"
            }
        ]"#;

        let remotes = parse_flatpak_remotes_json(json).unwrap();
        assert!(remotes.is_empty());
    }

    #[test]
    fn parse_remotes_invalid_json() {
        let json = "{ broken";
        let result = parse_flatpak_remotes_json(json);
        assert!(result.is_err());
    }

    // --- FlatpakSnapshot helper tests ---

    #[test]
    fn snapshot_empty() {
        let snap = FlatpakSnapshot::empty();
        assert_eq!(snap.total_count(), 0);
        assert!(snap.apps.is_empty());
        assert!(snap.runtimes.is_empty());
        assert!(snap.remotes.is_empty());
    }

    #[test]
    fn snapshot_total_count() {
        let mut snap = FlatpakSnapshot::empty();
        snap.apps.push(FlatpakRecord {
            id: "org.mozilla.firefox".into(),
            name: "Firefox".into(),
            version: "138.0".into(),
            arch: "x86_64".into(),
            origin: "flathub".into(),
            branch: "stable".into(),
            active_commit: "abc".into(),
            runtime: None,
            installation: "system".into(),
            kind: FlatpakRefKind::Application,
        });
        snap.runtimes.push(FlatpakRecord {
            id: "org.freedesktop.Platform".into(),
            name: "Freedesktop Platform".into(),
            version: "24.08".into(),
            arch: "x86_64".into(),
            origin: "flathub".into(),
            branch: "24.08".into(),
            active_commit: "def".into(),
            runtime: None,
            installation: "system".into(),
            kind: FlatpakRefKind::Runtime,
        });
        assert_eq!(snap.total_count(), 2);
    }

    #[test]
    fn snapshot_find_app() {
        let mut snap = FlatpakSnapshot::empty();
        snap.apps.push(FlatpakRecord {
            id: "org.mozilla.firefox".into(),
            name: "Firefox".into(),
            version: "138.0".into(),
            arch: "x86_64".into(),
            origin: "flathub".into(),
            branch: "stable".into(),
            active_commit: "abc".into(),
            runtime: None,
            installation: "system".into(),
            kind: FlatpakRefKind::Application,
        });
        assert!(snap.find_app("org.mozilla.firefox").is_some());
        assert!(snap.find_app("org.nonexistent.App").is_none());
    }

    #[test]
    fn snapshot_find_remote() {
        let mut snap = FlatpakSnapshot::empty();
        snap.remotes.push(FlatpakRemote {
            name: "flathub".into(),
            url: "https://dl.flathub.org/repo/".into(),
            title: "Flathub".into(),
            options: "system".into(),
        });
        assert!(snap.find_remote("flathub").is_some());
        assert!(snap.find_remote("nonexistent").is_none());
    }

    #[test]
    fn snapshot_apps_from_origin() {
        let mut snap = FlatpakSnapshot::empty();
        snap.apps.push(FlatpakRecord {
            id: "org.mozilla.firefox".into(),
            name: "Firefox".into(),
            version: "138.0".into(),
            arch: "x86_64".into(),
            origin: "flathub".into(),
            branch: "stable".into(),
            active_commit: "abc".into(),
            runtime: None,
            installation: "system".into(),
            kind: FlatpakRefKind::Application,
        });
        snap.apps.push(FlatpakRecord {
            id: "org.fedora.GnomeGames".into(),
            name: "GNOME Games".into(),
            version: "46.0".into(),
            arch: "x86_64".into(),
            origin: "fedora".into(),
            branch: "stable".into(),
            active_commit: "def".into(),
            runtime: None,
            installation: "system".into(),
            kind: FlatpakRefKind::Application,
        });

        let flathub = snap.apps_from_origin("flathub");
        assert_eq!(flathub.len(), 1);
        assert_eq!(flathub[0].id, "org.mozilla.firefox");

        let fedora = snap.apps_from_origin("fedora");
        assert_eq!(fedora.len(), 1);
        assert_eq!(fedora[0].id, "org.fedora.GnomeGames");
    }

    // --- FlatpakRefKind display ---

    #[test]
    fn flatpak_ref_kind_display() {
        assert_eq!(FlatpakRefKind::Application.to_string(), "application");
        assert_eq!(FlatpakRefKind::Runtime.to_string(), "runtime");
    }
}
