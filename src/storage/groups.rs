use crate::errors::Result;
use crate::model::{Group, Item};
use crate::storage::items;
use chrono::Utc;
use rusqlite::{params, Connection};

/// Create a group. Names are unique case-insensitively.
pub fn create(conn: &Connection, name: &str, description: Option<&str>) -> Result<Group> {
    let mut group = Group::new(name, description.map(str::to_string));
    group.validate()?;
    let now = Utc::now().to_rfc3339();
    group.created_at = now.clone();
    group.updated_at = now;

    conn.execute(
        "INSERT INTO groups (id, name, description, icon, color, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            group.id,
            group.name,
            group.description,
            group.icon,
            group.color,
            group.sort_order,
            group.created_at,
            group.updated_at,
        ],
    )?;

    Ok(group)
}

/// Get a group by ID.
pub fn get(conn: &Connection, id: &str) -> Result<Option<Group>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, icon, color, sort_order, created_at, updated_at
         FROM groups WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], row_to_group)?;
    Ok(rows.next().transpose()?)
}

/// Get a group by name (case-insensitive).
pub fn get_by_name(conn: &Connection, name: &str) -> Result<Option<Group>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, icon, color, sort_order, created_at, updated_at
         FROM groups WHERE name = ?1 COLLATE NOCASE",
    )?;
    let mut rows = stmt.query_map(params![name], row_to_group)?;
    Ok(rows.next().transpose()?)
}

/// List groups in display order.
pub fn list(conn: &Connection) -> Result<Vec<Group>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, icon, color, sort_order, created_at, updated_at
         FROM groups ORDER BY sort_order, name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], row_to_group)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// Update a group's name and/or description.
///
/// `None` for `name` keeps the current name; `None` for `description` clears
/// the description, matching the existing domain storage convention.
pub fn update(
    conn: &Connection,
    id: &str,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<bool> {
    if let Some(name) = name {
        if name.trim().is_empty() {
            return Err(crate::errors::ChapeauError::Validation(
                "group name must not be empty".into(),
            ));
        }
    }
    let now = Utc::now().to_rfc3339();
    let rows = conn.execute(
        "UPDATE groups
         SET name = COALESCE(?2, name), description = ?3, updated_at = ?4
         WHERE id = ?1",
        params![id, name, description, now],
    )?;
    Ok(rows > 0)
}

/// Set the group's ordering hint.
pub fn set_sort_order(conn: &Connection, id: &str, sort_order: i64) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let rows = conn.execute(
        "UPDATE groups SET sort_order = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, sort_order, now],
    )?;
    Ok(rows > 0)
}

/// Delete a group. Memberships cascade; items are preserved.
pub fn delete(conn: &Connection, id: &str) -> Result<bool> {
    let rows = conn.execute("DELETE FROM groups WHERE id = ?1", params![id])?;
    Ok(rows > 0)
}

/// Add an item to a group, updating its order when it is already a member.
pub fn add_member(conn: &Connection, group_id: &str, item_id: &str, sort_order: i64) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO group_items (group_id, item_id, sort_order, created_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(group_id, item_id)
         DO UPDATE SET sort_order = excluded.sort_order",
        params![group_id, item_id, sort_order, now],
    )?;
    Ok(())
}

/// Remove an item from a group. Returns true when a membership existed.
pub fn remove_member(conn: &Connection, group_id: &str, item_id: &str) -> Result<bool> {
    let rows = conn.execute(
        "DELETE FROM group_items WHERE group_id = ?1 AND item_id = ?2",
        params![group_id, item_id],
    )?;
    Ok(rows > 0)
}

/// List a group's members with their ordering.
pub fn list_members(conn: &Connection, group_id: &str) -> Result<Vec<(Item, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT i.id, i.kind, i.name, i.source, i.source_key, i.scope, i.version,
                i.description, i.icon, i.command, i.installed, i.pinned, i.hidden,
                i.metadata, i.created_at, i.updated_at, gi.sort_order
         FROM group_items gi
         JOIN items i ON i.id = gi.item_id
         WHERE gi.group_id = ?1
         ORDER BY gi.sort_order, i.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map(params![group_id], |row| {
        let item = items::row_to_item(row)?;
        let sort_order: i64 = row.get(16)?;
        Ok((item, sort_order))
    })?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// List the groups an item belongs to, ordered by group display order.
pub fn list_for_item(conn: &Connection, item_id: &str) -> Result<Vec<Group>> {
    let mut stmt = conn.prepare(
        "SELECT g.id, g.name, g.description, g.icon, g.color, g.sort_order,
                g.created_at, g.updated_at
         FROM group_items gi
         JOIN groups g ON g.id = gi.group_id
         WHERE gi.item_id = ?1
         ORDER BY g.sort_order, g.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map(params![item_id], row_to_group)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// Set a group's member order from an explicit list of item IDs.
pub fn reorder(conn: &Connection, group_id: &str, item_ids: &[String]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for (index, item_id) in item_ids.iter().enumerate() {
        tx.execute(
            "UPDATE group_items SET sort_order = ?3
             WHERE group_id = ?1 AND item_id = ?2",
            params![group_id, item_id, index as i64],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn row_to_group(row: &rusqlite::Row<'_>) -> rusqlite::Result<Group> {
    Ok(Group {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        icon: row.get(3)?,
        color: row.get(4)?,
        sort_order: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ItemKind, ItemSource, Scope};
    use crate::storage::Database;

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    fn item(db: &Database, name: &str, key: &str) -> Item {
        items::upsert(
            db.conn(),
            items::NewItem {
                kind: ItemKind::App,
                name,
                source: ItemSource::Desktop,
                source_key: key,
                scope: None,
                version: None,
                description: None,
                icon: None,
                command: None,
                metadata: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn create_get_and_list_groups() {
        let db = temp_db();
        let b = create(db.conn(), "Databases", Some("DB servers")).unwrap();
        let _a = create(db.conn(), "IDEs", None).unwrap();

        assert_eq!(get(db.conn(), &b.id).unwrap().unwrap(), b);
        assert_eq!(
            get_by_name(db.conn(), "databases").unwrap().unwrap().id,
            b.id
        );
        assert_eq!(list(db.conn()).unwrap().len(), 2);
        assert_eq!(list(db.conn()).unwrap()[0].id, b.id);
    }

    #[test]
    fn duplicate_names_are_rejected_case_insensitively() {
        let db = temp_db();
        create(db.conn(), "IDEs", None).unwrap();
        assert!(create(db.conn(), "ides", None).is_err());
    }

    #[test]
    fn membership_and_ordering_work() {
        let db = temp_db();
        let group = create(db.conn(), "Databases", None).unwrap();
        let postgres = item(&db, "PostgreSQL", "postgresql.service");
        let mariadb = item(&db, "MariaDB", "mariadb.service");

        add_member(db.conn(), &group.id, &postgres.id, 1).unwrap();
        add_member(db.conn(), &group.id, &mariadb.id, 0).unwrap();

        let members = list_members(db.conn(), &group.id).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].0.id, mariadb.id);
        assert_eq!(members[1].0.id, postgres.id);
        assert_eq!(members[0].1, 0);

        reorder(
            db.conn(),
            &group.id,
            &[postgres.id.clone(), mariadb.id.clone()],
        )
        .unwrap();
        let members = list_members(db.conn(), &group.id).unwrap();
        assert_eq!(members[0].0.id, postgres.id);
        assert_eq!(members[0].1, 0);
    }

    #[test]
    fn removing_and_deleting_preserves_items() {
        let db = temp_db();
        let group = create(db.conn(), "IDEs", None).unwrap();
        let code = item(&db, "VS Code", "code.desktop");
        add_member(db.conn(), &group.id, &code.id, 0).unwrap();

        assert!(remove_member(db.conn(), &group.id, &code.id).unwrap());
        assert!(list_members(db.conn(), &group.id).unwrap().is_empty());
        assert!(items::get(db.conn(), &code.id).unwrap().is_some());

        add_member(db.conn(), &group.id, &code.id, 0).unwrap();
        assert!(delete(db.conn(), &group.id).unwrap());
        assert!(list_members(db.conn(), &group.id).unwrap().is_empty());
        assert!(items::get(db.conn(), &code.id).unwrap().is_some());
    }

    #[test]
    fn list_for_item_returns_groups() {
        let db = temp_db();
        let ides = create(db.conn(), "IDEs", None).unwrap();
        let tools = create(db.conn(), "Tools", None).unwrap();
        let code = item(&db, "VS Code", "code.desktop");
        add_member(db.conn(), &ides.id, &code.id, 0).unwrap();
        add_member(db.conn(), &tools.id, &code.id, 0).unwrap();

        let groups = list_for_item(db.conn(), &code.id).unwrap();
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn service_scope_is_part_of_identity() {
        let db = temp_db();
        let system = items::upsert(
            db.conn(),
            items::NewItem {
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
        let user = items::upsert(
            db.conn(),
            items::NewItem {
                kind: ItemKind::Service,
                name: "postgresql",
                source: ItemSource::Systemd,
                source_key: "postgresql.service",
                scope: Some(Scope::User),
                version: None,
                description: None,
                icon: None,
                command: None,
                metadata: None,
            },
        )
        .unwrap();

        assert_ne!(system.id, user.id);
        assert_eq!(items::count(db.conn()).unwrap(), 2);
    }
}
