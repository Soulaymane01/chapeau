use crate::core::Observation;
use crate::errors::Result;
use chrono::Utc;
use rusqlite::{params, Connection};

/// Insert or replace an observation for a resource.
#[allow(clippy::too_many_arguments)]
pub fn upsert(
    conn: &Connection,
    resource_id: &str,
    installed: Option<bool>,
    version: Option<&str>,
    active: Option<bool>,
    enabled: Option<bool>,
    failed: Option<bool>,
    metadata: Option<&str>,
) -> Result<Observation> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO resource_observations
         (resource_id, observed_at, installed, version, active, enabled, failed, metadata)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            resource_id,
            now,
            installed.map(|b| b as i32),
            version,
            active.map(|b| b as i32),
            enabled.map(|b| b as i32),
            failed.map(|b| b as i32),
            metadata,
        ],
    )?;
    get(conn, resource_id)?
        .ok_or_else(|| crate::errors::ChapeauError::ResourceNotFound(resource_id.to_string()))
}

/// Get the latest observation for a resource.
pub fn get(conn: &Connection, resource_id: &str) -> Result<Option<Observation>> {
    let mut stmt = conn.prepare(
        "SELECT resource_id, observed_at, installed, version, active, enabled, failed, metadata
         FROM resource_observations WHERE resource_id = ?1",
    )?;
    let mut rows = stmt.query_map(params![resource_id], row_to_observation)?;
    Ok(rows.next().transpose()?)
}

/// Mark a resource as no longer present without discarding its other recorded
/// facts (version, metadata, service state). Used when a complete discovery run
/// no longer reports a previously observed resource.
///
/// Returns true if an observation existed and changed.
pub fn mark_absent(conn: &Connection, resource_id: &str) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let rows = conn.execute(
        "UPDATE resource_observations
         SET installed = 0, observed_at = ?2
         WHERE resource_id = ?1 AND (installed IS NULL OR installed != 0)",
        params![resource_id, now],
    )?;
    Ok(rows > 0)
}

/// Count observations that record the resource as currently absent.
pub fn count_absent(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM resource_observations WHERE installed = 0",
        [],
        |row| row.get(0),
    )?)
}

/// List all observations. Used by the explore views to assemble many rows
/// with a single query instead of one query per resource.
pub fn list_all(conn: &Connection) -> Result<Vec<Observation>> {
    let mut stmt = conn.prepare(
        "SELECT resource_id, observed_at, installed, version, active, enabled, failed, metadata
         FROM resource_observations",
    )?;
    let rows = stmt.query_map([], row_to_observation)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// Delete an observation. Returns true if a row was deleted.
pub fn delete(conn: &Connection, resource_id: &str) -> Result<bool> {
    let n = conn.execute(
        "DELETE FROM resource_observations WHERE resource_id = ?1",
        params![resource_id],
    )?;
    Ok(n > 0)
}

/// Count all observations.
pub fn count(conn: &Connection) -> Result<i64> {
    Ok(
        conn.query_row("SELECT COUNT(*) FROM resource_observations", [], |row| {
            row.get(0)
        })?,
    )
}

fn row_to_observation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Observation> {
    let metadata_str: Option<String> = row.get(7)?;
    let metadata = metadata_str.and_then(|s| serde_json::from_str(&s).ok());
    Ok(Observation {
        resource_id: row.get(0)?,
        observed_at: row.get(1)?,
        installed: row.get::<_, Option<i32>>(2)?.map(|v| v != 0),
        version: row.get(3)?,
        active: row.get::<_, Option<i32>>(4)?.map(|v| v != 0),
        enabled: row.get::<_, Option<i32>>(5)?.map(|v| v != 0),
        failed: row.get::<_, Option<i32>>(6)?.map(|v| v != 0),
        metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ResourceType;
    use crate::storage::{resources, Database};

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    #[test]
    fn mark_absent_preserves_recorded_facts() {
        let db = temp_db();
        let conn = db.conn();
        let res = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        upsert(
            conn,
            &res.id,
            Some(true),
            Some("7.2.5"),
            None,
            None,
            None,
            Some("{\"role\":\"application\"}"),
        )
        .unwrap();

        assert!(mark_absent(conn, &res.id).unwrap());

        let obs = get(conn, &res.id).unwrap().unwrap();
        assert_eq!(obs.installed, Some(false));
        assert_eq!(obs.version.as_deref(), Some("7.2.5"));
        assert_eq!(
            obs.metadata.as_ref().and_then(|m| m.get("role")),
            Some(&serde_json::json!("application"))
        );
    }

    #[test]
    fn mark_absent_is_idempotent() {
        let db = temp_db();
        let conn = db.conn();
        let res = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        upsert(conn, &res.id, Some(true), None, None, None, None, None).unwrap();

        assert!(mark_absent(conn, &res.id).unwrap());
        assert!(!mark_absent(conn, &res.id).unwrap());
        assert_eq!(count_absent(conn).unwrap(), 1);
    }

    #[test]
    fn mark_absent_without_observation_is_a_noop() {
        let db = temp_db();
        let conn = db.conn();
        let res = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();

        assert!(!mark_absent(conn, &res.id).unwrap());
        assert!(get(conn, &res.id).unwrap().is_none());
        assert_eq!(count_absent(conn).unwrap(), 0);
    }

    #[test]
    fn reobserving_clears_absent_state() {
        let db = temp_db();
        let conn = db.conn();
        let res = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        upsert(conn, &res.id, Some(true), None, None, None, None, None).unwrap();
        mark_absent(conn, &res.id).unwrap();

        upsert(
            conn,
            &res.id,
            Some(true),
            Some("7.2.6"),
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let obs = get(conn, &res.id).unwrap().unwrap();
        assert_eq!(obs.installed, Some(true));
        assert_eq!(count_absent(conn).unwrap(), 0);
    }
}
