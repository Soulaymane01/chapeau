use crate::backends::service_backend::ServiceBackend;
use crate::discovery::services::{ActiveState, SystemSnapshot, UnitFileState, UnitRecord};
use crate::errors::{ChapeauError, Result};
use std::collections::{HashMap, HashSet};
use zbus_systemd::systemd1::ManagerProxy;

pub struct SystemdDbusBackend;

impl SystemdDbusBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemdDbusBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceBackend for SystemdDbusBackend {
    fn is_available(&self) -> bool {
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        rt.block_on(async { zbus::Connection::system().await.is_ok() })
    }

    fn discover_units(&self) -> Result<SystemSnapshot> {
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        rt.block_on(async { self.discover_units_async().await })
    }
}

impl SystemdDbusBackend {
    async fn discover_units_async(&self) -> Result<SystemSnapshot> {
        let conn = zbus::Connection::system().await.map_err(|e| {
            ChapeauError::Discovery(format!("failed to connect to system D-Bus: {e}"))
        })?;

        let manager = ManagerProxy::new(&conn).await.map_err(|e| {
            ChapeauError::Discovery(format!("failed to create systemd Manager proxy: {e}"))
        })?;

        let units_raw = manager
            .list_units()
            .await
            .map_err(|e| ChapeauError::Discovery(format!("failed to list systemd units: {e}")))?;

        // Every installed unit file, including units that are currently not
        // loaded. Without this, a stopped service such as postgresql.service
        // would be invisible to Chapeau.
        let mut file_states: HashMap<String, UnitFileState> = HashMap::new();
        if let Ok(unit_files) = manager.list_unit_files().await {
            for (path, state) in &unit_files {
                let Some(file_name) = path.rsplit('/').next() else {
                    continue;
                };
                // Templates cannot run without an instance; alias symlinks are
                // not units of their own.
                if file_name.is_empty() || file_name.contains("@.") || state == "alias" {
                    continue;
                }
                file_states.insert(file_name.to_string(), UnitFileState::from_dbus(state));
            }
        }

        let mut units = Vec::with_capacity(units_raw.len() + file_states.len());
        let mut seen: HashSet<String> = HashSet::with_capacity(units_raw.len());

        for (
            name,
            description,
            load_state,
            active_state,
            sub_state,
            _followed,
            _path,
            _job_id,
            _job_type,
            _job_path,
        ) in units_raw
        {
            let unit_file_state = match file_states.get(&name) {
                Some(state) => Some(state.clone()),
                // Only fall back to a per-unit query when the bulk listing was
                // unavailable (older systemd, permission limits).
                None if file_states.is_empty() => manager
                    .get_unit_file_state(name.clone())
                    .await
                    .ok()
                    .map(|s| UnitFileState::from_dbus(&s)),
                None => None,
            };

            let (base_name, unit_type) = UnitRecord::parse_unit_name(&name);
            seen.insert(name.clone());

            units.push(UnitRecord {
                name: base_name,
                unit_type,
                active_state: ActiveState::from_dbus(&active_state),
                sub_state,
                unit_file_state,
                description,
                load_state,
            });
        }

        // Installed but not loaded: present on disk, currently inactive.
        for (file_name, file_state) in &file_states {
            if seen.contains(file_name) {
                continue;
            }
            let (base_name, unit_type) = UnitRecord::parse_unit_name(file_name);
            units.push(UnitRecord {
                name: base_name,
                unit_type,
                active_state: ActiveState::Inactive,
                sub_state: "dead".to_string(),
                unit_file_state: Some(file_state.clone()),
                description: String::new(),
                load_state: "not-loaded".to_string(),
            });
        }

        Ok(SystemSnapshot {
            units,
            snapshot_time: chrono::Utc::now().to_rfc3339(),
        })
    }
}
