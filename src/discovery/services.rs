use serde::{Deserialize, Serialize};

/// The type of systemd unit (service, socket, timer, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnitType {
    Service,
    Socket,
    Timer,
    Mount,
    Automount,
    Slice,
    Scope,
    Target,
    Path,
    Other(String),
}

impl UnitType {
    pub fn as_str(&self) -> &str {
        match self {
            UnitType::Service => "service",
            UnitType::Socket => "socket",
            UnitType::Timer => "timer",
            UnitType::Mount => "mount",
            UnitType::Automount => "automount",
            UnitType::Slice => "slice",
            UnitType::Scope => "scope",
            UnitType::Target => "target",
            UnitType::Path => "path",
            UnitType::Other(s) => s,
        }
    }

    pub fn from_suffix(suffix: &str) -> Self {
        match suffix {
            "service" => UnitType::Service,
            "socket" => UnitType::Socket,
            "timer" => UnitType::Timer,
            "mount" => UnitType::Mount,
            "automount" => UnitType::Automount,
            "slice" => UnitType::Slice,
            "scope" => UnitType::Scope,
            "target" => UnitType::Target,
            "path" => UnitType::Path,
            other => UnitType::Other(other.to_string()),
        }
    }
}

impl std::fmt::Display for UnitType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Active state of a systemd unit (from D-Bus property ActiveState)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActiveState {
    Active,
    Inactive,
    Failed,
    Activating,
    Deactivating,
    Maintenance,
    Unknown(String),
}

impl ActiveState {
    pub fn as_str(&self) -> &str {
        match self {
            ActiveState::Active => "active",
            ActiveState::Inactive => "inactive",
            ActiveState::Failed => "failed",
            ActiveState::Activating => "activating",
            ActiveState::Deactivating => "deactivating",
            ActiveState::Maintenance => "maintenance",
            ActiveState::Unknown(s) => s,
        }
    }

    pub fn from_dbus(s: &str) -> Self {
        match s {
            "active" => ActiveState::Active,
            "inactive" => ActiveState::Inactive,
            "failed" => ActiveState::Failed,
            "activating" => ActiveState::Activating,
            "deactivating" => ActiveState::Deactivating,
            "maintenance" => ActiveState::Maintenance,
            other => ActiveState::Unknown(other.to_string()),
        }
    }
}

impl std::fmt::Display for ActiveState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Unit file state — whether a unit is enabled/disabled/masked
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnitFileState {
    Enabled,
    EnabledRuntime,
    Linked,
    LinkedRuntime,
    Masked,
    MaskedRuntime,
    Disabled,
    Static,
    Indirect,
    Generated,
    Transient,
    Stub,
    Unknown(String),
}

impl UnitFileState {
    pub fn as_str(&self) -> &str {
        match self {
            UnitFileState::Enabled => "enabled",
            UnitFileState::EnabledRuntime => "enabled-runtime",
            UnitFileState::Linked => "linked",
            UnitFileState::LinkedRuntime => "linked-runtime",
            UnitFileState::Masked => "masked",
            UnitFileState::MaskedRuntime => "masked-runtime",
            UnitFileState::Disabled => "disabled",
            UnitFileState::Static => "static",
            UnitFileState::Indirect => "indirect",
            UnitFileState::Generated => "generated",
            UnitFileState::Transient => "transient",
            UnitFileState::Stub => "stub",
            UnitFileState::Unknown(s) => s,
        }
    }

    pub fn from_dbus(s: &str) -> Self {
        match s {
            "enabled" => UnitFileState::Enabled,
            "enabled-runtime" => UnitFileState::EnabledRuntime,
            "linked" => UnitFileState::Linked,
            "linked-runtime" => UnitFileState::LinkedRuntime,
            "masked" => UnitFileState::Masked,
            "masked-runtime" => UnitFileState::MaskedRuntime,
            "disabled" => UnitFileState::Disabled,
            "static" => UnitFileState::Static,
            "indirect" => UnitFileState::Indirect,
            "generated" => UnitFileState::Generated,
            "transient" => UnitFileState::Transient,
            "stub" => UnitFileState::Stub,
            other => UnitFileState::Unknown(other.to_string()),
        }
    }

    pub fn is_enabled(&self) -> bool {
        matches!(
            self,
            UnitFileState::Enabled
                | UnitFileState::EnabledRuntime
                | UnitFileState::Linked
                | UnitFileState::LinkedRuntime
        )
    }

    pub fn is_masked(&self) -> bool {
        matches!(self, UnitFileState::Masked | UnitFileState::MaskedRuntime)
    }
}

impl std::fmt::Display for UnitFileState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A single systemd unit observation from D-Bus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnitRecord {
    pub name: String,
    pub unit_type: UnitType,
    pub active_state: ActiveState,
    pub sub_state: String,
    pub unit_file_state: Option<UnitFileState>,
    pub description: String,
    pub load_state: String,
}

impl UnitRecord {
    /// Parse a unit name like "sshd.service" into (name, type).
    pub fn parse_unit_name(unit_name: &str) -> (String, UnitType) {
        if let Some(pos) = unit_name.rfind('.') {
            let base = &unit_name[..pos];
            let suffix = &unit_name[pos + 1..];
            (base.to_string(), UnitType::from_suffix(suffix))
        } else {
            (unit_name.to_string(), UnitType::Other("unknown".into()))
        }
    }
}

/// A full snapshot of systemd units at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSnapshot {
    pub units: Vec<UnitRecord>,
    pub snapshot_time: String,
}

impl SystemSnapshot {
    pub fn empty() -> Self {
        Self {
            units: Vec::new(),
            snapshot_time: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn unit_count(&self) -> usize {
        self.units.len()
    }

    pub fn active_count(&self) -> usize {
        self.units
            .iter()
            .filter(|u| u.active_state == ActiveState::Active)
            .count()
    }

    pub fn failed_count(&self) -> usize {
        self.units
            .iter()
            .filter(|u| u.active_state == ActiveState::Failed)
            .count()
    }

    pub fn enabled_count(&self) -> usize {
        self.units
            .iter()
            .filter(|u| u.unit_file_state.as_ref().map_or(false, |s| s.is_enabled()))
            .count()
    }

    pub fn masked_count(&self) -> usize {
        self.units
            .iter()
            .filter(|u| u.unit_file_state.as_ref().map_or(false, |s| s.is_masked()))
            .count()
    }

    pub fn services(&self) -> Vec<&UnitRecord> {
        self.units
            .iter()
            .filter(|u| u.unit_type == UnitType::Service)
            .collect()
    }

    pub fn find_unit(&self, name: &str) -> Option<&UnitRecord> {
        self.units.iter().find(|u| u.name == name)
    }

    pub fn failed_units(&self) -> Vec<&UnitRecord> {
        self.units
            .iter()
            .filter(|u| u.active_state == ActiveState::Failed)
            .collect()
    }

    pub fn units_by_type(&self, unit_type: &UnitType) -> Vec<&UnitRecord> {
        self.units
            .iter()
            .filter(|u| u.unit_type == *unit_type)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- UnitType ---

    #[test]
    fn unit_type_from_suffix() {
        assert_eq!(UnitType::from_suffix("service"), UnitType::Service);
        assert_eq!(UnitType::from_suffix("socket"), UnitType::Socket);
        assert_eq!(UnitType::from_suffix("timer"), UnitType::Timer);
        assert_eq!(UnitType::from_suffix("mount"), UnitType::Mount);
        assert_eq!(UnitType::from_suffix("slice"), UnitType::Slice);
        assert_eq!(
            UnitType::from_suffix("custom"),
            UnitType::Other("custom".into())
        );
    }

    #[test]
    fn unit_type_as_str() {
        assert_eq!(UnitType::Service.as_str(), "service");
        assert_eq!(UnitType::Socket.as_str(), "socket");
        assert_eq!(UnitType::Target.as_str(), "target");
    }

    #[test]
    fn unit_type_display() {
        assert_eq!(UnitType::Service.to_string(), "service");
        assert_eq!(UnitType::Timer.to_string(), "timer");
    }

    // --- ActiveState ---

    #[test]
    fn active_state_from_dbus() {
        assert_eq!(ActiveState::from_dbus("active"), ActiveState::Active);
        assert_eq!(ActiveState::from_dbus("inactive"), ActiveState::Inactive);
        assert_eq!(ActiveState::from_dbus("failed"), ActiveState::Failed);
        assert_eq!(
            ActiveState::from_dbus("activating"),
            ActiveState::Activating
        );
        assert_eq!(
            ActiveState::from_dbus("custom"),
            ActiveState::Unknown("custom".into())
        );
    }

    #[test]
    fn active_state_as_str() {
        assert_eq!(ActiveState::Active.as_str(), "active");
        assert_eq!(ActiveState::Inactive.as_str(), "inactive");
        assert_eq!(ActiveState::Failed.as_str(), "failed");
    }

    // --- UnitFileState ---

    #[test]
    fn unit_file_state_from_dbus() {
        assert_eq!(UnitFileState::from_dbus("enabled"), UnitFileState::Enabled);
        assert_eq!(UnitFileState::from_dbus("masked"), UnitFileState::Masked);
        assert_eq!(
            UnitFileState::from_dbus("disabled"),
            UnitFileState::Disabled
        );
        assert_eq!(UnitFileState::from_dbus("static"), UnitFileState::Static);
    }

    #[test]
    fn unit_file_state_is_enabled() {
        assert!(UnitFileState::Enabled.is_enabled());
        assert!(UnitFileState::EnabledRuntime.is_enabled());
        assert!(UnitFileState::Linked.is_enabled());
        assert!(!UnitFileState::Disabled.is_enabled());
        assert!(!UnitFileState::Static.is_enabled());
        assert!(!UnitFileState::Masked.is_enabled());
    }

    #[test]
    fn unit_file_state_is_masked() {
        assert!(UnitFileState::Masked.is_masked());
        assert!(UnitFileState::MaskedRuntime.is_masked());
        assert!(!UnitFileState::Enabled.is_masked());
        assert!(!UnitFileState::Disabled.is_masked());
    }

    // --- UnitRecord ---

    #[test]
    fn parse_unit_name_service() {
        let (name, utype) = UnitRecord::parse_unit_name("sshd.service");
        assert_eq!(name, "sshd");
        assert_eq!(utype, UnitType::Service);
    }

    #[test]
    fn parse_unit_name_timer() {
        let (name, utype) = UnitRecord::parse_unit_name("dnf-automatic.timer");
        assert_eq!(name, "dnf-automatic");
        assert_eq!(utype, UnitType::Timer);
    }

    #[test]
    fn parse_unit_name_socket() {
        let (name, utype) = UnitRecord::parse_unit_name("dbus.socket");
        assert_eq!(name, "dbus");
        assert_eq!(utype, UnitType::Socket);
    }

    #[test]
    fn parse_unit_name_no_dot() {
        let (name, utype) = UnitRecord::parse_unit_name("basic");
        assert_eq!(name, "basic");
        assert_eq!(utype, UnitType::Other("unknown".into()));
    }

    #[test]
    fn parse_unit_name_multi_dot() {
        // e.g. "user@1000.service" → name="user@1000", type=Service
        let (name, utype) = UnitRecord::parse_unit_name("user@1000.service");
        assert_eq!(name, "user@1000");
        assert_eq!(utype, UnitType::Service);
    }

    // --- SystemSnapshot ---

    #[test]
    fn system_snapshot_empty() {
        let snap = SystemSnapshot::empty();
        assert_eq!(snap.unit_count(), 0);
        assert_eq!(snap.active_count(), 0);
        assert_eq!(snap.failed_count(), 0);
        assert_eq!(snap.enabled_count(), 0);
        assert_eq!(snap.masked_count(), 0);
        assert!(snap.services().is_empty());
        assert!(snap.failed_units().is_empty());
    }

    #[test]
    fn system_snapshot_counts() {
        let snap = SystemSnapshot {
            units: vec![
                UnitRecord {
                    name: "sshd".into(),
                    unit_type: UnitType::Service,
                    active_state: ActiveState::Active,
                    sub_state: "running".into(),
                    unit_file_state: Some(UnitFileState::Enabled),
                    description: "OpenSSH server".into(),
                    load_state: "loaded".into(),
                },
                UnitRecord {
                    name: "nginx".into(),
                    unit_type: UnitType::Service,
                    active_state: ActiveState::Failed,
                    sub_state: "failed".into(),
                    unit_file_state: Some(UnitFileState::Enabled),
                    description: "Nginx web server".into(),
                    load_state: "loaded".into(),
                },
                UnitRecord {
                    name: "basic".into(),
                    unit_type: UnitType::Target,
                    active_state: ActiveState::Active,
                    sub_state: "active".into(),
                    unit_file_state: None,
                    description: "Basic system".into(),
                    load_state: "loaded".into(),
                },
            ],
            snapshot_time: "2025-01-01T00:00:00Z".into(),
        };

        assert_eq!(snap.unit_count(), 3);
        assert_eq!(snap.active_count(), 2);
        assert_eq!(snap.failed_count(), 1);
        assert_eq!(snap.enabled_count(), 2);
        assert_eq!(snap.masked_count(), 0);
        assert_eq!(snap.services().len(), 2);
        assert_eq!(snap.failed_units().len(), 1);
        assert!(snap.find_unit("sshd").is_some());
        assert!(snap.find_unit("nginx").is_some());
        assert!(snap.find_unit("nonexistent").is_none());
    }

    #[test]
    fn system_snapshot_masked_units() {
        let snap = SystemSnapshot {
            units: vec![UnitRecord {
                name: "firewalld".into(),
                unit_type: UnitType::Service,
                active_state: ActiveState::Inactive,
                sub_state: "dead".into(),
                unit_file_state: Some(UnitFileState::Masked),
                description: "Firewall daemon".into(),
                load_state: "loaded".into(),
            }],
            snapshot_time: "2025-01-01T00:00:00Z".into(),
        };

        assert_eq!(snap.masked_count(), 1);
    }

    #[test]
    fn system_snapshot_units_by_type() {
        let snap = SystemSnapshot {
            units: vec![
                UnitRecord {
                    name: "sshd".into(),
                    unit_type: UnitType::Service,
                    active_state: ActiveState::Active,
                    sub_state: "running".into(),
                    unit_file_state: None,
                    description: "".into(),
                    load_state: "loaded".into(),
                },
                UnitRecord {
                    name: "dbus".into(),
                    unit_type: UnitType::Socket,
                    active_state: ActiveState::Active,
                    sub_state: "running".into(),
                    unit_file_state: None,
                    description: "".into(),
                    load_state: "loaded".into(),
                },
            ],
            snapshot_time: "2025-01-01T00:00:00Z".into(),
        };

        assert_eq!(snap.units_by_type(&UnitType::Service).len(), 1);
        assert_eq!(snap.units_by_type(&UnitType::Socket).len(), 1);
        assert_eq!(snap.units_by_type(&UnitType::Timer).len(), 0);
    }

    // --- Serialization ---

    #[test]
    fn unit_record_serialization() {
        let record = UnitRecord {
            name: "sshd".into(),
            unit_type: UnitType::Service,
            active_state: ActiveState::Active,
            sub_state: "running".into(),
            unit_file_state: Some(UnitFileState::Enabled),
            description: "OpenSSH server".into(),
            load_state: "loaded".into(),
        };
        let json = serde_json::to_string(&record).unwrap();
        let back: UnitRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record, back);
    }

    #[test]
    fn active_state_serialization() {
        let states = vec![
            ActiveState::Active,
            ActiveState::Inactive,
            ActiveState::Failed,
            ActiveState::Activating,
            ActiveState::Unknown("custom".into()),
        ];
        for state in &states {
            let json = serde_json::to_string(state).unwrap();
            let back: ActiveState = serde_json::from_str(&json).unwrap();
            assert_eq!(*state, back);
        }
    }
}
