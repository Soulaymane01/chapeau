use crate::backends::service_backend::ServiceBackend;
use crate::backends::systemd::SystemdDbusBackend;
use crate::core::{RelationshipType, Resource, ResourceType};
use crate::errors::{ChapeauError, Result};
use crate::services::removal::Privilege;
use crate::storage::{history, observations, relationships, resources, roots, Database};
use std::collections::{HashMap, HashSet};
use std::process::Command;

/// Runtime state of a service, used for grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    Failed,
    Running,
    Stopped,
}

impl ServiceState {
    pub fn title(self) -> &'static str {
        match self {
            Self::Failed => "Failed",
            Self::Running => "Running",
            Self::Stopped => "Stopped",
        }
    }

    fn group(self) -> usize {
        match self {
            Self::Failed => 0,
            Self::Running => 1,
            Self::Stopped => 2,
        }
    }
}

/// A control action for a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceAction {
    Start,
    Stop,
}

impl ServiceAction {
    pub fn verb(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
        }
    }

    fn history_action(self) -> &'static str {
        match self {
            Self::Start => "service.start",
            Self::Stop => "service.stop",
        }
    }
}

/// One service row.
#[derive(Debug, Clone)]
pub struct ServiceEntry {
    pub resource: Resource,
    pub state: ServiceState,
    pub enabled: Option<bool>,
    /// True when the user clearly cares: the service is a root or is provided
    /// by a package that is a root.
    pub is_user: bool,
    /// Packages that ship this unit.
    pub provided_by: Vec<String>,
}

/// A state group of services.
#[derive(Debug, Clone)]
pub struct ServiceGroup {
    pub title: &'static str,
    pub entries: Vec<ServiceEntry>,
}

/// List services grouped by state (failed, running, stopped).
///
/// With `user_only`, keeps only services that are intentional roots or that
/// are provided by a package the user deliberately has.
pub fn list(db: &Database, user_only: bool) -> Result<Vec<ServiceGroup>> {
    let conn = db.conn();

    let root_ids: HashSet<String> = roots::root_ids(conn)?.into_iter().collect();
    let packages: HashMap<String, Resource> = resources::list_by_type(conn, ResourceType::Package)?
        .into_iter()
        .map(|package| (package.id.clone(), package))
        .collect();

    let mut grouped: Vec<Vec<ServiceEntry>> = vec![Vec::new(), Vec::new(), Vec::new()];

    for service in resources::list_by_type(conn, ResourceType::Service)? {
        let observation = observations::get(conn, &service.id)?;
        let active = observation.as_ref().and_then(|obs| obs.active);
        let failed = observation.as_ref().and_then(|obs| obs.failed) == Some(true);
        let enabled = observation.as_ref().and_then(|obs| obs.enabled);

        let mut provided_by = Vec::new();
        let mut provided_by_root = false;
        for relationship in relationships::list_to(conn, &service.id)? {
            if relationship.relationship_type != RelationshipType::Provides {
                continue;
            }
            if let Some(package) = packages.get(&relationship.source_id) {
                if root_ids.contains(&package.id) {
                    provided_by_root = true;
                }
                provided_by.push(package.native_id.clone());
            }
        }
        provided_by.sort();

        let is_user = root_ids.contains(&service.id) || provided_by_root;
        if user_only && !is_user {
            continue;
        }

        let state = if failed {
            ServiceState::Failed
        } else if active == Some(true) {
            ServiceState::Running
        } else {
            ServiceState::Stopped
        };

        grouped[state.group()].push(ServiceEntry {
            resource: service,
            state,
            enabled,
            is_user,
            provided_by,
        });
    }

    let mut groups = Vec::new();
    for entries in &mut grouped {
        if entries.is_empty() {
            continue;
        }
        entries.sort_by_key(|entry| entry.resource.native_id.to_lowercase());
        let title = entries[0].state.title();
        groups.push(ServiceGroup {
            title,
            entries: std::mem::take(entries),
        });
    }
    Ok(groups)
}

/// Outcome of a start/stop action.
#[derive(Debug)]
pub struct ServiceControlOutcome {
    pub succeeded: bool,
    pub stderr: String,
}

/// Start or stop a service through systemctl with the given privilege.
///
/// On success the service's observation is refreshed so the UI reflects the
/// new state without a full scan.
pub fn control(
    db: &Database,
    unit: &str,
    action: ServiceAction,
    privilege: Privilege,
) -> Result<ServiceControlOutcome> {
    let resource = resources::find_by_native_id(db.conn(), unit)?
        .ok_or_else(|| ChapeauError::ResourceNotFound(unit.to_string()))?;
    if resource.resource_type != ResourceType::Service {
        return Err(ChapeauError::Validation(format!(
            "'{unit}' is not a service"
        )));
    }

    let output = Command::new(privilege.program())
        .args(["/usr/bin/systemctl", action.verb(), unit])
        .output()?;

    let succeeded = output.status.success();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    history::insert(
        db.conn(),
        action.history_action(),
        Some("service"),
        Some(&resource.id),
        None,
        Some(&format!(
            "{} '{}': {}",
            action.verb(),
            unit,
            if succeeded { "succeeded" } else { "failed" }
        )),
    )?;

    if succeeded {
        let _ = refresh_observation(db, unit);
    }

    Ok(ServiceControlOutcome { succeeded, stderr })
}

/// Re-read one unit's state from systemd and update its observation.
fn refresh_observation(db: &Database, unit: &str) -> Result<()> {
    let backend = SystemdDbusBackend::new();
    if !backend.is_available() {
        return Ok(());
    }
    let snapshot = backend.discover_units()?;
    let Some(record) = snapshot.units.iter().find(|record| record.name == unit) else {
        return Ok(());
    };
    let Some(resource) = resources::find_by_native_id(db.conn(), unit)? else {
        return Ok(());
    };

    let active = Some(record.active_state == crate::discovery::services::ActiveState::Active);
    let failed = Some(record.active_state == crate::discovery::services::ActiveState::Failed);
    let enabled = record
        .unit_file_state
        .as_ref()
        .map(|state| state.is_enabled());
    observations::upsert(
        db.conn(),
        &resource.id,
        None,
        None,
        active,
        enabled,
        failed,
        None,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{RelationshipOrigin, ResourceType};
    use crate::storage::{observations, relationships, resources, roots};

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    fn observe(
        db: &Database,
        resource_id: &str,
        active: Option<bool>,
        enabled: Option<bool>,
        failed: Option<bool>,
    ) {
        observations::upsert(
            db.conn(),
            resource_id,
            None,
            None,
            active,
            enabled,
            failed,
            None,
        )
        .unwrap();
    }

    #[test]
    fn groups_by_state_and_sorts() {
        let db = temp_db();
        let running =
            resources::create(db.conn(), ResourceType::Service, "nginx.service", None).unwrap();
        let failed =
            resources::create(db.conn(), ResourceType::Service, "bad.service", None).unwrap();
        let stopped =
            resources::create(db.conn(), ResourceType::Service, "idle.service", None).unwrap();
        observe(&db, &running.id, Some(true), Some(true), Some(false));
        observe(&db, &failed.id, Some(false), Some(false), Some(true));
        observe(&db, &stopped.id, Some(false), Some(false), Some(false));

        let groups = list(&db, false).unwrap();
        let titles: Vec<&str> = groups.iter().map(|group| group.title).collect();
        assert_eq!(titles, vec!["Failed", "Running", "Stopped"]);
        assert_eq!(groups[0].entries[0].resource.native_id, "bad.service");
        assert_eq!(groups[1].entries[0].resource.native_id, "nginx.service");
        assert_eq!(groups[2].entries[0].resource.native_id, "idle.service");
    }

    #[test]
    fn user_filter_keeps_services_provided_by_roots() {
        let db = temp_db();
        let package =
            resources::create(db.conn(), ResourceType::Package, "postgresql-server", None).unwrap();
        let postgres =
            resources::create(db.conn(), ResourceType::Service, "postgresql.service", None)
                .unwrap();
        let unrelated =
            resources::create(db.conn(), ResourceType::Service, "chronyd.service", None).unwrap();
        relationships::create(
            db.conn(),
            &package.id,
            RelationshipType::Provides,
            &postgres.id,
            RelationshipOrigin::System,
        )
        .unwrap();
        roots::create(
            db.conn(),
            &package.id,
            crate::core::RootSource::Detected,
            None,
        )
        .unwrap();
        observe(&db, &postgres.id, Some(false), Some(true), Some(false));
        observe(&db, &unrelated.id, Some(true), Some(true), Some(false));

        let groups = list(&db, true).unwrap();
        let names: Vec<&str> = groups
            .iter()
            .flat_map(|group| group.entries.iter())
            .map(|entry| entry.resource.native_id.as_str())
            .collect();
        assert_eq!(names, vec!["postgresql.service"]);

        let entries = &groups[0].entries[0];
        assert!(entries.is_user);
        assert_eq!(entries.provided_by, vec!["postgresql-server"]);

        // Without the filter both services are present.
        let all = list(&db, false).unwrap();
        assert_eq!(
            all.iter().map(|group| group.entries.len()).sum::<usize>(),
            2
        );
    }

    #[test]
    fn root_services_are_user_services() {
        let db = temp_db();
        let service =
            resources::create(db.conn(), ResourceType::Service, "my.service", None).unwrap();
        roots::create(db.conn(), &service.id, crate::core::RootSource::User, None).unwrap();

        let groups = list(&db, true).unwrap();
        assert_eq!(groups[0].entries[0].resource.native_id, "my.service");
    }

    #[test]
    fn control_rejects_non_services() {
        let db = temp_db();
        resources::create(db.conn(), ResourceType::Package, "zsh", None).unwrap();
        let error = control(&db, "zsh", ServiceAction::Start, Privilege::Sudo).unwrap_err();
        assert!(error.to_string().contains("not a service"));
    }

    #[test]
    fn action_verbs() {
        assert_eq!(ServiceAction::Start.verb(), "start");
        assert_eq!(ServiceAction::Stop.verb(), "stop");
    }
}
