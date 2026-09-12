# Chapeau Architecture

## Overview

Chapeau is a semantic and organizational layer above Fedora's native system mechanisms. It does not replace DNF, systemd, or Flatpak. It observes them, records what they report, and adds a relationship and meaning layer on top.

```
Fedora System
  │
  ├── DNF5 ──────────── Package backend
  ├── systemd ────────── Service backend
  └── Flatpak ────────── Flatpak backend
        │
        ▼
    Backends
        │
        ▼
    Discovery
        │
        ▼
    SystemSnapshot
        │
        ▼
    Reconciliation
        │
        ▼
    SQLite Database
        │
        ▼
    Semantic Model
        │
        ├── SystemGraph (petgraph)
        ├── Analysis (orphaned/unused)
        ├── Planning (removal)
        └── Explanation (why)
        │
        ▼
    Services (application layer)
        │
        ├── chapeau (CLI)
        └── chapeau-gui (GTK4 + libadwaita)
```

## Module responsibilities

Each layer has a single, clearly defined responsibility.

### Services (`src/services/`)

Presentation-agnostic application layer shared by the CLI and the GUI. It
turns the semantic model into view models (`Overview`, `ResourceDetail`,
`DriftReport`, scan progress events) and is the only entry point frontends
use. Services return data, never formatted text; see
[Chapeau GUI](gui.md).

### CLI (`src/cli/`)

Parses user input, calls core/storage functions, and formats output. The CLI should not contain domain logic, database queries, or backend invocations.

### Core (`src/core/`)

Contains the domain model and semantic logic:

- **Types** (`resource.rs`, `relationship.rs`, `domain.rs`, `observation.rs`, `provenance.rs`) — Data structures and validation
- **Graph** (`graph.rs`) — In-memory directed graph built from the database using petgraph
- **Analysis** (`analysis.rs`) — Orphaned and unused resource detection
- **Planner** (`planner.rs`) — Removal plan generation
- **Explanation** (`explanation.rs`) — Human-readable "why" explanations

Core operates on data from storage. It does not call backends directly.

### Backends (`src/backends/`)

Interface with Fedora's native system mechanisms. Each backend is a trait implementation that discovers facts from the live system:

- **DNF5** (`dnf.rs`) — Queries DNF5 CLI for installed packages, repositories, and dependencies
- **systemd** (`systemd.rs`) — Connects to systemd via D-Bus to discover service units
- **Flatpak** (`flatpak.rs`) — Queries the Flatpak CLI for installed applications, runtimes, and remotes

Backends return raw record types. They do not write to the database.

### Discovery (`src/discovery/`)

Defines the intermediate data structures that backends populate:

- **Record types** — `PackageRecord`, `UnitRecord`, `FlatpakRecord`, `RepoRecord`, `FlatpakRemote`
- **SystemSnapshot** — The unified snapshot of all discovered system state
- **Enumerations** — `InstallReason`, `ActiveState`, `UnitFileState`, `UnitType`, etc.

Discovery defines what is captured, not how it is captured.

### Reconciliation (`src/reconciliation/`)

Synchronizes Chapeau's database with the live system:

- **Scanner** (`scanner.rs`) — Full system scan (`scan`), absence marking, and targeted post-removal reconciliation (`reconcile_after_removal`)
- **Drift** (`drift.rs`) — Read-only comparison of the recorded model against a fresh discovery (missing/new/changed); used by `chapeau drift` and recorded by each scan

Reconciliation compares discovered state against stored state and applies the necessary changes.

### Storage (`src/storage/`)

SQLite persistence layer. Provides CRUD operations for all entity types:

- **Database** (`database.rs`) — Connection management, WAL mode, foreign keys, transactions
- **Migrations** (`migrations.rs`) — Schema creation and versioning
- **Resources** (`resources.rs`) — Resource CRUD
- **Relationships** (`relationships.rs`) — Relationship CRUD
- **Domains** (`domains.rs`) — Domain CRUD and domain-resource associations
- **Observations** (`observations.rs`) — Observation upsert and retrieval
- **Provenance** (`provenance.rs`) — Provenance upsert and retrieval
- **History** (`history.rs`) — Operation history logging

Storage does not decide semantic meaning. It persists and retrieves data.

## Design rules

1. **Backends produce facts.** They discover what exists in the live system.
2. **Discovery defines the snapshot.** It structures raw facts into a unified representation.
3. **Reconciliation synchronizes state.** It compares snapshots against the database and applies changes.
4. **Core computes meaning.** It analyzes relationships, generates explanations, and produces removal plans.
5. **Storage persists everything.** It does not interpret data, only stores and retrieves it.
6. **CLI presents to the user.** It formats output and handles input, but contains no business logic.

## Dependencies

Key Rust crate dependencies:

| Crate | Purpose |
|-------|---------|
| `clap` | CLI argument parsing |
| `rusqlite` | SQLite database access |
| `petgraph` | In-memory directed graph |
| `serde` / `serde_json` | Serialization |
| `thiserror` / `anyhow` | Error handling |
| `uuid` | Unique identifier generation |
| `chrono` | Timestamp generation |
| `zbus` / `zbus_systemd` | D-Bus communication with systemd |
| `tokio` | Async runtime (for D-Bus) |
| `tracing` | Structured logging |
