use crate::errors::ChapeauError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Source-specific facts for an application.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppStatus {
    pub item_id: String,
    /// Absolute path to the desktop entry, if any.
    pub desktop_file: Option<String>,
    /// Raw Exec value from the desktop entry.
    pub exec: Option<String>,
    /// Desktop categories.
    pub categories: Vec<String>,
    /// Owning RPM package, when known.
    pub package_name: Option<String>,
    /// Flatpak remote or package source, when known.
    pub origin: Option<String>,
}

impl AppStatus {
    pub fn new(item_id: impl Into<String>) -> Self {
        Self {
            item_id: item_id.into(),
            desktop_file: None,
            exec: None,
            categories: Vec::new(),
            package_name: None,
            origin: None,
        }
    }
}

/// How a service is managed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeKind {
    Systemd,
    Custom,
}

impl RuntimeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Systemd => "systemd",
            Self::Custom => "custom",
        }
    }

    pub fn all() -> &'static [RuntimeKind] {
        &[RuntimeKind::Systemd, RuntimeKind::Custom]
    }
}

impl fmt::Display for RuntimeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RuntimeKind {
    type Err = ChapeauError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "systemd" => Ok(Self::Systemd),
            "custom" => Ok(Self::Custom),
            _ => Err(ChapeauError::Validation(format!(
                "invalid runtime kind: {value}"
            ))),
        }
    }
}

/// High-level service state shared by systemd and custom services.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ServiceState {
    Running,
    Stopped,
    Failed,
    Unknown,
}

impl ServiceState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }

    pub fn all() -> &'static [ServiceState] {
        &[
            ServiceState::Running,
            ServiceState::Stopped,
            ServiceState::Failed,
            ServiceState::Unknown,
        ]
    }

    /// Derive a state from systemd's active/failed observations.
    pub fn from_systemd(active: Option<bool>, failed: bool) -> Self {
        if failed {
            Self::Failed
        } else {
            match active {
                Some(true) => Self::Running,
                Some(false) => Self::Stopped,
                None => Self::Unknown,
            }
        }
    }
}

impl fmt::Display for ServiceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ServiceState {
    type Err = ChapeauError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "running" => Ok(Self::Running),
            "stopped" => Ok(Self::Stopped),
            "failed" => Ok(Self::Failed),
            "unknown" => Ok(Self::Unknown),
            _ => Err(ChapeauError::Validation(format!(
                "invalid service state: {value}"
            ))),
        }
    }
}

/// Runtime facts for a service.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceStatus {
    pub item_id: String,
    pub runtime: RuntimeKind,
    pub state: ServiceState,
    /// systemd sub-state, when available.
    pub sub_state: Option<String>,
    /// Boot persistence, when known.
    pub enabled: Option<bool>,
    /// Main process ID, when known.
    pub main_pid: Option<u32>,
    /// ISO-8601 timestamp or systemd timestamp string for start time.
    pub since: Option<String>,
    /// Memory usage in bytes, when known.
    pub memory_bytes: Option<u64>,
    /// Unit file path, when known.
    pub unit_path: Option<String>,
    /// Working directory, when known.
    pub working_dir: Option<String>,
    /// ISO-8601 timestamp of the last check.
    pub last_checked: String,
    /// Last error encountered while checking the service.
    pub last_error: Option<String>,
}

impl ServiceStatus {
    pub fn new(item_id: impl Into<String>, runtime: RuntimeKind) -> Self {
        Self {
            item_id: item_id.into(),
            runtime,
            state: ServiceState::Unknown,
            sub_state: None,
            enabled: None,
            main_pid: None,
            since: None,
            memory_bytes: None,
            unit_path: None,
            working_dir: None,
            last_checked: chrono::Utc::now().to_rfc3339(),
            last_error: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.state == ServiceState::Running
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_roundtrip() {
        for runtime in RuntimeKind::all() {
            assert_eq!(RuntimeKind::from_str(runtime.as_str()).unwrap(), *runtime);
        }
    }

    #[test]
    fn state_roundtrip() {
        for state in ServiceState::all() {
            assert_eq!(ServiceState::from_str(state.as_str()).unwrap(), *state);
        }
    }

    #[test]
    fn invalid_values_are_rejected() {
        assert!(RuntimeKind::from_str("bogus").is_err());
        assert!(ServiceState::from_str("bogus").is_err());
    }

    #[test]
    fn state_from_systemd() {
        assert_eq!(
            ServiceState::from_systemd(Some(true), false),
            ServiceState::Running
        );
        assert_eq!(
            ServiceState::from_systemd(Some(false), false),
            ServiceState::Stopped
        );
        assert_eq!(
            ServiceState::from_systemd(Some(true), true),
            ServiceState::Failed
        );
        assert_eq!(
            ServiceState::from_systemd(None, false),
            ServiceState::Unknown
        );
    }

    #[test]
    fn new_status_defaults() {
        let status = ServiceStatus::new("item-1", RuntimeKind::Systemd);
        assert_eq!(status.state, ServiceState::Unknown);
        assert!(status.enabled.is_none());
        assert!(!status.is_running());
        assert!(!status.last_checked.is_empty());
    }

    #[test]
    fn app_status_serialization() {
        let mut status = AppStatus::new("item-1");
        status.categories = vec!["Development".into(), "IDE".into()];
        status.package_name = Some("code".into());
        let json = serde_json::to_string(&status).unwrap();
        let back: AppStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, back);
    }
}
