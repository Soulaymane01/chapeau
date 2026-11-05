# Chapeau Inventory Manager Rework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Chapeau's semantic dependency-graph model with a local inventory and control panel for Fedora apps and services: discover what exists, organize it into groups, show useful runtime facts (ports, state, paths), and let the user open apps and start/stop services.

**Architecture:** Keep the proven system integration (DNF5, systemd D-Bus, Flatpak, SQLite, GTK worker). Replace the domain layer with four explicit concepts: `Item` (app or service), `Group`, `ServiceStatus`/`ServicePort`, and `CustomServiceDefinition`. The new scan reads desktop entries, Flatpak apps, systemd units, package metadata, and user-defined custom services; it does not build a dependency graph.

**Tech Stack:** Rust 2021, `clap`, `rusqlite` (bundled SQLite), `serde`/`serde_json`, `zbus`/`zbus_systemd`, `tokio`, `chrono`, `uuid`; GTK4 + libadwaita for `chapeau-gui`.

## Global Constraints

- Fedora-only backends: DNF5, systemd, Flatpak.
- No dependency graph, roots/intentional-resource classification, orphaned/unused analysis, or removal planner in the new core.
- No uninstall/package-removal UI or command in v1. The legacy removal code may remain dormant until a later feature.
- Apps default to desktop entries + Flatpak applications + manual entries. RPM package inventory is a secondary "All software" view, not the default app list.
- Services default to meaningful systemd units (pinned, grouped, provided by user-installed packages, listening, or failed). "All services" is an explicit opt-in.
- Custom services run **as the current user only**. Never run custom commands through `sudo`/`pkexec`.
- The GTK main thread must never touch SQLite. All database work goes through the existing worker.
- Go through a scan only; no background daemon.
- Each phase must end with `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test`. GUI phases additionally run `cargo clippy -p chapeau-gui --all-targets -- -D warnings` and `cargo build -p chapeau-gui`.
- Commit after every task. Keep the build green on the `rework/inventory-v2` branch.
- Legacy implementation is archived at tag `legacy-v0.1.0-semantic`; do not force-delete that tag.

---

## Phase 0 — Preparation and Legacy Archive (COMPLETE)

**Status:** Completed 2026-11-05.

**Actions taken:**

- Created annotated tag `legacy-v0.1.0-semantic` at commit `582688e` (the last legacy implementation).
- Created branch `rework/inventory-v2` for all rework work.
- Created a portable git bundle:
  `~/.local/state/chapeau/backups/chapeau-legacy-20261105-162905.bundle`
- Created a WAL-aware SQLite backup:
  `~/.local/state/chapeau/backups/chapeau-pre-rework-20261105-162905.db`
- Inspected the legacy database: 4,154 resources, 21,513 relationships, 63 detected roots, 0 domains, 0 domain memberships. There is no user-created grouping data to migrate.
- Wrote this plan.

**Rollback:**

```bash
git switch master
git reset --hard legacy-v0.1.0-semantic
# Database restore if ever needed:
cp ~/.local/state/chapeau/backups/chapeau-pre-rework-20261105-162905.db \
   ~/.local/state/chapeau/chapeau.db
rm -f ~/.local/state/chapeau/chapeau.db-wal \
      ~/.local/state/chapeau/chapeau.db-shm
```

**Deliverable:** Legacy state is recoverable, a clean rework branch exists, and no old code has been deleted yet.

---

## Target Product Contract

### Objects

```text
App       launchable software: VS Code, IntelliJ, Antigravity, Firefox
Service   controllable runtime: PostgreSQL, MariaDB, Redis, nginx, custom dev server
Group     user-defined collection: IDEs, Databases, Web, Tools
```

### App sources

1. Desktop entries from XDG application directories.
2. Flatpak applications.
3. Manual entries created by the user.
4. Optional RPM package inventory behind `--all` / an "All software" view.

### Service sources

1. systemd system units.
2. systemd user units.
3. Custom command services defined by the user.

### Actions

```text
App:      open
Service:  start, stop, restart, enable, disable
Group:    start all, stop all, restart all (services only, in order)
```

### Explicit non-goals for v1

- Dependency resolution or dependency graphs.
- Package removal/uninstall.
- Automatic cleanup/orphan analysis.
- Background monitoring daemon.
- Root/package classification policy.

---

## File Map

### New model

```text
src/model/mod.rs
src/model/item.rs          Item, ItemKind, ItemSource, Scope
src/model/group.rs         Group, GroupMember
src/model/status.rs        AppStatus, ServiceStatus, ServiceState, RuntimeKind
src/model/port.rs          ServicePort, PortProtocol
src/model/service.rs       CustomServiceDefinition
```

### New storage

```text
src/storage/items.rs
src/storage/groups.rs
src/storage/status.rs
src/storage/ports.rs
src/storage/custom_services.rs
src/storage/scan_runs.rs
src/storage/settings.rs
```

### New discovery / scan

```text
src/discovery/apps.rs
src/discovery/software.rs
src/discovery/service_details.rs
src/discovery/ports.rs
src/reconciliation/inventory.rs   (new scan orchestration)
```

### New application layer

```text
src/services/catalog.rs
src/services/groups.rs
src/services/control.rs
src/services/launch.rs
src/services/ports.rs
src/services/details.rs
```

### New CLI

```text
src/cli/apps.rs
src/cli/services.rs
src/cli/groups.rs
src/cli/actions.rs
src/cli/add.rs
src/cli/info.rs
```

### GUI changes

```text
gui/src/service.rs                new Requests/Responses
gui/src/ui/dashboard.rs
gui/src/ui/apps.rs
gui/src/ui/services.rs
gui/src/ui/groups.rs
gui/src/ui/item_detail.rs
gui/src/ui/settings.rs
gui/src/main.rs                   new navigation wiring
```

### Legacy code removed in Phase 6

```text
src/core/root_policy.rs
src/core/analysis.rs
src/core/planner.rs
src/core/graph.rs
src/core/explanation.rs
src/core/provenance.rs
src/storage/provenance.rs
src/services/analysis.rs
src/services/graph.rs
src/services/removal.rs          (unless retained dormant)
src/services/overview.rs
src/services/explore.rs
src/services/roots.rs
src/services/status.rs           (replaced by new status/dashboard)
src/cli/why.rs
src/cli/dependencies.rs
src/cli/dependents.rs
src/cli/graph.rs
src/cli/orphaned.rs
src/cli/unused.rs
src/cli/roots.rs
src/cli/remove.rs
src/cli/overview.rs
src/cli/show.rs                  (replaced by info)
src/cli/drift.rs                 (or keep only as a small scan summary)
gui/src/ui/analysis.rs
gui/src/ui/drift.rs
gui/src/ui/domains.rs
gui/src/ui/explore.rs
gui/src/ui/flatpaks.rs
gui/src/ui/home.rs
gui/src/ui/status.rs
gui/src/ui/graph.rs
gui/src/ui/sidebar.rs            (rewrite)
```

---

## Phase 1 — New Model, Schema, and Storage

**Goal:** Add the new inventory model alongside the legacy code without switching any frontend yet. The build stays green.

### Task 1.1: Add model types

**Files:**
- Create: `src/model/mod.rs`, `src/model/item.rs`, `src/model/group.rs`, `src/model/status.rs`, `src/model/port.rs`, `src/model/service.rs`
- Modify: `src/lib.rs`
- Test: unit tests in each model file

**Interfaces:**

```rust
pub enum ItemKind { App, Service }
pub enum ItemSource { Desktop, Flatpak, Systemd, Manual, Custom }
pub enum Scope { System, User }
pub enum PortProtocol { Tcp, Udp }
pub enum RuntimeKind { Systemd, Custom }
pub enum ServiceState { Running, Stopped, Failed, Unknown }

pub struct Item {
    pub id: String,
    pub kind: ItemKind,
    pub name: String,
    pub source: ItemSource,
    pub source_key: String,
    pub scope: Option<Scope>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub command: Option<String>,
    pub installed: bool,
    pub pinned: bool,
    pub hidden: bool,
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

pub struct Group { /* id, name, description, icon, color, sort_order, timestamps */ }
pub struct AppStatus { /* item_id, desktop_file, exec, categories, package_name, origin */ }
pub struct ServiceStatus { /* item_id, runtime, state, sub_state, enabled, main_pid, since, memory_bytes, unit_path, working_dir, last_checked, last_error */ }
pub struct ServicePort { /* item_id, protocol, address, port, pid, process_name, source, last_seen */ }
pub struct CustomServiceDefinition { /* item_id, start_command, stop_command, status_command, working_dir, env, health_port, health_url, log_path, start_order, enabled */ }
```

- [ ] Step 1: Write serialization and enum round-trip tests for every type.
- [ ] Step 2: Run `cargo test model:: -- --nocapture`; expect failure because types do not exist.
- [ ] Step 3: Implement the types with `serde`, `Display`, `FromStr`, and validation.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: add inventory model types`.

**Acceptance:** `ItemKind`, `ItemSource`, `Scope`, `ServiceState`, `RuntimeKind`, and all structs compile and serialize.

### Task 1.2: Migration v4 — inventory schema

**Files:**
- Modify: `src/storage/migrations.rs`
- Test: `tests/storage_tests.rs`

**Schema:**

```sql
CREATE TABLE items (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    source TEXT NOT NULL,
    source_key TEXT NOT NULL,
    scope TEXT NOT NULL DEFAULT '',
    version TEXT,
    description TEXT,
    icon TEXT,
    command TEXT,
    installed INTEGER NOT NULL DEFAULT 1,
    pinned INTEGER NOT NULL DEFAULT 0,
    hidden INTEGER NOT NULL DEFAULT 0,
    metadata TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(kind, source, scope, source_key)
);
CREATE INDEX idx_items_kind ON items(kind);
CREATE INDEX idx_items_name ON items(name COLLATE NOCASE);
CREATE INDEX idx_items_source_key ON items(source_key);

CREATE TABLE groups (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    icon TEXT,
    color TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE group_items (
    group_id TEXT NOT NULL,
    item_id TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    PRIMARY KEY(group_id, item_id),
    FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE,
    FOREIGN KEY(item_id) REFERENCES items(id) ON DELETE CASCADE
);
CREATE INDEX idx_group_items_item ON group_items(item_id);

CREATE TABLE app_status (
    item_id TEXT PRIMARY KEY,
    desktop_file TEXT,
    exec TEXT,
    categories TEXT,
    package_name TEXT,
    origin TEXT,
    FOREIGN KEY(item_id) REFERENCES items(id) ON DELETE CASCADE
);

CREATE TABLE service_status (
    item_id TEXT PRIMARY KEY,
    runtime TEXT NOT NULL,
    state TEXT NOT NULL,
    sub_state TEXT,
    enabled INTEGER,
    main_pid INTEGER,
    since TEXT,
    memory_bytes INTEGER,
    unit_path TEXT,
    working_dir TEXT,
    last_checked TEXT NOT NULL,
    last_error TEXT,
    FOREIGN KEY(item_id) REFERENCES items(id) ON DELETE CASCADE
);
CREATE INDEX idx_service_status_state ON service_status(state);

CREATE TABLE service_ports (
    item_id TEXT NOT NULL,
    protocol TEXT NOT NULL,
    address TEXT NOT NULL,
    port INTEGER NOT NULL,
    pid INTEGER,
    process_name TEXT,
    source TEXT NOT NULL,
    last_seen TEXT NOT NULL,
    PRIMARY KEY(item_id, protocol, address, port),
    FOREIGN KEY(item_id) REFERENCES items(id) ON DELETE CASCADE
);
CREATE INDEX idx_service_ports_port ON service_ports(port);

CREATE TABLE custom_service_definitions (
    item_id TEXT PRIMARY KEY,
    start_command TEXT NOT NULL,
    stop_command TEXT,
    status_command TEXT,
    working_dir TEXT,
    env TEXT,
    health_port INTEGER,
    health_url TEXT,
    log_path TEXT,
    start_order INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY(item_id) REFERENCES items(id) ON DELETE CASCADE
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE scan_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    status TEXT NOT NULL,
    apps_found INTEGER NOT NULL DEFAULT 0,
    services_found INTEGER NOT NULL DEFAULT 0,
    custom_found INTEGER NOT NULL DEFAULT 0,
    error TEXT
);
```

- [ ] Step 1: Write a migration test that opens a fresh in-memory DB and asserts all new tables and indexes exist.
- [ ] Step 2: Run it; expect failure.
- [ ] Step 3: Add `migration_v4` and wire `if current < 4`.
- [ ] Step 4: Run the test; expect pass.
- [ ] Step 5: Verify the legacy DB opens and migrates: copy the backup to a temp path and open it in a test/CLI check.
- [ ] Step 6: Commit `feat: add inventory schema v4`.

**Acceptance:** Fresh DB and legacy backup both migrate without data loss; old tables remain intact for rollback.

### Task 1.3: Item storage

**Files:**
- Create: `src/storage/items.rs`
- Modify: `src/storage/mod.rs`
- Test: inline tests

**Interfaces:**

```rust
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
}

pub fn upsert(conn: &Connection, item: NewItem<'_>) -> Result<Item>;
pub fn get(conn: &Connection, id: &str) -> Result<Option<Item>>;
pub fn get_by_source(conn: &Connection, kind: ItemKind, source: ItemSource, scope: Option<Scope>, source_key: &str) -> Result<Option<Item>>;
pub fn list(conn: &Connection, kind: Option<ItemKind>) -> Result<Vec<Item>>;
pub fn find_by_name(conn: &Connection, name: &str) -> Result<Vec<Item>>;
pub fn set_pinned(conn: &Connection, id: &str, pinned: bool) -> Result<bool>;
pub fn set_hidden(conn: &Connection, id: &str, hidden: bool) -> Result<bool>;
pub fn mark_missing_for_source(conn: &Connection, source: ItemSource, seen: &HashSet<String>) -> Result<usize>;
```

- [ ] Step 1: Write tests for create/list/get, idempotent upsert, case-insensitive name lookup, pin/hide flags, and absence marking.
- [ ] Step 2: Run tests; expect failure.
- [ ] Step 3: Implement with prepared statements; add `row_to_item`.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: add item storage`.

**Acceptance:** Upserting the same `(kind, source, scope, source_key)` updates name/version without creating a duplicate.

### Task 1.4: Group storage

**Files:**
- Create: `src/storage/groups.rs`
- Modify: `src/storage/mod.rs`
- Test: inline tests

**Interfaces:**

```rust
pub fn create(conn: &Connection, name: &str, description: Option<&str>) -> Result<Group>;
pub fn get_by_name(conn: &Connection, name: &str) -> Result<Option<Group>>;
pub fn list(conn: &Connection) -> Result<Vec<Group>>;
pub fn update(conn: &Connection, id: &str, name: &str, description: Option<&str>) -> Result<bool>;
pub fn delete(conn: &Connection, id: &str) -> Result<bool>;
pub fn add_member(conn: &Connection, group_id: &str, item_id: &str, sort_order: i64) -> Result<()>;
pub fn remove_member(conn: &Connection, group_id: &str, item_id: &str) -> Result<bool>;
pub fn list_members(conn: &Connection, group_id: &str) -> Result<Vec<(Item, i64)>>;
pub fn reorder(conn: &Connection, group_id: &str, item_ids: &[String]) -> Result<()>;
```

- [ ] Step 1: Write tests for CRUD, duplicate names, membership, ordering, and cascade delete.
- [ ] Step 2: Run tests; expect failure.
- [ ] Step 3: Implement.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: add group storage`.

**Acceptance:** Deleting a group removes memberships but not items; deleting an item removes its memberships.

### Task 1.5: Status, ports, custom services, scan runs, settings

**Files:**
- Create: `src/storage/status.rs`, `src/storage/ports.rs`, `src/storage/custom_services.rs`, `src/storage/scan_runs.rs`, `src/storage/settings.rs`
- Modify: `src/storage/mod.rs`
- Test: inline tests

**Interfaces:**

```rust
// status.rs
pub fn upsert_app(conn: &Connection, status: &AppStatus) -> Result<()>;
pub fn get_app(conn: &Connection, item_id: &str) -> Result<Option<AppStatus>>;
pub fn upsert_service(conn: &Connection, status: &ServiceStatus) -> Result<()>;
pub fn get_service(conn: &Connection, item_id: &str) -> Result<Option<ServiceStatus>>;
pub fn list_services(conn: &Connection) -> Result<Vec<ServiceStatus>>;

// ports.rs
pub fn replace_for_item(conn: &Connection, item_id: &str, ports: &[ServicePort]) -> Result<()>;
pub fn list_for_item(conn: &Connection, item_id: &str) -> Result<Vec<ServicePort>>;
pub fn list_all(conn: &Connection) -> Result<Vec<ServicePort>>;

// custom_services.rs
pub fn upsert(conn: &Connection, def: &CustomServiceDefinition) -> Result<()>;
pub fn get(conn: &Connection, item_id: &str) -> Result<Option<CustomServiceDefinition>>;
pub fn list(conn: &Connection) -> Result<Vec<CustomServiceDefinition>>;
pub fn delete(conn: &Connection, item_id: &str) -> Result<bool>;

// scan_runs.rs
pub fn start(conn: &Connection) -> Result<i64>;
pub fn finish(conn: &Connection, id: i64, status: &str, apps: usize, services: usize, custom: usize, error: Option<&str>) -> Result<()>;
pub fn latest(conn: &Connection) -> Result<Option<ScanRun>>;

// settings.rs
pub fn get(conn: &Connection, key: &str) -> Result<Option<String>>;
pub fn set(conn: &Connection, key: &str, value: &str) -> Result<()>;
```

- [ ] Step 1: Write tests for each storage function, including port replacement idempotence and cascade behavior.
- [ ] Step 2: Run tests; expect failure.
- [ ] Step 3: Implement all five modules.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: add inventory status and custom service storage`.

**Acceptance:** All new storage modules are covered and do not require legacy tables.

---

## Phase 2 — Discovery and Inventory Scan

**Goal:** Discover apps, services, ports, package attribution, and custom services; persist them through a new scanner. Do not switch the CLI/GUI yet.

### Task 2.1: Desktop entry parser

**Files:**
- Create: `src/discovery/apps.rs`
- Modify: `src/discovery/mod.rs`
- Test: inline tests with fixture strings

**Interfaces:**

```rust
pub struct DesktopEntry {
    pub desktop_id: String,
    pub path: PathBuf,
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
    pub categories: Vec<String>,
    pub no_display: bool,
    pub hidden: bool,
    pub try_exec: Option<String>,
    pub flatpak_app_id: Option<String>,
}

pub fn parse_desktop_entry(desktop_id: &str, path: &Path, content: &str) -> Option<DesktopEntry>;
pub fn scan_desktop_entries() -> Result<Vec<DesktopEntry>>;
```

Parsing rules:

- Only `[Desktop Entry]`, `Type=Application`.
- Support localized `Name[<locale>]`, falling back to `Name`.
- Skip `NoDisplay=true` and `Hidden=true`.
- Keep entries with a non-empty `Exec`; a missing `Exec` is invalid.
- If `X-Flatpak=<id>` is present, the entry is a Flatpak app.
- `TryExec` is resolved with `which` before showing the app.
- Desktop ID is the file name without `applications/`, with `/` replaced by `-`.
- Search `XDG_DATA_HOME`, each `XDG_DATA_DIRS`, `/usr/share/applications`, `/usr/local/share/applications`, `~/.local/share/applications`, and Flatpak export paths. Deduplicate by desktop ID with XDG precedence.

- [ ] Step 1: Write tests for a minimal valid entry, localized names, NoDisplay/Hidden, missing Exec, X-Flatpak, Categories, and field-code stripping.
- [ ] Step 2: Run tests; expect failure.
- [ ] Step 3: Implement parser and directory scan.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: discover desktop applications`.

**Acceptance:** A real Fedora scan finds VS Code, Firefox, and Flatpak apps without showing libraries or hidden entries.

### Task 2.2: Package inventory and file attribution

**Files:**
- Modify: `src/backends/dnf.rs` (add package inventory only; leave legacy dependency code untouched for now)
- Create: `src/discovery/software.rs`
- Test: parser tests with fixture output

**Interfaces:**

```rust
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub arch: String,
    pub reason: String,
    pub repo: String,
    pub summary: Option<String>,
}

pub fn discover_packages(backend: &DnfCliBackend) -> Result<Vec<InstalledPackage>>;
pub fn attribute_files(paths: &[PathBuf]) -> Result<HashMap<PathBuf, String>>;
```

- `dnf5 repoquery --installed --queryformat "%{name}\t%{evr}\t%{arch}\t%{reason}\t%{repoid}\t%{summary}\n"`.
- `rpm -qf <path>...` in one batch; map owned files to package names; ignore "not owned by any package".
- Do **not** run `%{files}`, `requires`, or `provides`.

- [ ] Step 1: Write parser tests for DNF output and `rpm -qf` output, including unowned paths and spaces.
- [ ] Step 2: Run tests; expect failure.
- [ ] Step 3: Implement.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: add package inventory and target file attribution`.

**Acceptance:** Desktop files and unit files can be attributed to packages without scanning every file in every RPM.

### Task 2.3: systemd service details

**Files:**
- Modify: `src/discovery/services.rs`
- Create: `src/discovery/service_details.rs`
- Test: parser tests with fixture `systemctl show` output

**Interfaces:**

```rust
pub struct UnitDetails {
    pub unit: String,
    pub active_state: String,
    pub sub_state: String,
    pub unit_file_state: Option<String>,
    pub main_pid: Option<u32>,
    pub since: Option<String>,
    pub memory_bytes: Option<u64>,
    pub fragment_path: Option<String>,
    pub working_directory: Option<String>,
}

pub fn show_units(units: &[String], user: bool) -> Result<Vec<UnitDetails>>;
```

- Run one `systemctl [--user] show --no-pager -p ... <unit>...` call.
- Parse `Key=Value` blocks separated by blank lines.
- Empty values become `None`.
- Support `systemctl --user` for user units.

- [ ] Step 1: Write parser tests covering active/inactive/failed, missing properties, and multiple units.
- [ ] Step 2: Run tests; expect failure.
- [ ] Step 3: Implement.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: add systemd unit detail discovery`.

**Acceptance:** One command returns status, PID, uptime, memory, unit path, and working directory for many units.

### Task 2.4: Port discovery

**Files:**
- Create: `src/discovery/ports.rs`
- Test: inline parser tests

**Interfaces:**

```rust
pub struct ListeningPort {
    pub protocol: PortProtocol,
    pub address: String,
    pub port: u16,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
}

pub fn parse_ss(output: &str, protocol: PortProtocol) -> Vec<ListeningPort>;
pub fn discover_listening_ports() -> Result<Vec<ListeningPort>>;
pub fn discover_socket_units() -> Result<Vec<SocketUnit>>;
pub fn unit_for_pid(pid: u32) -> Option<String>;
```

- Parse `ss -H -ltnp` and `ss -H -lunp`.
- Handle IPv4, IPv6 `[::]:5432`, `0.0.0.0:80`, `127.0.0.1%lo:631`.
- Extract `users:(("postgres",pid=1234,fd=5))` when available.
- `systemctl list-sockets --no-legend --no-pager` provides socket unit → service associations.
- `/proc/<pid>/cgroup` maps PIDs to units when `ss` hides process info.
- Port attribution is best effort; never fail the scan because a port owner is hidden.

- [ ] Step 1: Write parser tests for IPv4, IPv6, wildcard binds, UDP, and missing process info.
- [ ] Step 2: Run tests; expect failure.
- [ ] Step 3: Implement parsers and best-effort discovery.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: discover listening service ports`.

**Acceptance:** PostgreSQL/MariaDB ports are shown when visible; unattributed ports still appear.

### Task 2.5: Custom service definitions and status

**Files:**
- Create: `src/services/custom.rs` (or `src/discovery/custom.rs` if discovery-only)
- Test: unit tests with temporary directories and shell commands

**Interfaces:**

```rust
pub struct CustomServiceSpec {
    pub name: String,
    pub start_command: String,
    pub stop_command: Option<String>,
    pub status_command: Option<String>,
    pub working_dir: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
    pub health_port: Option<u16>,
    pub health_url: Option<String>,
    pub log_path: Option<PathBuf>,
    pub start_order: i64,
}

pub enum CustomState { Running, Stopped, Failed, Unknown }

pub fn status(spec: &CustomServiceDefinition) -> Result<CustomState>;
pub fn start(spec: &CustomServiceDefinition) -> Result<()>;
pub fn stop(spec: &CustomServiceDefinition) -> Result<()>;
```

Rules:

- Always run as the current user.
- `start` spawns detached (`setsid` on Unix), redirects stdout/stderr to `log_path`, returns immediately.
- `status` checks `status_command` first, then `health_port`/`health_url` via TCP connect, then reports `Unknown` if no reliable check exists.
- `stop` runs `stop_command`; if absent, return a validation error that the UI explains.
- Validate working directory and log path before spawning.
- Never invoke `sudo` or `pkexec`.

- [ ] Step 1: Write tests with `sh -c 'exit 0'`, a long-running `sleep`, and a temporary log file.
- [ ] Step 2: Run tests; expect failure.
- [ ] Step 3: Implement start/stop/status.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: add user-owned custom service commands`.

**Acceptance:** A custom service can start a detached command, report status via command/port, and stop through its stop command.

### Task 2.6: New inventory scanner

**Files:**
- Create: `src/reconciliation/inventory.rs`
- Modify: `src/reconciliation/mod.rs`
- Test: unit tests using fixture discovery data and an in-memory DB

**Interfaces:**

```rust
pub struct InventoryScan {
    pub apps: Vec<DiscoveredApp>,
    pub services: Vec<DiscoveredService>,
    pub custom: Vec<CustomServiceDefinition>,
    pub packages: Vec<InstalledPackage>,
}

pub struct ScanReport {
    pub apps: usize,
    pub services: usize,
    pub custom: usize,
    pub missing: usize,
}

pub fn scan(db: &Database, progress: &mut dyn FnMut(ScanEvent)) -> Result<ScanReport>;
pub fn apply(db: &Database, inventory: &InventoryScan) -> Result<ScanReport>;
```

`apply` in one transaction:

1. Insert a `scan_runs` row.
2. Upsert apps by `(kind, source, scope, source_key)`.
3. Upsert services (systemd system/user + custom).
4. Write `app_status`/`service_status`.
5. Replace `service_ports` for scanned services.
6. Mark resources missing only for sources successfully scanned.
7. Preserve groups, pins, hidden flags, and custom definitions.
8. Finish the scan run with counts.

- [ ] Step 1: Write tests: first scan creates apps/services; second scan updates without duplicates; missing source marks only that source absent; groups/pins survive.
- [ ] Step 2: Run tests; expect failure.
- [ ] Step 3: Implement.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: add inventory scan pipeline`.

**Acceptance:** `scan` can be run repeatedly and converges; no legacy `relationships` or `roots` writes occur.

---

## Phase 3 — Application Layer and CLI Read Views

**Goal:** Make the new inventory usable from the CLI. Keep legacy commands temporarily compiled, but `main` dispatches to the new commands.

### Task 3.1: Catalog views

**Files:**
- Create: `src/services/catalog.rs`
- Modify: `src/services/mod.rs`
- Test: inline tests with in-memory DB seeds

**Interfaces:**

```rust
pub struct AppView { pub item: Item, pub status: Option<AppStatus>, pub groups: Vec<Group> }
pub struct ServiceView { pub item: Item, pub status: Option<ServiceStatus>, pub ports: Vec<ServicePort>, pub groups: Vec<Group>, pub package: Option<String> }
pub struct GroupView { pub group: Group, pub items: Vec<Item>, pub running: usize, pub failed: usize }
pub struct Dashboard { pub groups: Vec<GroupView>, pub failed: Vec<ServiceView>, pub apps: usize, pub services: usize, pub last_scan: Option<ScanRun> }

pub fn dashboard(db: &Database) -> Result<Dashboard>;
pub fn apps(db: &Database, all: bool, group: Option<&str>) -> Result<Vec<AppView>>;
pub fn services(db: &Database, all: bool, group: Option<&str>) -> Result<Vec<ServiceView>>;
pub fn groups(db: &Database) -> Result<Vec<GroupView>>;
pub fn item_detail(db: &Database, id_or_name: &str) -> Result<ItemDetail>;
```

Default service filter:

```text
pinned OR hidden = false AND (
    grouped OR failed OR has_port OR provided_by_user_installed_package
)
```

If a service has no status row, show `Unknown`, not `Running`.

- [ ] Step 1: Write tests for app/service filtering, group aggregation, and dashboard counts.
- [ ] Step 2: Run tests; expect failure.
- [ ] Step 3: Implement.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: add inventory catalog views`.

**Acceptance:** A seeded DB produces stable, sorted views without touching the GTK layer.

### Task 3.2: Rewrite `main` and read-only CLI commands

**Files:**
- Modify: `src/main.rs`, `src/cli/mod.rs`
- Create: `src/cli/apps.rs`, `src/cli/services.rs`, `src/cli/groups.rs`, `src/cli/info.rs`
- Keep old command modules present but unreachable until Phase 6.
- Test: `tests/cli_tests.rs`

**CLI contract:**

```text
chapeau                         Dashboard
chapeau scan
chapeau apps [--all] [--group NAME]
chapeau services [--all] [--group NAME]
chapeau groups
chapeau group create NAME [--description TEXT]
chapeau group delete NAME
chapeau group add NAME ITEM
chapeau group remove NAME ITEM
chapeau info ITEM
chapeau db status
```

Output rules:

- Apps: name, source, version, groups, missing marker.
- Services: name, state, enabled, ports, package, groups, missing marker.
- Default views hide `hidden = true`; `--all` shows them with a `[hidden]` tag.
- No dependency/root language anywhere.

- [ ] Step 1: Write CLI integration tests for `apps`, `services`, `groups`, and `info` on a temp DB.
- [ ] Step 2: Run tests; expect failure.
- [ ] Step 3: Implement commands and dispatch.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: add inventory CLI read views`.

**Acceptance:** `chapeau apps`, `chapeau services`, and `chapeau groups` work against a scanned DB; `chapeau info` resolves app/service IDs and reports ambiguity clearly.

### Task 3.3: Simplify the DNF backend, add status command, remove dead scan paths

**Files:**
- Modify: `src/backends/dnf.rs`, `src/cli/status.rs`
- Test: existing DNF parsing tests plus new ones

- Remove dependency resolution functions from active use. Leave them only until Phase 6 if other legacy code still references them.
- Keep `is_available`, `discover_repositories`, `removal_argv` for now.
- New `chapeau status` reports inventory counts, running/failed services, last scan, and database path.
- Do not mention roots, relationships, drift, or provenance.

- [ ] Step 1: Write tests for the new status summary.
- [ ] Step 2: Run tests; expect failure.
- [ ] Step 3: Implement.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: add inventory status summary`.

**Acceptance:** `chapeau status` is a one-screen inventory dashboard.

---

## Phase 4 — Actions and Group Control

**Goal:** Make the product actually manageable: open apps, start/stop/restart services, refresh ports, and control whole groups.

### Task 4.1: Open apps

**Files:**
- Create: `src/services/launch.rs`
- Modify: `src/services/mod.rs`
- Test: unit test with a fake launcher path or command

**Interfaces:**

```rust
pub fn open_app(db: &Database, item_id: &str) -> Result<()>;
```

- Prefer `gio launch <desktop-file>` for desktop entries.
- For Flatpak desktop entries, the same path works.
- For manual apps, spawn `command` detached with `setsid`.
- Return a clear error if the command or desktop file no longer exists.

- [ ] Step 1: Write a test that a missing desktop file returns a validation error.
- [ ] Step 2: Run test; expect failure.
- [ ] Step 3: Implement.
- [ ] Step 4: Run test; expect pass.
- [ ] Step 5: Commit `feat: launch apps from inventory`.

### Task 4.2: systemd control

**Files:**
- Modify: `src/services/units.rs` (or create `src/services/control.rs` and reuse `privileged`)
- Modify: `src/services/privileged.rs` only if a new timeout is needed
- Test: argv construction and non-service rejection

**Interfaces:**

```rust
pub enum ServiceAction { Start, Stop, Restart, Enable, Disable }

pub fn control_service(db: &Database, item_id: &str, action: ServiceAction, privilege: Privilege) -> Result<ServiceControlOutcome>;
```

- System units: `sudo` (CLI) or `pkexec --disable-internal-agent` (GUI).
- User units: `systemctl --user`, no privilege escalation.
- Refresh only the affected unit via `systemctl show`; do not run a full scan.
- Record history.
- Never shell-expand user input.

- [ ] Step 1: Write tests for `systemctl` argv for system and user scope, and rejection of non-service items.
- [ ] Step 2: Run tests; expect failure.
- [ ] Step 3: Implement.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: control systemd services from inventory`.

### Task 4.3: Custom service actions

**Files:**
- Modify: `src/services/custom.rs`
- Create: `src/storage/custom_services.rs` integration tests
- Test: temp script fixtures

**Interfaces:**

```rust
pub fn start_custom(db: &Database, item_id: &str) -> Result<()>;
pub fn stop_custom(db: &Database, item_id: &str) -> Result<()>;
pub fn refresh_custom_status(db: &Database, item_id: &str) -> Result<ServiceStatus>;
```

- Load the definition from storage.
- Run start/stop as the current user.
- Redirect logs.
- Update `service_status` immediately after the action.
- Do not use pkexec.

- [ ] Step 1: Write tests using a temp script that writes a marker file and exits.
- [ ] Step 2: Run tests; expect failure.
- [ ] Step 3: Implement.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: run custom service actions`.

### Task 4.4: Group bulk actions

**Files:**
- Modify: `src/services/groups.rs`
- Create: `src/services/control.rs` group action functions if not already there
- Test: ordering and failure aggregation

**Interfaces:**

```rust
pub enum GroupAction { Start, Stop, Restart }

pub struct GroupActionResult {
    pub item_id: String,
    pub name: String,
    pub success: bool,
    pub error: Option<String>,
}

pub fn run_group_action(db: &Database, group_id: &str, action: GroupAction, privilege: Privilege) -> Result<Vec<GroupActionResult>>;
```

Rules:

- Start in ascending `sort_order`; stop/restart in descending order.
- Skip apps.
- Continue after a failure; return all results.
- If a custom service is included, it runs unprivileged; system services use the provided privilege.

- [ ] Step 1: Write a test with three fake services and a failing middle action; assert order and continuation.
- [ ] Step 2: Run tests; expect failure.
- [ ] Step 3: Implement.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: add group bulk service actions`.

### Task 4.5: Port refresh and health

**Files:**
- Create: `src/services/ports.rs`
- Test: parser tests plus a local TcpListener test

**Interfaces:**

```rust
pub fn refresh_ports(db: &Database, item_id: Option<&str>) -> Result<usize>;
pub fn check_tcp(port: u16) -> bool;
```

- Discover listening ports once, then replace per-service port rows.
- Map PID → unit and socket unit → service.
- For custom services, merge configured `health_port` and discovered ports.
- Never fail the refresh because of permission-denied port owners.

- [ ] Step 1: Write a test that binds a local `TcpListener` and confirms the port appears.
- [ ] Step 2: Run test; expect failure.
- [ ] Step 3: Implement.
- [ ] Step 4: Run test; expect pass.
- [ ] Step 5: Commit `feat: refresh service ports and health`.

### Task 4.6: CLI actions

**Files:**
- Create: `src/cli/actions.rs`, `src/cli/add.rs`
- Modify: `src/cli/mod.rs`, `src/main.rs`
- Test: CLI parsing and validation tests

**CLI contract:**

```text
chapeau open <app>
chapeau start <service> [--group NAME]
chapeau stop <service> [--group NAME]
chapeau restart <service> [--group NAME]
chapeau enable <service>
chapeau disable <service>
chapeau refresh-ports
chapeau add app --name NAME --command CMD [--icon ICON] [--group NAME]
chapeau add service --name NAME --start CMD [--stop CMD] [--status CMD]
                       [--working-dir DIR] [--port PORT] [--log FILE] [--group NAME]
```

- Resolve service by item ID first, then exact name; if multiple matches, print them and exit non-zero.
- `--group` applies the action to every service in the group.
- Print per-item results for bulk actions.

- [ ] Step 1: Write CLI tests for resolution, ambiguity, and bulk result output.
- [ ] Step 2: Run tests; expect failure.
- [ ] Step 3: Implement.
- [ ] Step 4: Run tests; expect pass.
- [ ] Step 5: Commit `feat: add inventory CLI actions`.

**Acceptance:** A user can open an app, start/stop a service, and start a whole group from the terminal.

---

## Phase 5 — GUI Rework

**Goal:** Replace the old semantic UI with the inventory/control-panel UI while keeping the worker architecture and GTK shell.

### Task 5.1: New worker requests and responses

**Files:**
- Modify: `gui/src/service.rs`
- Test: unit tests for response mapping where possible

**Interfaces:**

```rust
pub enum Request {
    Dashboard,
    Apps { all: bool, group: Option<String> },
    Services { all: bool, group: Option<String> },
    Groups,
    ItemDetail(String),
    OpenApp(String),
    ServiceControl { item_id: String, action: ServiceAction },
    GroupControl { group_id: String, action: GroupAction },
    RefreshPorts,
    Scan,
}

pub enum Response {
    Dashboard(Result<Dashboard, String>),
    Apps(Result<Vec<AppView>, String>),
    Services(Result<Vec<ServiceView>, String>),
    Groups(Result<Vec<GroupView>, String>),
    ItemDetail(Result<ItemDetail, String>),
    ActionDone(Result<ActionOutcome, String>),
    ScanProgress(ScanEvent),
    ScanDone(Result<ScanReport, String>),
    WorkerFailed(String),
}
```

- Keep `catch_unwind` and the serial worker.
- Ensure scan progress still works.
- Do not add SQL or backend calls to the UI.

- [ ] Step 1: Write a mapping test for action outcomes.
- [ ] Step 2: Run test; expect failure.
- [ ] Step 3: Implement.
- [ ] Step 4: Run test; expect pass.
- [ ] Step 5: Commit `feat: add inventory GUI worker protocol`.

### Task 5.2: Dashboard

**Files:**
- Create: `gui/src/ui/dashboard.rs`
- Modify: `gui/src/main.rs`
- Test: manual GTK checklist; no automated GTK tests

Content:

- group cards with counts and aggregate status
- Start all / Stop all / Restart all buttons
- failed services section
- last scan time and scan button
- empty state with "Scan now"

- [ ] Step 1: Create the page with a loading state and empty state.
- [ ] Step 2: Wire `Request::Dashboard`.
- [ ] Step 3: Verify manually with a seeded DB.
- [ ] Step 4: Commit `feat: add inventory dashboard`.

### Task 5.3: Apps page

**Files:**
- Create: `gui/src/ui/apps.rs`
- Modify: `gui/src/main.rs`
- Test: manual checklist

Content:

- searchable icon list/grid
- source badge (Desktop/Flatpak/Manual)
- group filter
- Open button
- detail navigation

- [ ] Step 1: Build the page and populate from `Vec<AppView>`.
- [ ] Step 2: Wire Open and detail actions.
- [ ] Step 3: Verify search, group filter, and missing state.
- [ ] Step 4: Commit `feat: add apps page`.

### Task 5.4: Services page

**Files:**
- Create: `gui/src/ui/services.rs`
- Modify: `gui/src/main.rs`
- Test: manual checklist

Content:

- state grouping (Failed, Running, Stopped, Unknown)
- name, package, ports, enabled badge
- start/stop/restart, enable/disable
- group filter
- "All services" switch
- detail navigation

- [ ] Step 1: Build the page and populate from `Vec<ServiceView>`.
- [ ] Step 2: Wire actions and refresh.
- [ ] Step 3: Verify against PostgreSQL/MariaDB.
- [ ] Step 4: Commit `feat: add services page`.

### Task 5.5: Groups page and bulk actions

**Files:**
- Create: `gui/src/ui/groups.rs`
- Modify: `gui/src/main.rs`
- Test: manual checklist

Content:

- group list with aggregate status
- create/rename/delete
- membership editor
- reorder services
- Start all / Stop all / Restart all with result toasts

- [ ] Step 1: Build the page.
- [ ] Step 2: Wire group CRUD and ordering.
- [ ] Step 3: Wire bulk actions and failure reporting.
- [ ] Step 4: Commit `feat: add groups page with bulk actions`.

### Task 5.6: Detail and settings

**Files:**
- Create: `gui/src/ui/item_detail.rs`, `gui/src/ui/settings.rs`
- Modify: `gui/src/main.rs`
- Test: manual checklist

Detail:

- app facts, groups, Open
- service facts, ports, URLs, package, unit path, working dir, logs
- custom service definitions
- pin/hide controls

Settings:

- hidden services switch
- manual app entries
- custom service definitions
- refresh ports
- database path and last scan

- [ ] Step 1: Build both pages.
- [ ] Step 2: Wire pin/hide and manual/custom forms.
- [ ] Step 3: Verify logs, paths, and port links.
- [ ] Step 4: Commit `feat: add item detail and settings pages`.

### Task 5.7: Replace navigation and remove old pages

**Files:**
- Modify: `gui/src/main.rs`, `gui/src/ui/mod.rs`, `gui/src/ui/sidebar.rs`
- Delete: old page modules only after the new pages are wired and the GUI builds

New sidebar:

```text
Dashboard
Apps
Services
Groups
Settings
```

- [ ] Step 1: Update navigation and actions.
- [ ] Step 2: Remove old page registrations.
- [ ] Step 3: Run `cargo build -p chapeau-gui`.
- [ ] Step 4: Run the manual checklist for every page.
- [ ] Step 5: Commit `feat: replace GUI navigation with inventory views`.

**Phase 5 acceptance:** The GUI opens on the dashboard; apps open; services start/stop; groups control multiple services; no graph/analysis/roots UI remains.

---

## Phase 6 — Legacy Removal and Dependency Cleanup

**Goal:** Delete the old semantic layer now that the new path is working. The git tag remains the backup.

### Task 6.1: Remove legacy core and services

**Files:** Delete the legacy files listed in the File Map above.
**Also modify:** `src/lib.rs`, `src/core/mod.rs`, `src/services/mod.rs`, `src/cli/mod.rs`, `src/main.rs`.

- [ ] Step 1: Remove legacy modules from exports.
- [ ] Step 2: Delete files.
- [ ] Step 3: Fix any remaining imports to use `model`/new storage.
- [ ] Step 4: Run `cargo check`.
- [ ] Step 5: Commit `refactor: remove legacy semantic core`.

**Acceptance:** No code references roots, relationships, dependency analysis, planner, graph, provenance, or explanation.

### Task 6.2: Remove legacy dependencies and code paths

**Files:** `Cargo.toml`, remaining CLI files, `src/storage/mod.rs`.

- [ ] Step 1: Remove `petgraph` and any unused crates (`tracing` if unused).
- [ ] Step 2: Remove legacy CLI subcommands and `main` match arms.
- [ ] Step 3: Remove legacy tests or rewrite them against the new model.
- [ ] Step 4: Run `cargo test` and `cargo clippy`.
- [ ] Step 5: Commit `chore: drop legacy dependencies and commands`.

### Task 6.3: Migration v5 — retire legacy tables

**Files:** `src/storage/migrations.rs`, migration tests.

**Migration v5:**

```sql
DROP TABLE IF EXISTS relationships;
DROP TABLE IF EXISTS domain_resources;
DROP TABLE IF EXISTS domains;
DROP TABLE IF EXISTS roots;
DROP TABLE IF EXISTS resource_observations;
DROP TABLE IF EXISTS provenance;
-- Keep history for audit.
```

Before dropping, best-effort migration:

- Copy `domains` to `groups` if any exist.
- Copy `domain_resources` to `group_items` where the old resource can be mapped to a new item.
- Copy `roots` with source `user`/`adopted` to `items.pinned = 1` where a matching item exists.
- Ignore detected roots (legacy classification).
- Record the migration result in `history`.

- [ ] Step 1: Add a migration test using the legacy backup schema.
- [ ] Step 2: Run test; expect failure.
- [ ] Step 3: Implement v5.
- [ ] Step 4: Run test; expect pass.
- [ ] Step 5: Commit `feat: migrate legacy organization data and retire old tables`.

**Acceptance:** The legacy DB migrates; groups/pins survive where mapping is possible; old tables are gone.

---

## Phase 7 — Final Integration, Documentation, and QA

### Task 7.1: README and docs rewrite

- Rewrite `README.md` around apps/services/groups.
- Replace `docs/architecture.md`, `docs/data-model.md`, `docs/database.md`, `docs/reconciliation.md`, `docs/usage.md`, `docs/commands.md`.
- Remove or archive `docs/roots.md`, `docs/removal.md`, `docs/my-system.md`, and graph/provenance references.
- Add `docs/inventory.md` and `docs/custom-services.md`.
- Update `docs/gui.md` with the new pages.

- [ ] Step 1: Rewrite README.
- [ ] Step 2: Rewrite architecture/data/database docs.
- [ ] Step 3: Add app/service/group/custom-service docs.
- [ ] Step 4: Remove stale docs.
- [ ] Step 5: Commit `docs: rewrite documentation for inventory manager`.

### Task 7.2: Packaging and desktop metadata

- Update `gui/resources/io.github.Soulaymane01.Chapeau.metainfo.xml` description.
- Verify `desktop-file-validate` and `appstreamcli validate`.
- Verify `packaging/chapeau.spec` still builds both binaries.
- Add `iproute`/`ss` runtime recommendation if needed (usually part of base Fedora).
- Verify `graphviz` is no longer required.

- [ ] Step 1: Update metadata.
- [ ] Step 2: Run validation.
- [ ] Step 3: Build release binaries.
- [ ] Step 4: Commit `chore: update packaging for inventory manager`.

### Task 7.3: Full test matrix and manual acceptance

Automated:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy -p chapeau-gui --all-targets -- -D warnings
cargo test
cargo build --release
cargo build --release -p chapeau-gui
```

Manual checklist:

- [ ] Fresh DB scan finds desktop apps and Flatpak apps.
- [ ] VS Code / IntelliJ / Antigravity appear when installed.
- [ ] PostgreSQL/MariaDB services appear with state and ports.
- [ ] Starting/stopping a system service prompts for privilege and updates state.
- [ ] A user systemd service starts without privilege.
- [ ] Creating a custom service works; start/stop/status/logs work.
- [ ] Groups can be created, reordered, and started/stopped as a unit.
- [ ] Hidden items stay hidden after scans.
- [ ] Scan is idempotent and fast.
- [ ] GUI dashboard opens without stale spinners.
- [ ] Old database backup restores successfully.

- [ ] Step 1: Run automated checks.
- [ ] Step 2: Run manual checklist on Fedora.
- [ ] Step 3: Fix defects.
- [ ] Step 4: Commit `test: complete inventory manager acceptance`.

### Task 7.4: Release notes and version bump

- Update `CHANGELOG.md` with the rework.
- Bump to `0.2.0` if this is a breaking release.
- Tag `v0.2.0-inventory` after QA.

- [ ] Step 1: Write changelog.
- [ ] Step 2: Bump versions.
- [ ] Step 3: Commit and tag.

---

## Definition of Done

The rework is complete when:

1. `chapeau scan` discovers apps, systemd services, custom services, and ports without building a dependency graph.
2. `chapeau apps` lists launchable apps and `chapeau open <app>` launches them.
3. `chapeau services` lists meaningful services with state/ports, and start/stop/restart works.
4. `chapeau groups` supports create/delete/add/remove/reorder and bulk start/stop.
5. `chapeau add app` and `chapeau add service` create manual entries.
6. The GUI exposes dashboard, apps, services, groups, detail, and settings pages.
7. No roots, relationships, dependency graph, orphan analysis, or removal planner remains in the active codebase.
8. The legacy database migrates with a backup, or a fresh DB is created with a clear message.
9. All automated checks pass and the manual acceptance checklist passes on Fedora.

---

## Execution Notes

- Work on `rework/inventory-v2`.
- Keep the legacy tag `legacy-v0.1.0-semantic` untouched.
- Prefer adding new modules alongside legacy code, then switching frontends, then deleting legacy code in Phase 6. This keeps the build reviewable and bisectable.
- Do not mix GUI and storage changes in the same commit.
- Every new behavior needs a test before the implementation.
- If a task grows beyond one reviewable commit, split it rather than committing a broken build.
