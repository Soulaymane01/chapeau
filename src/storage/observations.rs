use crate::core::Observation;
use crate::errors::Result;
use chrono::Utc;
use rusqlite::{params, Connection};

/// Insert or replace an observation for a resource.
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
