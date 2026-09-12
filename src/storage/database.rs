use super::migrations;
use crate::errors::Result;
use rusqlite::{Connection, Transaction};
use std::path::{Path, PathBuf};

/// Manages the SQLite connection and provides transaction support.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open (or create) the database at the given path.
    ///
    /// - Creates parent directories as needed
    /// - Enables WAL mode
    /// - Enables foreign keys
    /// - Runs all pending migrations
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;

        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.execute_batch("PRAGMA busy_timeout = 5000;")?;

        let db = Self { conn };
        migrations::run(&db.conn)?;
        Ok(db)
    }

    /// Open an in-memory database for testing.
    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;

        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        let db = Self { conn };
        migrations::run(&db.conn)?;
        Ok(db)
    }

    /// Return the default production database path (~/.local/state/chapeau/chapeau.db).
    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").expect("HOME not set");
        PathBuf::from(home).join(".local/state/chapeau/chapeau.db")
    }

    /// Get a reference to the underlying connection.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Execute a closure within a transaction.
    ///
    /// The transaction is committed if the closure returns Ok, rolled back on Err.
    pub fn transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Transaction<'_>) -> Result<T>,
    {
        let tx = self.conn.unchecked_transaction()?;
        match f(&tx) {
            Ok(val) => {
                tx.commit()?;
                Ok(val)
            }
            Err(e) => {
                tx.rollback()?;
                Err(e)
            }
        }
    }

    /// Return diagnostic info about the database.
    pub fn status(&self) -> Result<DatabaseStatus> {
        let db_path: String = self.conn.query_row(
            "SELECT file FROM pragma_database_list() WHERE name = 'main'",
            [],
            |row| row.get(0),
        )?;

        let wal_mode: String = self
            .conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))?;

        let fk_enabled: bool = self
            .conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;

        let resource_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM resources", [], |row| row.get(0))?;

        let relationship_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM relationships", [], |row| row.get(0))?;

        let domain_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM domains", [], |row| row.get(0))?;

        let root_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM roots", [], |row| row.get(0))?;

        Ok(DatabaseStatus {
            db_path,
            wal_mode,
            fk_enabled,
            resource_count,
            relationship_count,
            domain_count,
            root_count,
        })
    }
}

/// Diagnostic information about the database.
#[derive(Debug)]
pub struct DatabaseStatus {
    pub db_path: String,
    pub wal_mode: String,
    pub fk_enabled: bool,
    pub resource_count: i64,
    pub relationship_count: i64,
    pub domain_count: i64,
    pub root_count: i64,
}
