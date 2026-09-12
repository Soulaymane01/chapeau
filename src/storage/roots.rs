use crate::core::root::{Root, RootSource};
use crate::errors::Result;
use chrono::Utc;
use rusqlite::{params, Connection};

/// Create a new root.
pub fn create(
    conn: &Connection,
    resource_id: &str,
    source: RootSource,
    reason: Option<&str>,
) -> Result<Root> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO roots (resource_id, source, reason, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![resource_id, source.to_string(), reason, now, now],
    )?;
    Ok(Root {
        resource_id: resource_id.to_string(),
        source,
        reason: reason.map(|s| s.to_string()),
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Create a root within an existing transaction.
pub fn create_tx(
    conn: &Connection,
    resource_id: &str,
    source: RootSource,
    reason: Option<&str>,
) -> Result<Root> {
    create(conn, resource_id, source, reason)
}

/// Get a root by resource ID.
pub fn get(conn: &Connection, resource_id: &str) -> Result<Option<Root>> {
    let mut stmt = conn.prepare(
        "SELECT resource_id, source, reason, created_at, updated_at
         FROM roots WHERE resource_id = ?1",
    )?;
    let mut rows = stmt.query_map(params![resource_id], row_to_root)?;
    Ok(rows.next().transpose()?)
}

/// List all roots, ordered by creation time (newest first).
pub fn list(conn: &Connection) -> Result<Vec<Root>> {
    let mut stmt = conn.prepare(
        "SELECT resource_id, source, reason, created_at, updated_at
         FROM roots ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], row_to_root)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// Delete a root by resource ID. Returns true if a root was deleted.
pub fn delete(conn: &Connection, resource_id: &str) -> Result<bool> {
    let rows = conn.execute("DELETE FROM roots WHERE resource_id = ?1", [resource_id])?;
    Ok(rows > 0)
}

/// Check if a resource is a root.
pub fn is_root(conn: &Connection, resource_id: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM roots WHERE resource_id = ?1",
        [resource_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Count all roots.
pub fn count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM roots", [], |row| row.get(0))?)
}

/// Get all root resource IDs as a set for efficient lookup.
pub fn root_ids(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT resource_id FROM roots")?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(ids)
}

/// Update the root's source and reason.
pub fn update(
    conn: &Connection,
    resource_id: &str,
    source: RootSource,
    reason: Option<&str>,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let rows = conn.execute(
        "UPDATE roots SET source = ?1, reason = ?2, updated_at = ?3
         WHERE resource_id = ?4",
        params![source.to_string(), reason, now, resource_id],
    )?;
    Ok(rows > 0)
}

fn row_to_root(row: &rusqlite::Row<'_>) -> rusqlite::Result<Root> {
    let source_str: String = row.get(1)?;
    let source: RootSource = source_str
        .parse()
        .map_err(|_| rusqlite::Error::InvalidParameterName(source_str))?;
    Ok(Root {
        resource_id: row.get(0)?,
        source,
        reason: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{root::RootSource, ResourceType};
    use crate::storage::{resources, Database};

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    #[test]
    fn test_create_and_get_root() {
        let db = temp_db();
        let conn = db.conn();
        let res = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        let root = create(
            conn,
            &res.id,
            RootSource::User,
            Some("explicitly installed"),
        )
        .unwrap();
        assert_eq!(root.source, RootSource::User);
        assert_eq!(root.reason, Some("explicitly installed".to_string()));

        let fetched = get(conn, &res.id).unwrap().unwrap();
        assert_eq!(fetched.resource_id, res.id);
        assert_eq!(fetched.source, RootSource::User);
    }

    #[test]
    fn test_get_nonexistent_root() {
        let db = temp_db();
        let conn = db.conn();
        assert!(get(conn, "nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_list_roots() {
        let db = temp_db();
        let conn = db.conn();
        let res1 = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        let res2 =
            resources::create(conn, ResourceType::Package, "postgres", Some("PostgreSQL")).unwrap();
        create(conn, &res1.id, RootSource::User, None).unwrap();
        create(conn, &res2.id, RootSource::Detected, None).unwrap();

        let roots = list(conn).unwrap();
        assert_eq!(roots.len(), 2);
    }

    #[test]
    fn test_delete_root() {
        let db = temp_db();
        let conn = db.conn();
        let res = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        create(conn, &res.id, RootSource::User, None).unwrap();
        assert!(delete(conn, &res.id).unwrap());
        assert!(get(conn, &res.id).unwrap().is_none());
        // Deleting again returns false
        assert!(!delete(conn, &res.id).unwrap());
    }

    #[test]
    fn test_is_root() {
        let db = temp_db();
        let conn = db.conn();
        let res = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        assert!(!is_root(conn, &res.id).unwrap());
        create(conn, &res.id, RootSource::User, None).unwrap();
        assert!(is_root(conn, &res.id).unwrap());
    }

    #[test]
    fn test_count() {
        let db = temp_db();
        let conn = db.conn();
        assert_eq!(count(conn).unwrap(), 0);
        let res1 = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        let res2 =
            resources::create(conn, ResourceType::Package, "postgres", Some("PostgreSQL")).unwrap();
        create(conn, &res1.id, RootSource::User, None).unwrap();
        create(conn, &res2.id, RootSource::Detected, None).unwrap();
        assert_eq!(count(conn).unwrap(), 2);
    }

    #[test]
    fn test_root_ids() {
        let db = temp_db();
        let conn = db.conn();
        let res1 = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        let res2 =
            resources::create(conn, ResourceType::Package, "postgres", Some("PostgreSQL")).unwrap();
        create(conn, &res1.id, RootSource::User, None).unwrap();
        create(conn, &res2.id, RootSource::Detected, None).unwrap();

        let ids = root_ids(conn).unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&res1.id));
        assert!(ids.contains(&res2.id));
    }

    #[test]
    fn test_update_root() {
        let db = temp_db();
        let conn = db.conn();
        let res = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        create(conn, &res.id, RootSource::Detected, None).unwrap();
        assert!(update(conn, &res.id, RootSource::User, Some("now explicit")).unwrap());
        let root = get(conn, &res.id).unwrap().unwrap();
        assert_eq!(root.source, RootSource::User);
        assert_eq!(root.reason, Some("now explicit".to_string()));
    }

    #[test]
    fn test_root_cascade_on_resource_delete() {
        let db = temp_db();
        let conn = db.conn();
        let res = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        create(conn, &res.id, RootSource::User, None).unwrap();
        assert!(is_root(conn, &res.id).unwrap());
        // Delete the resource — root should cascade
        resources::delete(conn, &res.id).unwrap();
        assert!(!is_root(conn, &res.id).unwrap());
    }

    #[test]
    fn test_duplicate_root_fails() {
        let db = temp_db();
        let conn = db.conn();
        let res = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        create(conn, &res.id, RootSource::User, None).unwrap();
        // Second insert should fail (PRIMARY KEY)
        assert!(create(conn, &res.id, RootSource::Detected, None).is_err());
    }
}
