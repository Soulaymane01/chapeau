use crate::core::{Domain, RelationshipType};
use crate::errors::Result;
use chrono::Utc;
use rusqlite::{params, Connection, Transaction};
use std::str::FromStr;
use uuid::Uuid;

/// Create a new domain. Returns the created domain.
pub fn create(conn: &Connection, name: &str, description: Option<&str>) -> Result<Domain> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO domains (id, name, description, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, name, description, now, now],
    )?;
    Ok(Domain {
        id,
        name: name.to_string(),
        description: description.map(|s| s.to_string()),
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Create a domain within an existing transaction.
pub fn create_tx(tx: &Transaction<'_>, name: &str, description: Option<&str>) -> Result<Domain> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    tx.execute(
        "INSERT INTO domains (id, name, description, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, name, description, now, now],
    )?;
    Ok(Domain {
        id,
        name: name.to_string(),
        description: description.map(|s| s.to_string()),
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Get a domain by id.
pub fn get(conn: &Connection, id: &str) -> Result<Option<Domain>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, created_at, updated_at FROM domains WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], row_to_domain)?;
    Ok(rows.next().transpose()?)
}

/// Get a domain by name.
pub fn get_by_name(conn: &Connection, name: &str) -> Result<Option<Domain>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, created_at, updated_at FROM domains WHERE name = ?1",
    )?;
    let mut rows = stmt.query_map(params![name], row_to_domain)?;
    Ok(rows.next().transpose()?)
}

/// List all domains, ordered by name.
pub fn list(conn: &Connection) -> Result<Vec<Domain>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, created_at, updated_at FROM domains ORDER BY name",
    )?;
    let rows = stmt.query_map([], row_to_domain)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// Delete a domain by id. Returns true if a row was deleted.
pub fn delete(conn: &Connection, id: &str) -> Result<bool> {
    let n = conn.execute("DELETE FROM domains WHERE id = ?1", params![id])?;
    Ok(n > 0)
}

/// Update a domain's name and/or description.
pub fn update(
    conn: &Connection,
    id: &str,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let n = conn.execute(
        "UPDATE domains SET name = COALESCE(?2, name), description = ?3, updated_at = ?4 WHERE id = ?1",
        params![id, name, description, now],
    )?;
    Ok(n > 0)
}

/// List resources belonging to a domain, via 'owns' or 'uses' relationships in domain_resources.
pub fn list_resources(
    conn: &Connection,
    domain_id: &str,
) -> Result<Vec<(crate::core::Resource, crate::core::RelationshipType)>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.type, r.native_id, r.display_name, r.created_at, r.updated_at,
                dr.relationship
         FROM domain_resources dr
         JOIN resources r ON r.id = dr.resource_id
         WHERE dr.domain_id = ?1
           AND dr.relationship IN ('owns', 'uses')
         ORDER BY r.type, r.native_id",
    )?;
    let rows = stmt.query_map(params![domain_id], |row| {
        let res = crate::storage::resources::row_to_resource(row)?;
        let rel_type_str: String = row.get(6)?;
        let rel_type = RelationshipType::from_str(&rel_type_str).unwrap_or(RelationshipType::Uses);
        Ok((res, rel_type))
    })?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// List the domains a resource belongs to, with the relationship type
/// ('owns' or 'uses'), ordered by domain name.
pub fn list_for_resource(
    conn: &Connection,
    resource_id: &str,
) -> Result<Vec<(Domain, RelationshipType)>> {
    let mut stmt = conn.prepare(
        "SELECT d.id, d.name, d.description, d.created_at, d.updated_at, dr.relationship
         FROM domain_resources dr
         JOIN domains d ON d.id = dr.domain_id
         WHERE dr.resource_id = ?1
           AND dr.relationship IN ('owns', 'uses')
         ORDER BY d.name",
    )?;
    let rows = stmt.query_map(params![resource_id], |row| {
        let domain = row_to_domain(row)?;
        let rel_type_str: String = row.get(5)?;
        let rel_type = RelationshipType::from_str(&rel_type_str).unwrap_or(RelationshipType::Uses);
        Ok((domain, rel_type))
    })?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// Whether a resource is associated with a domain under the given relationship.
pub fn has_resource(
    conn: &Connection,
    domain_id: &str,
    resource_id: &str,
    relationship: RelationshipType,
) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM domain_resources
         WHERE domain_id = ?1 AND resource_id = ?2 AND relationship = ?3",
        params![domain_id, resource_id, relationship.as_str()],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Associate a resource with a domain ('owns' or 'uses').
///
/// Domain associations live in `domain_resources`, never in the
/// resource-to-resource `relationships` table. Re-adding an existing
/// association updates its reason.
pub fn add_resource(
    conn: &Connection,
    domain_id: &str,
    resource_id: &str,
    relationship: RelationshipType,
    reason: Option<&str>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO domain_resources
             (domain_id, resource_id, relationship, reason, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(domain_id, resource_id, relationship)
         DO UPDATE SET reason = excluded.reason, updated_at = excluded.updated_at",
        params![
            domain_id,
            resource_id,
            relationship.as_str(),
            reason,
            now,
            now
        ],
    )?;
    Ok(())
}

/// Remove a resource's association with a domain (both 'owns' and 'uses').
/// Returns true when at least one association existed.
pub fn remove_resource(conn: &Connection, domain_id: &str, resource_id: &str) -> Result<bool> {
    let n = conn.execute(
        "DELETE FROM domain_resources WHERE domain_id = ?1 AND resource_id = ?2",
        params![domain_id, resource_id],
    )?;
    Ok(n > 0)
}

fn row_to_domain(row: &rusqlite::Row<'_>) -> rusqlite::Result<Domain> {
    Ok(Domain {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}
