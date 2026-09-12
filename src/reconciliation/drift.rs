use crate::core::{Observation, ResourceType};
use crate::discovery::flatpaks::FlatpakRecord;
use crate::discovery::packages::PackageRecord;
use crate::discovery::services::{ActiveState, UnitRecord};
use crate::discovery::SystemSnapshot;
use crate::errors::Result;
use crate::storage::{observations, resources};
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::fmt;

/// One detected difference between Chapeau's recorded state and the live system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftEntry {
    /// Native resource ID (package name, unit name, Flatpak ID).
    pub name: String,
    /// Resource type the entry belongs to.
    pub resource_type: ResourceType,
    /// What Chapeau had recorded, if anything.
    pub recorded: Option<String>,
    /// What the live system reports, if anything.
    pub actual: Option<String>,
}

/// Differences between Chapeau's recorded state and a fresh discovery.
///
/// This is a *change report*: resources that are already recorded as absent are
/// not reported again, and resources that did not previously exist in Chapeau's
/// model are only reported as "added" once a baseline exists for their type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DriftReport {
    /// Recorded as present, but no longer found on the system.
    pub missing: Vec<DriftEntry>,
    /// Found on the system but not previously recorded (needs a baseline).
    pub added: Vec<DriftEntry>,
    /// Recorded and still present, but with a different state or version.
    pub changed: Vec<DriftEntry>,
}

impl DriftReport {
    /// True when there are no differences at all.
    pub fn is_empty(&self) -> bool {
        self.missing.is_empty() && self.added.is_empty() && self.changed.is_empty()
    }

    /// Total number of differences.
    pub fn total(&self) -> usize {
        self.missing.len() + self.added.len() + self.changed.len()
    }

    /// One-line human-readable summary of the report.
    pub fn summary(&self) -> String {
        if self.is_empty() {
            return "no drift detected".to_string();
        }

        let mut parts = Vec::new();
        if !self.missing.is_empty() {
            parts.push(format!("{} missing", self.missing.len()));
        }
        if !self.added.is_empty() {
            parts.push(format!("{} new", self.added.len()));
        }
        if !self.changed.is_empty() {
            parts.push(format!("{} changed", self.changed.len()));
        }
        parts.join(", ")
    }
}

impl fmt::Display for DriftReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

/// Compare Chapeau's recorded state against a freshly discovered snapshot.
///
/// Read-only: nothing is written. The scan itself uses this before it applies a
/// snapshot so it can report and record what changed; `chapeau drift` uses it
/// against a live discovery without committing anything.
pub fn compute(conn: &Connection, snapshot: &SystemSnapshot) -> Result<DriftReport> {
    let mut report = DriftReport::default();

    if snapshot.sources.dnf {
        compare_packages(conn, snapshot, &mut report)?;
    }
    if snapshot.sources.flatpak {
        compare_flatpaks(conn, snapshot, &mut report)?;
    }
    if snapshot.sources.systemd {
        compare_services(conn, snapshot, &mut report)?;
    }

    Ok(report)
}

fn compare_packages(
    conn: &Connection,
    snapshot: &SystemSnapshot,
    report: &mut DriftReport,
) -> Result<()> {
    let recorded = resources::list_by_type(conn, ResourceType::Package)?;
    let discovered: HashMap<&str, &PackageRecord> = snapshot
        .packages
        .iter()
        .map(|pkg| (pkg.name.as_str(), pkg))
        .collect();

    for res in &recorded {
        let observation = observations::get(conn, &res.id)?;
        match discovered.get(res.native_id.as_str()) {
            Some(pkg) => {
                if let Some(old) = observation.as_ref().and_then(|o| o.version.as_deref()) {
                    if old != pkg.version {
                        report.changed.push(DriftEntry {
                            name: res.native_id.clone(),
                            resource_type: ResourceType::Package,
                            recorded: Some(old.to_string()),
                            actual: Some(pkg.version.clone()),
                        });
                    }
                }
            }
            None => {
                let was_present = observation
                    .as_ref()
                    .and_then(|o| o.installed)
                    .unwrap_or(false);
                if was_present {
                    report.missing.push(DriftEntry {
                        name: res.native_id.clone(),
                        resource_type: ResourceType::Package,
                        recorded: observation.as_ref().and_then(|o| o.version.clone()),
                        actual: None,
                    });
                }
            }
        }
    }

    if !recorded.is_empty() {
        let known: HashSet<&str> = recorded.iter().map(|r| r.native_id.as_str()).collect();
        for pkg in &snapshot.packages {
            if !known.contains(pkg.name.as_str()) {
                report.added.push(DriftEntry {
                    name: pkg.name.clone(),
                    resource_type: ResourceType::Package,
                    recorded: None,
                    actual: Some(pkg.version.clone()),
                });
            }
        }
    }

    Ok(())
}

fn compare_flatpaks(
    conn: &Connection,
    snapshot: &SystemSnapshot,
    report: &mut DriftReport,
) -> Result<()> {
    let recorded = resources::list_by_type(conn, ResourceType::Flatpak)?;
    let discovered: HashMap<&str, &FlatpakRecord> = snapshot
        .flatpak_apps
        .iter()
        .chain(snapshot.flatpak_runtimes.iter())
        .map(|fp| (fp.id.as_str(), fp))
        .collect();

    for res in &recorded {
        let observation = observations::get(conn, &res.id)?;
        match discovered.get(res.native_id.as_str()) {
            Some(fp) => {
                if let Some(old) = observation.as_ref().and_then(|o| o.version.as_deref()) {
                    if old != fp.version {
                        report.changed.push(DriftEntry {
                            name: res.native_id.clone(),
                            resource_type: ResourceType::Flatpak,
                            recorded: Some(old.to_string()),
                            actual: Some(fp.version.clone()),
                        });
                    }
                }
            }
            None => {
                let was_present = observation
                    .as_ref()
                    .and_then(|o| o.installed)
                    .unwrap_or(false);
                if was_present {
                    report.missing.push(DriftEntry {
                        name: res.native_id.clone(),
                        resource_type: ResourceType::Flatpak,
                        recorded: observation.as_ref().and_then(|o| o.version.clone()),
                        actual: None,
                    });
                }
            }
        }
    }

    if !recorded.is_empty() {
        let known: HashSet<&str> = recorded.iter().map(|r| r.native_id.as_str()).collect();
        for fp in snapshot
            .flatpak_apps
            .iter()
            .chain(snapshot.flatpak_runtimes.iter())
        {
            if !known.contains(fp.id.as_str()) {
                report.added.push(DriftEntry {
                    name: fp.id.clone(),
                    resource_type: ResourceType::Flatpak,
                    recorded: None,
                    actual: Some(fp.version.clone()),
                });
            }
        }
    }

    Ok(())
}

fn compare_services(
    conn: &Connection,
    snapshot: &SystemSnapshot,
    report: &mut DriftReport,
) -> Result<()> {
    let recorded = resources::list_by_type(conn, ResourceType::Service)?;
    let discovered: HashMap<&str, &UnitRecord> = snapshot
        .units
        .iter()
        .map(|unit| (unit.name.as_str(), unit))
        .collect();

    for res in &recorded {
        let observation = observations::get(conn, &res.id)?;
        match discovered.get(res.native_id.as_str()) {
            Some(unit) => {
                if let Some(observation) = &observation {
                    let (recorded_diffs, actual_diffs) = state_differences(observation, unit);
                    if !recorded_diffs.is_empty() {
                        report.changed.push(DriftEntry {
                            name: res.native_id.clone(),
                            resource_type: ResourceType::Service,
                            recorded: Some(recorded_diffs.join(", ")),
                            actual: Some(actual_diffs.join(", ")),
                        });
                    }
                }
            }
            None => {
                if let Some(observation) = &observation {
                    let already_absent = observation.installed == Some(false);
                    if !already_absent && is_trackable_unit(&res.native_id) {
                        if let Some(state) = recorded_state(observation) {
                            report.missing.push(DriftEntry {
                                name: res.native_id.clone(),
                                resource_type: ResourceType::Service,
                                recorded: Some(state),
                                actual: None,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Whether a vanished unit is meaningful drift.
///
/// Device, mount, automount, slice and scope units appear and disappear with
/// hardware, removable media and user sessions; their churn is not user
/// intent. Services, sockets, timers, targets and paths are meaningful.
fn is_trackable_unit(native_id: &str) -> bool {
    use crate::discovery::services::UnitType;

    let (_, unit_type) = UnitRecord::parse_unit_name(native_id);
    matches!(
        unit_type,
        UnitType::Service | UnitType::Socket | UnitType::Timer | UnitType::Target | UnitType::Path
    )
}

/// Human-readable state recorded in an observation, if any.
fn recorded_state(observation: &Observation) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(active) = observation.active {
        parts.push(format!("active {}", yes_no(active)));
    }
    if let Some(enabled) = observation.enabled {
        parts.push(format!("enabled {}", yes_no(enabled)));
    }
    if let Some(failed) = observation.failed {
        parts.push(format!("failed {}", yes_no(failed)));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

/// Compare recorded service state against a live unit. Returns parallel lists
/// of the differing fields on each side (e.g. `["active yes"]` / `["active no"]`).
fn state_differences(observation: &Observation, unit: &UnitRecord) -> (Vec<String>, Vec<String>) {
    let current_active = Some(unit.active_state == ActiveState::Active);
    let current_enabled = unit
        .unit_file_state
        .as_ref()
        .map(|state| state.is_enabled());
    let current_failed = Some(unit.active_state == ActiveState::Failed);

    let mut recorded = Vec::new();
    let mut actual = Vec::new();

    let fields: [(&str, Option<bool>, Option<bool>); 3] = [
        ("active", observation.active, current_active),
        ("enabled", observation.enabled, current_enabled),
        ("failed", observation.failed, current_failed),
    ];

    for (label, old, new) in fields {
        if let (Some(old), Some(new)) = (old, new) {
            if old != new {
                recorded.push(format!("{} {}", label, yes_no(old)));
                actual.push(format!("{} {}", label, yes_no(new)));
            }
        }
    }

    (recorded, actual)
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::root::RootSource;
    use crate::discovery::packages::InstallReason;
    use crate::discovery::services::UnitFileState;
    use crate::storage::Database;
    use crate::storage::{resources, roots};

    fn pkg_record(name: &str, version: &str) -> PackageRecord {
        PackageRecord {
            name: name.into(),
            version: version.into(),
            arch: "x86_64".into(),
            repository: "fedora".into(),
            reason: InstallReason::User,
            from_repo: Some("fedora".into()),
            install_time: None,
            summary: None,
            source_rpm: None,
            file_facts: crate::discovery::packages::PackageFileFacts::default(),
        }
    }

    fn unit_record(name: &str, active: ActiveState, enabled: Option<UnitFileState>) -> UnitRecord {
        UnitRecord {
            name: name.into(),
            unit_type: crate::discovery::services::UnitType::Service,
            active_state: active,
            sub_state: "running".into(),
            unit_file_state: enabled,
            description: name.into(),
            load_state: "loaded".into(),
        }
    }

    fn snapshot_with(
        packages: Vec<PackageRecord>,
        units: Vec<UnitRecord>,
        sources: crate::discovery::DiscoverySources,
    ) -> SystemSnapshot {
        let mut snap = SystemSnapshot::empty();
        snap.packages = packages;
        snap.units = units;
        snap.sources = sources;
        snap
    }

    fn package_sources() -> crate::discovery::DiscoverySources {
        crate::discovery::DiscoverySources {
            dnf: true,
            ..Default::default()
        }
    }

    fn insert_recorded_package(
        db: &Database,
        name: &str,
        version: &str,
        installed: bool,
    ) -> crate::core::Resource {
        let res = resources::create(db.conn(), ResourceType::Package, name, Some(name)).unwrap();
        observations::upsert(
            db.conn(),
            &res.id,
            Some(installed),
            Some(version),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        res
    }

    #[test]
    fn missing_package_is_reported() {
        let db = Database::open_memory().unwrap();
        insert_recorded_package(&db, "redis", "7.2.5", true);

        let report = compute(db.conn(), &snapshot_with(vec![], vec![], package_sources())).unwrap();

        assert_eq!(report.missing.len(), 1);
        assert_eq!(report.missing[0].name, "redis");
        assert_eq!(report.missing[0].recorded.as_deref(), Some("7.2.5"));
        assert!(report.added.is_empty());
        assert!(report.changed.is_empty());
    }

    #[test]
    fn already_absent_package_is_not_reported_again() {
        let db = Database::open_memory().unwrap();
        insert_recorded_package(&db, "redis", "7.2.5", false);

        let report = compute(db.conn(), &snapshot_with(vec![], vec![], package_sources())).unwrap();

        assert!(report.is_empty());
    }

    #[test]
    fn partial_discovery_never_reports_missing() {
        let db = Database::open_memory().unwrap();
        insert_recorded_package(&db, "redis", "7.2.5", true);

        // DNF was not consulted: absence cannot be inferred.
        let report = compute(
            db.conn(),
            &snapshot_with(vec![], vec![], Default::default()),
        )
        .unwrap();

        assert!(report.is_empty());
    }

    #[test]
    fn version_change_is_reported() {
        let db = Database::open_memory().unwrap();
        insert_recorded_package(&db, "firefox", "128.0", true);

        let report = compute(
            db.conn(),
            &snapshot_with(
                vec![pkg_record("firefox", "129.0")],
                vec![],
                package_sources(),
            ),
        )
        .unwrap();

        assert_eq!(report.changed.len(), 1);
        assert_eq!(report.changed[0].recorded.as_deref(), Some("128.0"));
        assert_eq!(report.changed[0].actual.as_deref(), Some("129.0"));
    }

    #[test]
    fn new_package_requires_a_baseline() {
        let db = Database::open_memory().unwrap();

        // Empty database: no baseline, so nothing is "added".
        let report = compute(
            db.conn(),
            &snapshot_with(
                vec![pkg_record("neovim", "0.10.2")],
                vec![],
                package_sources(),
            ),
        )
        .unwrap();
        assert!(report.added.is_empty());

        // With a baseline, the new package is reported.
        insert_recorded_package(&db, "firefox", "129.0", true);
        let report = compute(
            db.conn(),
            &snapshot_with(
                vec![
                    pkg_record("firefox", "129.0"),
                    pkg_record("neovim", "0.10.2"),
                ],
                vec![],
                package_sources(),
            ),
        )
        .unwrap();
        assert_eq!(report.added.len(), 1);
        assert_eq!(report.added[0].name, "neovim");
    }

    #[test]
    fn service_state_change_is_reported() {
        let db = Database::open_memory().unwrap();
        let res =
            resources::create(db.conn(), ResourceType::Service, "sshd.service", None).unwrap();
        observations::upsert(
            db.conn(),
            &res.id,
            None,
            None,
            Some(true),
            Some(true),
            Some(false),
            None,
        )
        .unwrap();

        let snap = snapshot_with(
            vec![],
            vec![unit_record(
                "sshd.service",
                ActiveState::Inactive,
                Some(UnitFileState::Enabled),
            )],
            crate::discovery::DiscoverySources {
                systemd: true,
                ..Default::default()
            },
        );
        let report = compute(db.conn(), &snap).unwrap();

        assert_eq!(report.changed.len(), 1);
        assert_eq!(report.changed[0].recorded.as_deref(), Some("active yes"));
        assert_eq!(report.changed[0].actual.as_deref(), Some("active no"));
    }

    #[test]
    fn vanished_service_is_reported_missing() {
        let db = Database::open_memory().unwrap();
        let res =
            resources::create(db.conn(), ResourceType::Service, "sshd.service", None).unwrap();
        observations::upsert(
            db.conn(),
            &res.id,
            None,
            None,
            Some(true),
            None,
            Some(false),
            None,
        )
        .unwrap();

        let snap = snapshot_with(
            vec![],
            vec![],
            crate::discovery::DiscoverySources {
                systemd: true,
                ..Default::default()
            },
        );
        let report = compute(db.conn(), &snap).unwrap();

        assert_eq!(report.missing.len(), 1);
        assert_eq!(
            report.missing[0].recorded.as_deref(),
            Some("active yes, failed no")
        );
    }

    #[test]
    fn vanished_service_already_marked_absent_is_not_reported_again() {
        let db = Database::open_memory().unwrap();
        let res =
            resources::create(db.conn(), ResourceType::Service, "sshd.service", None).unwrap();
        observations::upsert(
            db.conn(),
            &res.id,
            Some(false),
            None,
            Some(true),
            None,
            Some(false),
            None,
        )
        .unwrap();

        let snap = snapshot_with(
            vec![],
            vec![],
            crate::discovery::DiscoverySources {
                systemd: true,
                ..Default::default()
            },
        );
        let report = compute(db.conn(), &snap).unwrap();
        assert!(report.is_empty());
    }

    #[test]
    fn hardware_unit_churn_is_not_reported_missing() {
        let db = Database::open_memory().unwrap();
        // A device unit disappears when hardware disconnects; not drift.
        let res = resources::create(
            db.conn(),
            ResourceType::Service,
            "dev-disk-by\\x2duuid-1234.device",
            None,
        )
        .unwrap();
        observations::upsert(
            db.conn(),
            &res.id,
            None,
            None,
            Some(true),
            None,
            Some(false),
            None,
        )
        .unwrap();

        let snap = snapshot_with(
            vec![],
            vec![],
            crate::discovery::DiscoverySources {
                systemd: true,
                ..Default::default()
            },
        );
        let report = compute(db.conn(), &snap).unwrap();
        assert!(report.missing.is_empty());
    }

    #[test]
    fn summary_formats_counts() {
        let empty = DriftReport::default();
        assert_eq!(empty.summary(), "no drift detected");

        let report = DriftReport {
            missing: vec![DriftEntry {
                name: "redis".into(),
                resource_type: ResourceType::Package,
                recorded: None,
                actual: None,
            }],
            added: vec![
                DriftEntry {
                    name: "neovim".into(),
                    resource_type: ResourceType::Package,
                    recorded: None,
                    actual: None,
                },
                DriftEntry {
                    name: "htop".into(),
                    resource_type: ResourceType::Package,
                    recorded: None,
                    actual: None,
                },
            ],
            changed: vec![],
        };
        assert_eq!(report.total(), 3);
        assert_eq!(report.summary(), "1 missing, 2 new");
    }

    #[test]
    fn missing_root_survives_scan_reconciliation() {
        // Explicit roots are preserved by root reconciliation even when their
        // resource is absent; the resource row itself is never deleted.
        let db = Database::open_memory().unwrap();
        let res = insert_recorded_package(&db, "postgresql-server", "16.4", true);
        roots::create(db.conn(), &res.id, RootSource::User, Some("databases")).unwrap();

        observations::mark_absent(db.conn(), &res.id).unwrap();

        let missing = roots::list_missing(db.conn()).unwrap();
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].resource_id, res.id);
        // The resource itself is preserved.
        assert!(resources::get(db.conn(), &res.id).unwrap().is_some());
    }
}
