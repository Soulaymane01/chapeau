use crate::errors::Result;
use chrono::Utc;
use rusqlite::{params, Connection};

/// A record of one inventory scan run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRun {
    pub id: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub apps_found: usize,
    pub services_found: usize,
    pub custom_found: usize,
    pub error: Option<String>,
}

/// Start a scan run and return its ID.
pub fn start(conn: &Connection) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO scan_runs (started_at, status)
         VALUES (?1, 'running')",
        params![now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Finish a scan run.
pub fn finish(
    conn: &Connection,
    id: i64,
    status: &str,
    apps_found: usize,
    services_found: usize,
    custom_found: usize,
    error: Option<&str>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE scan_runs
         SET finished_at = ?2, status = ?3, apps_found = ?4, services_found = ?5,
             custom_found = ?6, error = ?7
         WHERE id = ?1",
        params![
            id,
            now,
            status,
            apps_found as i64,
            services_found as i64,
            custom_found as i64,
            error,
        ],
    )?;
    Ok(())
}

/// Get the most recent scan run.
pub fn latest(conn: &Connection) -> Result<Option<ScanRun>> {
    let mut stmt = conn.prepare(
        "SELECT id, started_at, finished_at, status, apps_found, services_found,
                custom_found, error
         FROM scan_runs ORDER BY id DESC LIMIT 1",
    )?;
    let mut rows = stmt.query_map([], row_to_scan_run)?;
    Ok(rows.next().transpose()?)
}

fn row_to_scan_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScanRun> {
    let apps: i64 = row.get(4)?;
    let services: i64 = row.get(5)?;
    let custom: i64 = row.get(6)?;
    Ok(ScanRun {
        id: row.get(0)?,
        started_at: row.get(1)?,
        finished_at: row.get(2)?,
        status: row.get(3)?,
        apps_found: usize::try_from(apps).unwrap_or(0),
        services_found: usize::try_from(services).unwrap_or(0),
        custom_found: usize::try_from(custom).unwrap_or(0),
        error: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;

    #[test]
    fn start_finish_and_read_latest() {
        let db = Database::open_memory().unwrap();
        let id = start(db.conn()).unwrap();
        finish(db.conn(), id, "ok", 12, 7, 2, None).unwrap();

        let run = latest(db.conn()).unwrap().unwrap();
        assert_eq!(run.id, id);
        assert_eq!(run.status, "ok");
        assert_eq!(run.apps_found, 12);
        assert_eq!(run.services_found, 7);
        assert_eq!(run.custom_found, 2);
        assert!(run.finished_at.is_some());
        assert!(run.error.is_none());
    }

    #[test]
    fn latest_returns_none_on_empty_database() {
        let db = Database::open_memory().unwrap();
        assert!(latest(db.conn()).unwrap().is_none());
    }

    #[test]
    fn failed_run_records_error() {
        let db = Database::open_memory().unwrap();
        let id = start(db.conn()).unwrap();
        finish(db.conn(), id, "failed", 0, 0, 0, Some("dnf failed")).unwrap();

        let run = latest(db.conn()).unwrap().unwrap();
        assert_eq!(run.status, "failed");
        assert_eq!(run.error.as_deref(), Some("dnf failed"));
    }
}
