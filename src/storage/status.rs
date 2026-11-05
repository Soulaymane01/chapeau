use crate::errors::Result;
use crate::model::{AppStatus, RuntimeKind, ServiceState, ServiceStatus};
use rusqlite::{params, Connection};
use std::str::FromStr;

/// Insert or update an application's source-specific facts.
pub fn upsert_app(conn: &Connection, status: &AppStatus) -> Result<()> {
    let categories = serde_json::to_string(&status.categories)?;
    conn.execute(
        "INSERT INTO app_status
             (item_id, desktop_file, exec, categories, package_name, origin)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(item_id) DO UPDATE SET
             desktop_file = excluded.desktop_file,
             exec = excluded.exec,
             categories = excluded.categories,
             package_name = excluded.package_name,
             origin = excluded.origin",
        params![
            status.item_id,
            status.desktop_file,
            status.exec,
            categories,
            status.package_name,
            status.origin,
        ],
    )?;
    Ok(())
}

/// Get an application's source-specific facts.
pub fn get_app(conn: &Connection, item_id: &str) -> Result<Option<AppStatus>> {
    let mut stmt = conn.prepare(
        "SELECT item_id, desktop_file, exec, categories, package_name, origin
         FROM app_status WHERE item_id = ?1",
    )?;
    let mut rows = stmt.query_map(params![item_id], row_to_app_status)?;
    Ok(rows.next().transpose()?)
}

/// Insert or update a service's runtime facts.
pub fn upsert_service(conn: &Connection, status: &ServiceStatus) -> Result<()> {
    conn.execute(
        "INSERT INTO service_status
             (item_id, runtime, state, sub_state, enabled, main_pid, since,
              memory_bytes, unit_path, working_dir, last_checked, last_error)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(item_id) DO UPDATE SET
             runtime = excluded.runtime,
             state = excluded.state,
             sub_state = excluded.sub_state,
             enabled = excluded.enabled,
             main_pid = excluded.main_pid,
             since = excluded.since,
             memory_bytes = excluded.memory_bytes,
             unit_path = excluded.unit_path,
             working_dir = excluded.working_dir,
             last_checked = excluded.last_checked,
             last_error = excluded.last_error",
        params![
            status.item_id,
            status.runtime.as_str(),
            status.state.as_str(),
            status.sub_state,
            status.enabled.map(|value| value as i32),
            status.main_pid.map(i64::from),
            status.since,
            status.memory_bytes.map(|value| value as i64),
            status.unit_path,
            status.working_dir,
            status.last_checked,
            status.last_error,
        ],
    )?;
    Ok(())
}

/// Get a service's runtime facts.
pub fn get_service(conn: &Connection, item_id: &str) -> Result<Option<ServiceStatus>> {
    let mut stmt = conn.prepare(
        "SELECT item_id, runtime, state, sub_state, enabled, main_pid, since,
                memory_bytes, unit_path, working_dir, last_checked, last_error
         FROM service_status WHERE item_id = ?1",
    )?;
    let mut rows = stmt.query_map(params![item_id], row_to_service_status)?;
    Ok(rows.next().transpose()?)
}

/// List every service status row, ordered by item ID.
pub fn list_services(conn: &Connection) -> Result<Vec<ServiceStatus>> {
    let mut stmt = conn.prepare(
        "SELECT item_id, runtime, state, sub_state, enabled, main_pid, since,
                memory_bytes, unit_path, working_dir, last_checked, last_error
         FROM service_status ORDER BY item_id",
    )?;
    let rows = stmt.query_map([], row_to_service_status)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

fn row_to_app_status(row: &rusqlite::Row<'_>) -> rusqlite::Result<AppStatus> {
    let categories: Option<String> = row.get(3)?;
    let categories = categories
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();
    Ok(AppStatus {
        item_id: row.get(0)?,
        desktop_file: row.get(1)?,
        exec: row.get(2)?,
        categories,
        package_name: row.get(4)?,
        origin: row.get(5)?,
    })
}

fn row_to_service_status(row: &rusqlite::Row<'_>) -> rusqlite::Result<ServiceStatus> {
    let runtime_str: String = row.get(1)?;
    let runtime = RuntimeKind::from_str(&runtime_str)
        .map_err(|_| rusqlite::Error::InvalidParameterName(runtime_str.clone()))?;
    let state_str: String = row.get(2)?;
    let state = ServiceState::from_str(&state_str)
        .map_err(|_| rusqlite::Error::InvalidParameterName(state_str.clone()))?;
    let main_pid: Option<i64> = row.get(5)?;
    let memory_bytes: Option<i64> = row.get(7)?;

    Ok(ServiceStatus {
        item_id: row.get(0)?,
        runtime,
        state,
        sub_state: row.get(3)?,
        enabled: row.get::<_, Option<i32>>(4)?.map(|value| value != 0),
        main_pid: main_pid.and_then(|value| u32::try_from(value).ok()),
        since: row.get(6)?,
        memory_bytes: memory_bytes.and_then(|value| u64::try_from(value).ok()),
        unit_path: row.get(8)?,
        working_dir: row.get(9)?,
        last_checked: row.get(10)?,
        last_error: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ItemKind, ItemSource};
    use crate::storage::{items, Database};

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    fn item(db: &Database, kind: ItemKind, name: &str, source: ItemSource, key: &str) -> String {
        items::upsert(
            db.conn(),
            items::NewItem {
                kind,
                name,
                source,
                source_key: key,
                scope: None,
                version: None,
                description: None,
                icon: None,
                command: None,
                metadata: None,
            },
        )
        .unwrap()
        .id
    }

    #[test]
    fn app_status_roundtrip() {
        let db = temp_db();
        let id = item(
            &db,
            ItemKind::App,
            "Code",
            ItemSource::Desktop,
            "code.desktop",
        );
        let mut status = AppStatus::new(&id);
        status.desktop_file = Some("/usr/share/applications/code.desktop".into());
        status.exec = Some("/usr/bin/code %F".into());
        status.categories = vec!["Development".into(), "IDE".into()];
        status.package_name = Some("code".into());
        status.origin = Some("fedora".into());

        upsert_app(db.conn(), &status).unwrap();
        assert_eq!(get_app(db.conn(), &id).unwrap().unwrap(), status);

        status.package_name = Some("code-insiders".into());
        upsert_app(db.conn(), &status).unwrap();
        assert_eq!(
            get_app(db.conn(), &id)
                .unwrap()
                .unwrap()
                .package_name
                .as_deref(),
            Some("code-insiders")
        );
    }

    #[test]
    fn service_status_roundtrip() {
        let db = temp_db();
        let id = item(
            &db,
            ItemKind::Service,
            "PostgreSQL",
            ItemSource::Systemd,
            "postgresql.service",
        );
        let mut status = ServiceStatus::new(&id, RuntimeKind::Systemd);
        status.state = ServiceState::Running;
        status.sub_state = Some("running".into());
        status.enabled = Some(true);
        status.main_pid = Some(4321);
        status.since = Some("2026-11-05T10:00:00Z".into());
        status.memory_bytes = Some(84 * 1024 * 1024);
        status.unit_path = Some("/usr/lib/systemd/system/postgresql.service".into());
        status.working_dir = Some("/var/lib/pgsql".into());
        status.last_error = Some("none".into());

        upsert_service(db.conn(), &status).unwrap();
        assert_eq!(get_service(db.conn(), &id).unwrap().unwrap(), status);
        assert_eq!(list_services(db.conn()).unwrap().len(), 1);
    }

    #[test]
    fn service_status_can_be_updated() {
        let db = temp_db();
        let id = item(
            &db,
            ItemKind::Service,
            "MariaDB",
            ItemSource::Systemd,
            "mariadb.service",
        );
        let mut status = ServiceStatus::new(&id, RuntimeKind::Systemd);
        upsert_service(db.conn(), &status).unwrap();

        status.state = ServiceState::Failed;
        status.enabled = Some(false);
        status.last_error = Some("exit code 1".into());
        upsert_service(db.conn(), &status).unwrap();

        let stored = get_service(db.conn(), &id).unwrap().unwrap();
        assert_eq!(stored.state, ServiceState::Failed);
        assert_eq!(stored.enabled, Some(false));
        assert_eq!(stored.last_error.as_deref(), Some("exit code 1"));
    }

    #[test]
    fn deleting_an_item_cascades_status() {
        let db = temp_db();
        let id = item(
            &db,
            ItemKind::Service,
            "Redis",
            ItemSource::Systemd,
            "redis.service",
        );
        let status = ServiceStatus::new(&id, RuntimeKind::Systemd);
        upsert_service(db.conn(), &status).unwrap();

        crate::storage::items::delete(db.conn(), &id).unwrap();
        assert!(get_service(db.conn(), &id).unwrap().is_none());
    }
}
