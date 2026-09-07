# Chapeau Database

## Location

```
~/.local/state/chapeau/chapeau.db
```

The directory is created automatically on first use. An in-memory database (`:memory:`) is used for testing.

## Engine

SQLite via the `rusqlite` crate with the following PRAGMA settings:

| PRAGMA | Value | Purpose |
|--------|-------|---------|
| `journal_mode` | `WAL` | Write-Ahead Logging for safe concurrent reads |
| `foreign_keys` | `ON` | Enforces referential integrity |
| `busy_timeout` | `5000` | Waits up to 5 seconds on locked databases |

## Schema

All tables are created by a single migration (`migration_v1`).

### domains

```sql
CREATE TABLE domains (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
```

Logical groupings created by the user. Unique on `name`.

### resources

```sql
CREATE TABLE resources (
    id              TEXT PRIMARY KEY,
    type            TEXT NOT NULL,
    native_id       TEXT NOT NULL,
    display_name    TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    UNIQUE(type, native_id)
);
```

Everything Chapeau tracks. Unique on `(type, native_id)`.

### domain_resources

```sql
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
```

Associates domains with resources. Primary key is the composite `(domain_id, resource_id, relationship)`. Cascade deletes when either the domain or resource is removed.

### relationships

```sql
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
```

Directed edges between resources. Both `source_id` and `target_id` must reference valid resource IDs. Unique on `(source_id, relationship, target_id)`. Cascade deletes when either resource is removed.

### provenance

```sql
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
```

How a resource was installed. One-to-one with resources. Cascade deletes when the resource is removed.

### resource_observations

```sql
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
```

Cached state observations from the live system. One-to-one with resources. Updated on each scan. Cascade deletes when the resource is removed.

### history

```sql
CREATE TABLE history (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp       TEXT NOT NULL,
    action          TEXT NOT NULL,
    resource_type   TEXT,
    resource_id     TEXT,
    domain_id       TEXT,
    details         TEXT
);
```

Audit log of Chapeau operations. No foreign keys — history entries are not cascade-deleted when resources are removed.

### _migrations

```sql
CREATE TABLE IF NOT EXISTS _migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);
```

Tracks which migrations have been applied.

## Foreign key cascades

When a resource is deleted, the following are automatically cascade-deleted:

- All relationships where it appears as source or target
- Its provenance record
- Its observation record
- All domain_resource entries involving it

When a domain is deleted, all domain_resource entries for that domain are cascade-deleted.

**Note:** History entries are NOT cascade-deleted. They persist as an audit trail.

## Inspecting the database

### Using Chapeau

```bash
chapeau db status
```

Shows database path, WAL mode, foreign key status, and entity counts.

### Using SQLite directly

Read-only inspection:

```bash
sqlite3 ~/.local/state/chapeau/chapeau.db ".tables"
sqlite3 ~/.local/state/chapeau/chapeau.db "SELECT COUNT(*) FROM resources;"
sqlite3 ~/.local/state/chapeau/chapeau.db "SELECT type, COUNT(*) FROM resources GROUP BY type;"
```

**Warning:** Manually modifying the database is unsupported and may break Chapeau. The database is an internal implementation detail.

## Transactions

Chapeau uses SQLite transactions for multi-step operations. The `Database::transaction()` method executes a closure within a transaction, committing on success and rolling back on error.

Key transaction boundaries:

- `scan` — All resource creation, observation updates, and relationship creation happen in a single transaction
- `reconcile_after_removal` — Stale package deletion and associated data cleanup happen in a single transaction
