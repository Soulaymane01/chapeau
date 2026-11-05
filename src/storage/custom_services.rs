use crate::errors::Result;
use crate::model::CustomServiceDefinition;
use rusqlite::{params, Connection};
use std::collections::BTreeMap;

/// Insert or update a custom service definition.
pub fn upsert(conn: &Connection, definition: &CustomServiceDefinition) -> Result<()> {
    definition.validate()?;
    let env = serde_json::to_string(&definition.env)?;
    conn.execute(
        "INSERT INTO custom_service_definitions
             (item_id, start_command, stop_command, status_command, working_dir,
              env, health_port, health_url, log_path, start_order, enabled)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(item_id) DO UPDATE SET
             start_command = excluded.start_command,
             stop_command = excluded.stop_command,
             status_command = excluded.status_command,
             working_dir = excluded.working_dir,
             env = excluded.env,
             health_port = excluded.health_port,
             health_url = excluded.health_url,
             log_path = excluded.log_path,
             start_order = excluded.start_order,
             enabled = excluded.enabled",
        params![
            definition.item_id,
            definition.start_command,
            definition.stop_command,
            definition.status_command,
            definition.working_dir,
            env,
            definition.health_port.map(i64::from),
            definition.health_url,
            definition.log_path,
            definition.start_order,
            definition.enabled as i32,
        ],
    )?;
    Ok(())
}

/// Get a custom service definition by item ID.
pub fn get(conn: &Connection, item_id: &str) -> Result<Option<CustomServiceDefinition>> {
    let mut stmt = conn.prepare(
        "SELECT item_id, start_command, stop_command, status_command, working_dir,
                env, health_port, health_url, log_path, start_order, enabled
         FROM custom_service_definitions WHERE item_id = ?1",
    )?;
    let mut rows = stmt.query_map(params![item_id], row_to_definition)?;
    Ok(rows.next().transpose()?)
}

/// List all custom service definitions.
pub fn list(conn: &Connection) -> Result<Vec<CustomServiceDefinition>> {
    let mut stmt = conn.prepare(
        "SELECT item_id, start_command, stop_command, status_command, working_dir,
                env, health_port, health_url, log_path, start_order, enabled
         FROM custom_service_definitions ORDER BY start_order, item_id",
    )?;
    let rows = stmt.query_map([], row_to_definition)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// Delete a custom service definition. Returns true when it existed.
pub fn delete(conn: &Connection, item_id: &str) -> Result<bool> {
    let rows = conn.execute(
        "DELETE FROM custom_service_definitions WHERE item_id = ?1",
        params![item_id],
    )?;
    Ok(rows > 0)
}

fn row_to_definition(row: &rusqlite::Row<'_>) -> rusqlite::Result<CustomServiceDefinition> {
    let env: Option<String> = row.get(5)?;
    let env: BTreeMap<String, String> = env
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();
    let health_port: Option<i64> = row.get(6)?;

    Ok(CustomServiceDefinition {
        item_id: row.get(0)?,
        start_command: row.get(1)?,
        stop_command: row.get(2)?,
        status_command: row.get(3)?,
        working_dir: row.get(4)?,
        env,
        health_port: health_port.and_then(|value| u16::try_from(value).ok()),
        health_url: row.get(7)?,
        log_path: row.get(8)?,
        start_order: row.get(9)?,
        enabled: row.get::<_, i64>(10)? != 0,
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

    fn service(db: &Database, name: &str, key: &str) -> String {
        items::upsert(
            db.conn(),
            items::NewItem {
                kind: ItemKind::Service,
                name,
                source: ItemSource::Custom,
                source_key: key,
                scope: Some(crate::model::Scope::User),
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
    fn definition_roundtrip() {
        let db = temp_db();
        let id = service(&db, "Local API", "local-api");
        let mut definition = CustomServiceDefinition::new(&id, "npm run dev");
        definition.stop_command = Some("kill $(cat .pid)".into());
        definition.status_command = Some("test -f .running".into());
        definition.working_dir = Some("/home/user/project".into());
        definition
            .env
            .insert("NODE_ENV".into(), "development".into());
        definition.health_port = Some(3000);
        definition.health_url = Some("http://localhost:3000/health".into());
        definition.log_path = Some("/home/user/.local/state/chapeau/logs/api.log".into());
        definition.start_order = 3;

        upsert(db.conn(), &definition).unwrap();
        assert_eq!(get(db.conn(), &id).unwrap().unwrap(), definition);
        assert_eq!(list(db.conn()).unwrap().len(), 1);
    }

    #[test]
    fn update_replaces_fields() {
        let db = temp_db();
        let id = service(&db, "Local API", "local-api");
        let mut definition = CustomServiceDefinition::new(&id, "start.sh");
        upsert(db.conn(), &definition).unwrap();

        definition.start_command = "start-v2.sh".into();
        definition.enabled = false;
        upsert(db.conn(), &definition).unwrap();

        let stored = get(db.conn(), &id).unwrap().unwrap();
        assert_eq!(stored.start_command, "start-v2.sh");
        assert!(!stored.enabled);
    }

    #[test]
    fn invalid_definition_is_rejected() {
        let db = temp_db();
        let id = service(&db, "Local API", "local-api");
        let mut definition = CustomServiceDefinition::new(&id, "start.sh");
        definition.health_url = Some("localhost:3000".into());
        assert!(upsert(db.conn(), &definition).is_err());
    }

    #[test]
    fn deleting_an_item_cascades_definition() {
        let db = temp_db();
        let id = service(&db, "Local API", "local-api");
        let definition = CustomServiceDefinition::new(&id, "start.sh");
        upsert(db.conn(), &definition).unwrap();

        items::delete(db.conn(), &id).unwrap();
        assert!(get(db.conn(), &id).unwrap().is_none());
    }
}
