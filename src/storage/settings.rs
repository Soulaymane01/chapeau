use crate::errors::Result;
use chrono::Utc;
use rusqlite::{params, Connection};

/// Read a setting value.
pub fn get(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
    Ok(rows.next().transpose()?)
}

/// Write or replace a setting value.
pub fn set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO settings (key, value, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET
             value = excluded.value,
             updated_at = excluded.updated_at",
        params![key, value, now],
    )?;
    Ok(())
}

/// Delete a setting. Returns true when it existed.
pub fn delete(conn: &Connection, key: &str) -> Result<bool> {
    let rows = conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
    Ok(rows > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;

    #[test]
    fn set_get_and_replace() {
        let db = Database::open_memory().unwrap();
        assert!(get(db.conn(), "theme").unwrap().is_none());

        set(db.conn(), "theme", "dark").unwrap();
        assert_eq!(get(db.conn(), "theme").unwrap().as_deref(), Some("dark"));

        set(db.conn(), "theme", "light").unwrap();
        assert_eq!(get(db.conn(), "theme").unwrap().as_deref(), Some("light"));
    }

    #[test]
    fn delete_setting() {
        let db = Database::open_memory().unwrap();
        set(db.conn(), "theme", "dark").unwrap();
        assert!(delete(db.conn(), "theme").unwrap());
        assert!(get(db.conn(), "theme").unwrap().is_none());
        assert!(!delete(db.conn(), "theme").unwrap());
    }
}
