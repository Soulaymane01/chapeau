use crate::errors::Result;
use rusqlite::Connection;

/// Run all pending migrations, versioned.
pub fn run(conn: &Connection) -> Result<()> {
    // Create the migration tracking table if it doesn't exist.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )?;

    let current: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM _migrations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if current < 1 {
        // Check if the v1 tables already exist (legacy Phase 1 database).
        let tables_exist = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='resources'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;

        if tables_exist {
            // Legacy database: tables exist but no migration record. Just record it.
            conn.execute(
                "INSERT INTO _migrations (version, applied_at) VALUES (1, datetime('now'))",
                [],
            )?;
        } else {
            migration_v1(conn)?;
        }
    }

    // Future migrations go here:
    if current < 2 {
        migration_v2(conn)?;
    }

    if current < 3 {
        migration_v3(conn)?;
    }

    if current < 4 {
        migration_v4(conn)?;
    }

    Ok(())
}

/// Migration v1: initial schema.
fn migration_v1(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE domains (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL UNIQUE,
            description TEXT,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );

        CREATE TABLE resources (
            id              TEXT PRIMARY KEY,
            type            TEXT NOT NULL,
            native_id       TEXT NOT NULL,
            display_name    TEXT,
            created_at      TEXT NOT NULL,
            updated_at      TEXT NOT NULL,
            UNIQUE(type, native_id)
        );

        CREATE TABLE domain_resources (
            domain_id       TEXT NOT NULL,
            resource_id     TEXT NOT NULL,
            relationship    TEXT NOT NULL,
            reason          TEXT,
            created_at      TEXT NOT NULL,
            updated_at      TEXT NOT NULL,
            PRIMARY KEY(domain_id, resource_id, relationship),
            FOREIGN KEY(domain_id)   REFERENCES domains(id)    ON DELETE CASCADE,
            FOREIGN KEY(resource_id) REFERENCES resources(id)  ON DELETE CASCADE
        );

        CREATE TABLE relationships (
            id              TEXT PRIMARY KEY,
            source_id       TEXT NOT NULL,
            relationship    TEXT NOT NULL,
            target_id       TEXT NOT NULL,
            origin          TEXT NOT NULL,
            created_at      TEXT NOT NULL,
            updated_at      TEXT NOT NULL,
            FOREIGN KEY(source_id) REFERENCES resources(id) ON DELETE CASCADE,
            FOREIGN KEY(target_id) REFERENCES resources(id) ON DELETE CASCADE,
            UNIQUE(source_id, relationship, target_id)
        );

        CREATE TABLE provenance (
            resource_id          TEXT PRIMARY KEY,
            installation_method  TEXT,
            installation_time    TEXT,
            transaction_id       TEXT,
            source_type          TEXT,
            source_id            TEXT,
            adopted_at           TEXT,
            FOREIGN KEY(resource_id) REFERENCES resources(id) ON DELETE CASCADE
        );

        CREATE TABLE resource_observations (
            resource_id     TEXT PRIMARY KEY,
            observed_at     TEXT NOT NULL,
            installed       INTEGER,
            version         TEXT,
            active          INTEGER,
            enabled         INTEGER,
            failed          INTEGER,
            metadata        TEXT,
            FOREIGN KEY(resource_id) REFERENCES resources(id) ON DELETE CASCADE
        );

        CREATE TABLE history (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp       TEXT NOT NULL,
            action          TEXT NOT NULL,
            resource_type   TEXT,
            resource_id     TEXT,
            domain_id       TEXT,
            details         TEXT
        );

        INSERT INTO _migrations (version, applied_at)
        VALUES (1, datetime('now'));",
    )?;

    Ok(())
}

/// Migration v2: Reset service identities.
///
/// Phase 13 changed systemd unit native_id from the stripped base name
/// (e.g. "bluetooth") to the full unit name (e.g. "bluetooth.service").
/// This migration deletes all Service resources so the next scan recreates
/// them with correct full-name identities. FK cascades clean up observations,
/// relationships, domain_resources, and provenance automatically.
///
/// Service data is rebuildable — the next `chapeau scan` recreates everything.
fn migration_v2(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "DELETE FROM resources WHERE type = 'service';

         INSERT INTO _migrations (version, applied_at)
         VALUES (2, datetime('now'));",
    )?;

    Ok(())
}

/// Migration v3: Add roots table for intentional resources.
///
/// A "root" is a resource that Chapeau considers intentionally present
/// from the user's perspective. This is distinct from:
/// - provenance (how something was installed)
/// - domain ownership (logical grouping)
/// - dependency relationships (what depends on what)
///
/// Roots survive scan reconciliation and are never erased by system observation.
fn migration_v3(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE roots (
            resource_id TEXT PRIMARY KEY,
            source      TEXT NOT NULL,
            reason      TEXT,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL,
            FOREIGN KEY(resource_id) REFERENCES resources(id) ON DELETE CASCADE
        );

        INSERT INTO _migrations (version, applied_at)
        VALUES (3, datetime('now'));",
    )?;

    Ok(())
}

/// Migration v4: inventory model.
///
/// Replaces the semantic resource/relationship model with the product model:
/// apps, services, groups, runtime status, and ports. The legacy tables are
/// left intact so a user can roll back to the archived release; migration v5
/// removes them after the new path is validated.
fn migration_v4(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE items (
            id          TEXT PRIMARY KEY,
            kind        TEXT NOT NULL,
            name        TEXT NOT NULL,
            source      TEXT NOT NULL,
            source_key  TEXT NOT NULL,
            scope       TEXT NOT NULL DEFAULT '',
            version     TEXT,
            description TEXT,
            icon        TEXT,
            command     TEXT,
            installed   INTEGER NOT NULL DEFAULT 1,
            pinned      INTEGER NOT NULL DEFAULT 0,
            hidden      INTEGER NOT NULL DEFAULT 0,
            metadata    TEXT,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL,
            UNIQUE(kind, source, scope, source_key)
        );

        CREATE INDEX idx_items_kind ON items(kind);
        CREATE INDEX idx_items_name ON items(name COLLATE NOCASE);
        CREATE INDEX idx_items_source_key ON items(source_key);

        CREATE TABLE groups (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL UNIQUE,
            description TEXT,
            icon        TEXT,
            color       TEXT,
            sort_order  INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );

        CREATE TABLE group_items (
            group_id   TEXT NOT NULL,
            item_id    TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            PRIMARY KEY(group_id, item_id),
            FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE,
            FOREIGN KEY(item_id) REFERENCES items(id) ON DELETE CASCADE
        );

        CREATE INDEX idx_group_items_item ON group_items(item_id);

        CREATE TABLE app_status (
            item_id      TEXT PRIMARY KEY,
            desktop_file TEXT,
            exec         TEXT,
            categories   TEXT,
            package_name TEXT,
            origin       TEXT,
            FOREIGN KEY(item_id) REFERENCES items(id) ON DELETE CASCADE
        );

        CREATE TABLE service_status (
            item_id      TEXT PRIMARY KEY,
            runtime      TEXT NOT NULL,
            state        TEXT NOT NULL,
            sub_state    TEXT,
            enabled      INTEGER,
            main_pid     INTEGER,
            since        TEXT,
            memory_bytes INTEGER,
            unit_path    TEXT,
            working_dir  TEXT,
            last_checked TEXT NOT NULL,
            last_error   TEXT,
            FOREIGN KEY(item_id) REFERENCES items(id) ON DELETE CASCADE
        );

        CREATE INDEX idx_service_status_state ON service_status(state);

        CREATE TABLE service_ports (
            item_id      TEXT NOT NULL,
            protocol     TEXT NOT NULL,
            address      TEXT NOT NULL,
            port         INTEGER NOT NULL,
            pid          INTEGER,
            process_name TEXT,
            source       TEXT NOT NULL,
            last_seen    TEXT NOT NULL,
            PRIMARY KEY(item_id, protocol, address, port),
            FOREIGN KEY(item_id) REFERENCES items(id) ON DELETE CASCADE
        );

        CREATE INDEX idx_service_ports_port ON service_ports(port);

        CREATE TABLE custom_service_definitions (
            item_id        TEXT PRIMARY KEY,
            start_command  TEXT NOT NULL,
            stop_command   TEXT,
            status_command TEXT,
            working_dir    TEXT,
            env            TEXT,
            health_port    INTEGER,
            health_url     TEXT,
            log_path       TEXT,
            start_order    INTEGER NOT NULL DEFAULT 0,
            enabled        INTEGER NOT NULL DEFAULT 1,
            FOREIGN KEY(item_id) REFERENCES items(id) ON DELETE CASCADE
        );

        CREATE TABLE settings (
            key        TEXT PRIMARY KEY,
            value      TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE scan_runs (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            started_at     TEXT NOT NULL,
            finished_at    TEXT,
            status         TEXT NOT NULL,
            apps_found     INTEGER NOT NULL DEFAULT 0,
            services_found INTEGER NOT NULL DEFAULT 0,
            custom_found   INTEGER NOT NULL DEFAULT 0,
            error          TEXT
        );

        INSERT INTO _migrations (version, applied_at)
        VALUES (4, datetime('now'));",
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;

    fn table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
            > 0
    }

    #[test]
    fn fresh_database_has_inventory_schema() {
        let db = Database::open_memory().unwrap();
        for table in [
            "items",
            "groups",
            "group_items",
            "app_status",
            "service_status",
            "service_ports",
            "custom_service_definitions",
            "settings",
            "scan_runs",
        ] {
            assert!(table_exists(db.conn(), table), "missing table {table}");
        }

        let version: i64 = db
            .conn()
            .query_row("SELECT MAX(version) FROM _migrations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 4);
    }

    #[test]
    fn v3_database_migrates_to_v4() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE _migrations (
                 version INTEGER PRIMARY KEY,
                 applied_at TEXT NOT NULL
             );",
        )
        .unwrap();
        migration_v1(&conn).unwrap();
        migration_v2(&conn).unwrap();
        migration_v3(&conn).unwrap();

        conn.execute(
            "INSERT INTO domains (id, name, description, created_at, updated_at)
             VALUES ('d1', 'databases', NULL, datetime('now'), datetime('now'))",
            [],
        )
        .unwrap();

        run(&conn).unwrap();

        assert!(table_exists(&conn, "items"));
        assert!(table_exists(&conn, "groups"));
        assert!(table_exists(&conn, "domains"));

        let version: i64 = conn
            .query_row("SELECT MAX(version) FROM _migrations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 4);

        let domain_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM domains", [], |row| row.get(0))
            .unwrap();
        assert_eq!(domain_count, 1);
    }

    #[test]
    fn items_are_unique_by_source_identity() {
        let db = Database::open_memory().unwrap();
        let insert = |id: &str| {
            db.conn()
                .execute(
                    "INSERT INTO items
                     (id, kind, name, source, source_key, scope, created_at, updated_at)
                     VALUES (?1, 'app', 'Code', 'desktop', 'code.desktop', '', datetime('now'), datetime('now'))",
                    [id],
                )
                .unwrap()
        };

        insert("i1");
        assert!(db
            .conn()
            .execute(
                "INSERT INTO items
                 (id, kind, name, source, source_key, scope, created_at, updated_at)
                 VALUES ('i2', 'app', 'Code', 'desktop', 'code.desktop', '', datetime('now'), datetime('now'))",
                [],
            )
            .is_err());
    }
}
