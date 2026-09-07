use crate::errors::Result;
use chrono::Utc;
use rusqlite::{params, Connection};

/// A history entry as stored in the database.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub id: i64,
    pub timestamp: String,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub domain_id: Option<String>,
    pub details: Option<String>,
}

/// Insert a history entry.
pub fn insert(
    conn: &Connection,
    action: &str,
    resource_type: Option<&str>,
    resource_id: Option<&str>,
    domain_id: Option<&str>,
    details: Option<&str>,
) -> Result<HistoryEntry> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO history (timestamp, action, resource_type, resource_id, domain_id, details)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![now, action, resource_type, resource_id, domain_id, details],
    )?;
    let id: i64 = conn.last_insert_rowid();
    Ok(HistoryEntry {
        id,
        timestamp: now,
        action: action.to_string(),
        resource_type: resource_type.map(|s| s.to_string()),
        resource_id: resource_id.map(|s| s.to_string()),
        domain_id: domain_id.map(|s| s.to_string()),
        details: details.map(|s| s.to_string()),
    })
}

/// List recent history entries, newest first.
pub fn list(conn: &Connection, limit: i64) -> Result<Vec<HistoryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, action, resource_type, resource_id, domain_id, details
         FROM history ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], row_to_history)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// Count all history entries.
pub fn count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))?)
}

fn row_to_history(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryEntry> {
    Ok(HistoryEntry {
        id: row.get(0)?,
        timestamp: row.get(1)?,
        action: row.get(2)?,
        resource_type: row.get(3)?,
        resource_id: row.get(4)?,
        domain_id: row.get(5)?,
        details: row.get(6)?,
    })
}
