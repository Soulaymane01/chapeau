use crate::core::{Relationship, RelationshipOrigin, RelationshipType};
use crate::errors::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Transaction};
use std::str::FromStr;
use uuid::Uuid;

/// Create a relationship between two resources.
pub fn create(
    conn: &Connection,
    source_id: &str,
    relationship_type: RelationshipType,
    target_id: &str,
    origin: RelationshipOrigin,
) -> Result<Relationship> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO relationships (id, source_id, relationship, target_id, origin, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            source_id,
            relationship_type.as_str(),
            target_id,
            origin.as_str(),
            now,
            now,
        ],
    )?;
    Ok(Relationship {
        id,
        source_id: source_id.to_string(),
        relationship_type,
        target_id: target_id.to_string(),
        origin,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Create a relationship within an existing transaction.
pub fn create_tx(
    tx: &Transaction<'_>,
    source_id: &str,
    relationship_type: RelationshipType,
    target_id: &str,
    origin: RelationshipOrigin,
) -> Result<Relationship> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    tx.execute(
        "INSERT INTO relationships (id, source_id, relationship, target_id, origin, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            source_id,
            relationship_type.as_str(),
            target_id,
            origin.as_str(),
            now,
            now,
        ],
    )?;
    Ok(Relationship {
        id,
        source_id: source_id.to_string(),
        relationship_type,
        target_id: target_id.to_string(),
        origin,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Get a relationship by id.
pub fn get(conn: &Connection, id: &str) -> Result<Option<Relationship>> {
    let mut stmt = conn.prepare(
        "SELECT id, source_id, relationship, target_id, origin, created_at, updated_at
         FROM relationships WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], row_to_relationship)?;
    Ok(rows.next().transpose()?)
}

/// List all relationships.
pub fn list(conn: &Connection) -> Result<Vec<Relationship>> {
    let mut stmt = conn.prepare(
        "SELECT id, source_id, relationship, target_id, origin, created_at, updated_at
         FROM relationships ORDER BY created_at",
    )?;
    let rows = stmt.query_map([], row_to_relationship)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// List relationships where source_id matches.
pub fn list_from(conn: &Connection, source_id: &str) -> Result<Vec<Relationship>> {
    let mut stmt = conn.prepare(
        "SELECT id, source_id, relationship, target_id, origin, created_at, updated_at
         FROM relationships WHERE source_id = ?1 ORDER BY created_at",
    )?;
    let rows = stmt.query_map(params![source_id], row_to_relationship)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// List relationships where target_id matches.
pub fn list_to(conn: &Connection, target_id: &str) -> Result<Vec<Relationship>> {
    let mut stmt = conn.prepare(
        "SELECT id, source_id, relationship, target_id, origin, created_at, updated_at
         FROM relationships WHERE target_id = ?1 ORDER BY created_at",
    )?;
    let rows = stmt.query_map(params![target_id], row_to_relationship)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// List relationships filtered by type.
pub fn list_by_type(
    conn: &Connection,
    relationship_type: RelationshipType,
) -> Result<Vec<Relationship>> {
    let mut stmt = conn.prepare(
        "SELECT id, source_id, relationship, target_id, origin, created_at, updated_at
         FROM relationships WHERE relationship = ?1 ORDER BY created_at",
    )?;
    let rows = stmt.query_map(params![relationship_type.as_str()], row_to_relationship)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// Delete a relationship by id. Returns true if a row was deleted.
pub fn delete(conn: &Connection, id: &str) -> Result<bool> {
    let n = conn.execute("DELETE FROM relationships WHERE id = ?1", params![id])?;
    Ok(n > 0)
}

/// Count all relationships.
pub fn count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM relationships", [], |row| row.get(0))?)
}

/// List all relationships where a resource appears as either source or target.
pub fn neighbors(conn: &Connection, resource_id: &str) -> Result<Vec<Relationship>> {
    let mut stmt = conn.prepare(
        "SELECT id, source_id, relationship, target_id, origin, created_at, updated_at
         FROM relationships WHERE source_id = ?1 OR target_id = ?1 ORDER BY created_at",
    )?;
    let rows = stmt.query_map(params![resource_id], row_to_relationship)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// Find an existing relationship by (source_id, relationship_type, target_id).
pub fn find_existing(
    conn: &Connection,
    source_id: &str,
    relationship_type: RelationshipType,
    target_id: &str,
) -> Result<Option<Relationship>> {
    let mut stmt = conn.prepare(
        "SELECT id, source_id, relationship, target_id, origin, created_at, updated_at
         FROM relationships WHERE source_id = ?1 AND relationship = ?2 AND target_id = ?3",
    )?;
    let mut rows = stmt.query_map(
        params![source_id, relationship_type.as_str(), target_id],
        row_to_relationship,
    )?;
    Ok(rows.next().transpose()?)
}

fn row_to_relationship(row: &rusqlite::Row<'_>) -> rusqlite::Result<Relationship> {
    Ok(Relationship {
        id: row.get(0)?,
        source_id: row.get(1)?,
        relationship_type: RelationshipType::from_str(&row.get::<_, String>(2)?)
            .unwrap_or(RelationshipType::DependsOn),
        target_id: row.get(3)?,
        origin: RelationshipOrigin::from_str(&row.get::<_, String>(4)?)
            .unwrap_or(RelationshipOrigin::Derived),
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}
