use crate::backends::service_backend::ServiceBackend;
use crate::discovery::services::{ActiveState, SystemSnapshot, UnitFileState, UnitRecord};
use crate::errors::{ChapeauError, Result};
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

        let mut units = Vec::with_capacity(units_raw.len());

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
            let unit_file_state = manager
                .get_unit_file_state(name.clone())
                .await
                .ok()
                .map(|s| UnitFileState::from_dbus(&s));

            let (base_name, unit_type) = UnitRecord::parse_unit_name(&name);

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

        Ok(SystemSnapshot {
            units,
            snapshot_time: chrono::Utc::now().to_rfc3339(),
        })
    }
}
