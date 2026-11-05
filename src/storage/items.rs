use crate::errors::{ChapeauError, Result};
use crate::model::{Item, ItemKind, ItemSource, Scope};
use chrono::Utc;
use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::str::FromStr;
use uuid::Uuid;

/// Fields required to create or update an inventory item.
#[derive(Debug, Clone)]
pub struct NewItem<'a> {
    pub kind: ItemKind,
    pub name: &'a str,
    pub source: ItemSource,
    pub source_key: &'a str,
    pub scope: Option<Scope>,
    pub version: Option<&'a str>,
    pub description: Option<&'a str>,
    pub icon: Option<&'a str>,
    pub command: Option<&'a str>,
    pub metadata: Option<&'a serde_json::Value>,
}

/// Insert an item or update the mutable facts for an existing source identity.
///
/// The identity is `(kind, source, scope, source_key)`. User choices such as
/// `pinned` and `hidden` are preserved across scans.
pub fn upsert(conn: &Connection, item: NewItem<'_>) -> Result<Item> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let scope = item.scope.map(Scope::as_str).unwrap_or("");
    let metadata = item
        .metadata
        .map(serde_json::Value::to_string)
        .unwrap_or_else(|| "{}".to_string());

    conn.execute(
        "INSERT INTO items
         (id, kind, name, source, source_key, scope, version, description, icon,
          command, installed, pinned, hidden, metadata, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, 0, 0, ?11, ?12, ?12)
         ON CONFLICT(kind, source, scope, source_key) DO UPDATE SET
             name = excluded.name,
             version = excluded.version,
             description = excluded.description,
             icon = excluded.icon,
             command = excluded.command,
             installed = 1,
             metadata = excluded.metadata,
             updated_at = excluded.updated_at",
        params![
            id,
            item.kind.as_str(),
            item.name,
            item.source.as_str(),
            item.source_key,
            scope,
            item.version,
            item.description,
            item.icon,
            item.command,
            metadata,
            now,
        ],
    )?;

    get_by_source(conn, item.kind, item.source, item.scope, item.source_key)?.ok_or_else(|| {
        ChapeauError::Validation(format!(
            "item '{}' could not be read after upsert",
            item.source_key
        ))
    })
}

/// Get an item by its Chapeau UUID.
pub fn get(conn: &Connection, id: &str) -> Result<Option<Item>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, source, source_key, scope, version, description,
                icon, command, installed, pinned, hidden, metadata, created_at,
                updated_at
         FROM items WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], row_to_item)?;
    Ok(rows.next().transpose()?)
}

/// Get an item by its source identity.
pub fn get_by_source(
    conn: &Connection,
    kind: ItemKind,
    source: ItemSource,
    scope: Option<Scope>,
    source_key: &str,
) -> Result<Option<Item>> {
    let scope = scope.map(Scope::as_str).unwrap_or("");
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, source, source_key, scope, version, description,
                icon, command, installed, pinned, hidden, metadata, created_at,
                updated_at
         FROM items
         WHERE kind = ?1 AND source = ?2 AND scope = ?3 AND source_key = ?4",
    )?;
    let mut rows = stmt.query_map(
        params![kind.as_str(), source.as_str(), scope, source_key],
        row_to_item,
    )?;
    Ok(rows.next().transpose()?)
}

/// List items, optionally filtered by kind, ordered by name.
pub fn list(conn: &Connection, kind: Option<ItemKind>) -> Result<Vec<Item>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, source, source_key, scope, version, description,
                icon, command, installed, pinned, hidden, metadata, created_at,
                updated_at
         FROM items
         WHERE (?1 IS NULL OR kind = ?1)
         ORDER BY name COLLATE NOCASE, kind, source",
    )?;
    let rows = stmt.query_map(params![kind.map(ItemKind::as_str)], row_to_item)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// Find items by exact, case-insensitive display name.
pub fn find_by_name(conn: &Connection, name: &str) -> Result<Vec<Item>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, source, source_key, scope, version, description,
                icon, command, installed, pinned, hidden, metadata, created_at,
                updated_at
         FROM items
         WHERE name = ?1 COLLATE NOCASE
         ORDER BY kind, source, name",
    )?;
    let rows = stmt.query_map(params![name], row_to_item)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// Find items by exact source key (desktop ID, app ID, or unit name).
pub fn find_by_source_key(conn: &Connection, source_key: &str) -> Result<Vec<Item>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, source, source_key, scope, version, description,
                icon, command, installed, pinned, hidden, metadata, created_at,
                updated_at
         FROM items
         WHERE source_key = ?1
         ORDER BY kind, source, name",
    )?;
    let rows = stmt.query_map(params![source_key], row_to_item)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// Set the pinned flag. Returns true when the item exists.
pub fn set_pinned(conn: &Connection, id: &str, pinned: bool) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let rows = conn.execute(
        "UPDATE items SET pinned = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, pinned as i32, now],
    )?;
    Ok(rows > 0)
}

/// Set the hidden flag. Returns true when the item exists.
pub fn set_hidden(conn: &Connection, id: &str, hidden: bool) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let rows = conn.execute(
        "UPDATE items SET hidden = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, hidden as i32, now],
    )?;
    Ok(rows > 0)
}

/// Delete an item and all cascading status/group rows.
pub fn delete(conn: &Connection, id: &str) -> Result<bool> {
    let rows = conn.execute("DELETE FROM items WHERE id = ?1", params![id])?;
    Ok(rows > 0)
}

/// Count all items.
pub fn count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM items", [], |row| row.get(0))?)
}

/// Mark items from one source identity as absent when they were not seen.
///
/// Only call this for a source that was completely scanned. User state
/// (`pinned`, `hidden`) and group memberships are preserved.
pub fn mark_missing(
    conn: &Connection,
    kind: ItemKind,
    source: ItemSource,
    scope: Option<Scope>,
    seen: &HashSet<String>,
) -> Result<usize> {
    let scope_str = scope.map(Scope::as_str).unwrap_or("");
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, source, source_key, scope, version, description,
                icon, command, installed, pinned, hidden, metadata, created_at,
                updated_at
         FROM items
         WHERE kind = ?1 AND source = ?2 AND scope = ?3",
    )?;
    let items = stmt
        .query_map(
            params![kind.as_str(), source.as_str(), scope_str],
            row_to_item,
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let now = Utc::now().to_rfc3339();
    let mut marked = 0;
    for item in items {
        if !seen.contains(&item.source_key) {
            let rows = conn.execute(
                "UPDATE items SET installed = 0, updated_at = ?2
                 WHERE id = ?1 AND installed = 1",
                params![item.id, now],
            )?;
            if rows > 0 {
                marked += 1;
            }
        }
    }
    Ok(marked)
}

/// Map a database row in item column order.
pub(crate) fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<Item> {
    let kind_str: String = row.get(1)?;
    let kind = ItemKind::from_str(&kind_str)
        .map_err(|_| rusqlite::Error::InvalidParameterName(kind_str.clone()))?;
    let source_str: String = row.get(3)?;
    let source = ItemSource::from_str(&source_str)
        .map_err(|_| rusqlite::Error::InvalidParameterName(source_str.clone()))?;
    let scope_str: String = row.get(5)?;
    let scope = if scope_str.is_empty() {
        None
    } else {
        Some(
            Scope::from_str(&scope_str)
                .map_err(|_| rusqlite::Error::InvalidParameterName(scope_str.clone()))?,
        )
    };
    let metadata_str: Option<String> = row.get(13)?;
    let metadata = metadata_str
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    Ok(Item {
        id: row.get(0)?,
        kind,
        name: row.get(2)?,
        source,
        source_key: row.get(4)?,
        scope,
        version: row.get(6)?,
        description: row.get(7)?,
        icon: row.get(8)?,
        command: row.get(9)?,
        installed: row.get::<_, i64>(10)? != 0,
        pinned: row.get::<_, i64>(11)? != 0,
        hidden: row.get::<_, i64>(12)? != 0,
        metadata,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    fn new_item<'a>(name: &'a str, source_key: &'a str) -> NewItem<'a> {
        NewItem {
            kind: ItemKind::App,
            name,
            source: ItemSource::Desktop,
            source_key,
            scope: None,
            version: Some("1.0"),
            description: Some("Test app"),
            icon: Some("test-app"),
            command: Some("/usr/bin/test-app"),
            metadata: None,
        }
    }

    #[test]
    fn upsert_creates_and_reads_item() {
        let db = temp_db();
        let item = upsert(db.conn(), new_item("Test App", "test.desktop")).unwrap();

        assert_eq!(item.name, "Test App");
        assert_eq!(item.kind, ItemKind::App);
        assert_eq!(item.source, ItemSource::Desktop);
        assert_eq!(item.source_key, "test.desktop");
        assert_eq!(item.version.as_deref(), Some("1.0"));
        assert!(item.installed);
        assert!(!item.pinned);
        assert!(!item.hidden);

        assert_eq!(get(db.conn(), &item.id).unwrap().unwrap(), item);
        assert_eq!(
            get_by_source(
                db.conn(),
                ItemKind::App,
                ItemSource::Desktop,
                None,
                "test.desktop"
            )
            .unwrap()
            .unwrap(),
            item
        );
    }

    #[test]
    fn upsert_is_idempotent_and_preserves_user_flags() {
        let db = temp_db();
        let first = upsert(db.conn(), new_item("Test App", "test.desktop")).unwrap();
        set_pinned(db.conn(), &first.id, true).unwrap();
        set_hidden(db.conn(), &first.id, true).unwrap();

        let mut update = new_item("Test App Renamed", "test.desktop");
        update.version = Some("2.0");
        let second = upsert(db.conn(), update).unwrap();

        assert_eq!(second.id, first.id);
        assert_eq!(second.name, "Test App Renamed");
        assert_eq!(second.version.as_deref(), Some("2.0"));
        assert!(second.pinned);
        assert!(second.hidden);
        assert_eq!(count(db.conn()).unwrap(), 1);
    }

    #[test]
    fn find_by_name_is_case_insensitive() {
        let db = temp_db();
        let item = upsert(db.conn(), new_item("Visual Studio Code", "code.desktop")).unwrap();

        let found = find_by_name(db.conn(), "visual studio code").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, item.id);
    }

    #[test]
    fn list_filters_by_kind() {
        let db = temp_db();
        upsert(db.conn(), new_item("App", "app.desktop")).unwrap();
        upsert(
            db.conn(),
            NewItem {
                kind: ItemKind::Service,
                name: "postgresql",
                source: ItemSource::Systemd,
                source_key: "postgresql.service",
                scope: Some(Scope::System),
                version: None,
                description: None,
                icon: None,
                command: None,
                metadata: None,
            },
        )
        .unwrap();

        assert_eq!(list(db.conn(), None).unwrap().len(), 2);
        assert_eq!(list(db.conn(), Some(ItemKind::App)).unwrap().len(), 1);
        assert_eq!(list(db.conn(), Some(ItemKind::Service)).unwrap().len(), 1);
    }

    #[test]
    fn mark_missing_only_affects_matching_source_and_scope() {
        let db = temp_db();
        let app = upsert(db.conn(), new_item("App", "app.desktop")).unwrap();
        let service = upsert(
            db.conn(),
            NewItem {
                kind: ItemKind::Service,
                name: "postgresql",
                source: ItemSource::Systemd,
                source_key: "postgresql.service",
                scope: Some(Scope::System),
                version: None,
                description: None,
                icon: None,
                command: None,
                metadata: None,
            },
        )
        .unwrap();

        let seen: HashSet<String> = HashSet::new();
        let marked =
            mark_missing(db.conn(), ItemKind::App, ItemSource::Desktop, None, &seen).unwrap();
        assert_eq!(marked, 1);
        assert!(!get(db.conn(), &app.id).unwrap().unwrap().installed);
        assert!(get(db.conn(), &service.id).unwrap().unwrap().installed);
    }

    #[test]
    fn deleting_an_item_cascades_status_and_memberships() {
        let db = temp_db();
        let item = upsert(db.conn(), new_item("App", "app.desktop")).unwrap();
        let group = crate::storage::groups::create(db.conn(), "IDEs", None).unwrap();
        crate::storage::groups::add_member(db.conn(), &group.id, &item.id, 0).unwrap();

        assert!(delete(db.conn(), &item.id).unwrap());
        assert!(get(db.conn(), &item.id).unwrap().is_none());
        assert!(crate::storage::groups::list_members(db.conn(), &group.id)
            .unwrap()
            .is_empty());
    }
}
