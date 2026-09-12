use crate::core::{Resource, ResourceType};
use crate::errors::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Transaction};
use std::str::FromStr;
use uuid::Uuid;

/// Create a new resource. Returns the created resource.
pub fn create(
    conn: &Connection,
    resource_type: ResourceType,
    native_id: &str,
    display_name: Option<&str>,
) -> Result<Resource> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO resources (id, type, native_id, display_name, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            resource_type.as_str(),
            native_id,
            display_name,
            now,
            now
        ],
    )?;
    Ok(Resource {
        id,
        resource_type,
        native_id: native_id.to_string(),
        display_name: display_name.map(|s| s.to_string()),
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Create a resource within an existing transaction.
pub fn create_tx(
    tx: &Transaction<'_>,
    resource_type: ResourceType,
    native_id: &str,
    display_name: Option<&str>,
) -> Result<Resource> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    tx.execute(
        "INSERT INTO resources (id, type, native_id, display_name, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            resource_type.as_str(),
            native_id,
            display_name,
            now,
            now
        ],
    )?;
    Ok(Resource {
        id,
        resource_type,
        native_id: native_id.to_string(),
        display_name: display_name.map(|s| s.to_string()),
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Get a resource by id.
pub fn get(conn: &Connection, id: &str) -> Result<Option<Resource>> {
    let mut stmt = conn.prepare(
        "SELECT id, type, native_id, display_name, created_at, updated_at
         FROM resources WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], row_to_resource)?;
    Ok(rows.next().transpose()?)
}

/// Get a resource by type and native_id.
pub fn get_by_native(
    conn: &Connection,
    resource_type: ResourceType,
    native_id: &str,
) -> Result<Option<Resource>> {
    let mut stmt = conn.prepare(
        "SELECT id, type, native_id, display_name, created_at, updated_at
         FROM resources WHERE type = ?1 AND native_id = ?2",
    )?;
    let mut rows = stmt.query_map(params![resource_type.as_str(), native_id], row_to_resource)?;
    Ok(rows.next().transpose()?)
}

/// List all resources, ordered by type then native_id.
pub fn list(conn: &Connection) -> Result<Vec<Resource>> {
    let mut stmt = conn.prepare(
        "SELECT id, type, native_id, display_name, created_at, updated_at
         FROM resources ORDER BY type, native_id",
    )?;
    let rows = stmt.query_map([], row_to_resource)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// List resources of a given type.
pub fn list_by_type(conn: &Connection, resource_type: ResourceType) -> Result<Vec<Resource>> {
    let mut stmt = conn.prepare(
        "SELECT id, type, native_id, display_name, created_at, updated_at
         FROM resources WHERE type = ?1 ORDER BY native_id",
    )?;
    let rows = stmt.query_map(params![resource_type.as_str()], row_to_resource)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// Delete a resource by id. Returns true if a row was deleted.
pub fn delete(conn: &Connection, id: &str) -> Result<bool> {
    let n = conn.execute("DELETE FROM resources WHERE id = ?1", params![id])?;
    Ok(n > 0)
}

/// Count all resources.
pub fn count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM resources", [], |row| row.get(0))?)
}

/// Count resources of a given type.
pub fn count_by_type(conn: &Connection, resource_type: ResourceType) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM resources WHERE type = ?1",
        params![resource_type.as_str()],
        |row| row.get(0),
    )?)
}

/// Find a resource by native_id alone, ignoring type.
pub fn find_by_native_id(conn: &Connection, native_id: &str) -> Result<Option<Resource>> {
    let mut stmt = conn.prepare(
        "SELECT id, type, native_id, display_name, created_at, updated_at
         FROM resources WHERE native_id = ?1 LIMIT 1",
    )?;
    let mut rows = stmt.query_map(params![native_id], row_to_resource)?;
    Ok(rows.next().transpose()?)
}

/// Find all resources matching a native_id (may match multiple types).
pub fn find_all_by_native_id(conn: &Connection, native_id: &str) -> Result<Vec<Resource>> {
    let mut stmt = conn.prepare(
        "SELECT id, type, native_id, display_name, created_at, updated_at
         FROM resources WHERE native_id = ?1 ORDER BY type",
    )?;
    let rows = stmt.query_map(params![native_id], row_to_resource)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// List resources that are roots (joined with roots table).
pub fn list_roots(conn: &Connection) -> Result<Vec<Resource>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.type, r.native_id, r.display_name, r.created_at, r.updated_at
         FROM resources r
         JOIN roots ro ON ro.resource_id = r.id
         ORDER BY r.type, r.native_id",
    )?;
    let rows = stmt.query_map([], row_to_resource)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// List resources of a given type that are roots.
pub fn list_roots_by_type(conn: &Connection, resource_type: ResourceType) -> Result<Vec<Resource>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.type, r.native_id, r.display_name, r.created_at, r.updated_at
         FROM resources r
         JOIN roots ro ON ro.resource_id = r.id
         WHERE r.type = ?1
         ORDER BY r.native_id",
    )?;
    let rows = stmt.query_map(params![resource_type.as_str()], row_to_resource)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

pub(crate) fn row_to_resource(row: &rusqlite::Row<'_>) -> rusqlite::Result<Resource> {
    Ok(Resource {
        id: row.get(0)?,
        resource_type: ResourceType::from_str(&row.get::<_, String>(1)?)
            .unwrap_or(ResourceType::Package),
        native_id: row.get(2)?,
        display_name: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}
