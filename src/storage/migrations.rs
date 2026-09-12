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
