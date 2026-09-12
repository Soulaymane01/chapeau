use crate::core::Provenance;
use crate::errors::Result;
use rusqlite::{params, Connection};

/// Insert or replace provenance for a resource.
#[allow(clippy::too_many_arguments)]
pub fn upsert(
    conn: &Connection,
    resource_id: &str,
    installation_method: Option<&str>,
    installation_time: Option<&str>,
    transaction_id: Option<&str>,
    source_type: Option<&str>,
    source_id: Option<&str>,
    adopted_at: Option<&str>,
) -> Result<Provenance> {
    conn.execute(
        "INSERT OR REPLACE INTO provenance
         (resource_id, installation_method, installation_time, transaction_id,
          source_type, source_id, adopted_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            resource_id,
            installation_method,
            installation_time,
            transaction_id,
            source_type,
            source_id,
            adopted_at,
        ],
    )?;
    get(conn, resource_id)?
        .ok_or_else(|| crate::errors::ChapeauError::ResourceNotFound(resource_id.to_string()))
}

/// Get provenance for a resource.
pub fn get(conn: &Connection, resource_id: &str) -> Result<Option<Provenance>> {
    let mut stmt = conn.prepare(
        "SELECT resource_id, installation_method, installation_time, transaction_id,
                source_type, source_id, adopted_at
         FROM provenance WHERE resource_id = ?1",
    )?;
    let mut rows = stmt.query_map(params![resource_id], row_to_provenance)?;
    Ok(rows.next().transpose()?)
}

/// Delete provenance for a resource. Returns true if a row was deleted.
pub fn delete(conn: &Connection, resource_id: &str) -> Result<bool> {
    let n = conn.execute(
        "DELETE FROM provenance WHERE resource_id = ?1",
        params![resource_id],
    )?;
    Ok(n > 0)
}

/// Count all provenance records.
pub fn count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM provenance", [], |row| row.get(0))?)
}

fn row_to_provenance(row: &rusqlite::Row<'_>) -> rusqlite::Result<Provenance> {
    Ok(Provenance {
        resource_id: row.get(0)?,
        installation_method: row.get(1)?,
        installation_time: row.get(2)?,
        transaction_id: row.get(3)?,
        source_type: row.get(4)?,
        source_id: row.get(5)?,
        adopted_at: row.get(6)?,
    })
}
