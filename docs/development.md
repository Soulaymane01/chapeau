# Developing Chapeau

## Prerequisites

- Rust toolchain (rustc + cargo)
- Fedora system with DNF5, systemd, and Flatpak
- For the GUI only: `gtk4-devel` and `libadwaita-devel`

## Building

```bash
# CLI (default workspace member)
cargo build

# GUI (separate workspace member, needs GTK development headers)
sudo dnf install gtk4-devel libadwaita-devel
cargo build -p chapeau-gui

# Release builds
cargo build --release
cargo build --release -p chapeau-gui
```

## Running checks

```bash
# Type checking
cargo check

# Full build
cargo build

# Tests
cargo test

# Formatting
cargo fmt
cargo fmt --check

# Linting
cargo clippy --all-targets --all-features -- -D warnings
```

## Project structure

```
src/
├── main.rs                  # CLI entry point, command dispatch
├── lib.rs                   # Module declarations and re-exports
├── errors.rs                # Error types (ChapeauError)
│
├── cli/                     # CLI command implementations
│   ├── mod.rs
│   ├── scan.rs              # chapeau scan
│   ├── status.rs            # chapeau status
│   ├── packages.rs          # chapeau packages
│   ├── services.rs          # chapeau services
│   ├── flatpaks.rs          # chapeau flatpaks
│   ├── repositories.rs      # chapeau repositories
│   ├── why.rs               # chapeau why <resource>
│   ├── dependencies.rs      # chapeau dependencies <resource>
│   ├── dependents.rs        # chapeau dependents <resource>
│   ├── domains.rs           # chapeau domains <subcommand>
│   ├── orphaned.rs          # chapeau orphaned
│   ├── unused.rs            # chapeau unused
│   ├── remove.rs            # chapeau remove <resource>
│   └── db.rs                # chapeau db status
│
├── core/                    # Domain model and semantic logic
│   ├── mod.rs
│   ├── resource.rs          # Resource type and struct
│   ├── relationship.rs      # Relationship types and struct
│   ├── domain.rs            # Domain struct
│   ├── observation.rs       # Observation struct
│   ├── provenance.rs        # Provenance struct
│   ├── graph.rs             # SystemGraph (petgraph)
│   ├── analysis.rs          # Orphaned/unused detection
│   ├── planner.rs           # Removal plan generation
│   └── explanation.rs       # "Why" explanation formatting
│
├── backends/                # System interface layer
│   ├── mod.rs
│   ├── package_backend.rs   # PackageBackend trait
│   ├── service_backend.rs   # ServiceBackend trait
│   ├── flatpak_backend.rs   # FlatpakBackendTrait trait
│   ├── dnf.rs               # DNF5 CLI backend
│   ├── systemd.rs           # systemd D-Bus backend
│   └── flatpak.rs           # Flatpak CLI backend
│
├── discovery/               # Intermediate data structures
│   ├── mod.rs               # SystemSnapshot, ScanSummary
│   ├── packages.rs          # PackageRecord, RepoRecord, etc.
│   ├── services.rs          # UnitRecord, ActiveState, etc.
│   ├── flatpaks.rs          # FlatpakRecord, FlatpakRemote, etc.
│   └── repositories.rs      # RepositorySnapshot (minimal)
│
├── reconciliation/          # State synchronization
│   ├── mod.rs
│   ├── scanner.rs           # scan, reconcile_after_removal
│   └── drift.rs             # Drift detection
│
├── services/                # Application layer shared by CLI and GUI
│   ├── mod.rs
│   ├── overview.rs          # My System view model
│   ├── detail.rs            # Resource detail view model
│   ├── drift.rs             # Read-only drift detection
│   └── scan.rs              # Scan with progress events
│
└── storage/                 # SQLite persistence
    ├── mod.rs
    ├── database.rs          # Connection, WAL, transactions
    ├── migrations.rs        # Schema creation
    ├── resources.rs         # Resource CRUD
    ├── relationships.rs     # Relationship CRUD
    ├── domains.rs           # Domain CRUD
    ├── observations.rs      # Observation CRUD
    ├── provenance.rs        # Provenance CRUD
    └── history.rs           # Operation history

gui/                         # GUI workspace member (chapeau-gui)
├── Cargo.toml
└── src/
    ├── main.rs              # GTK application, window wiring
    ├── service.rs           # Database/scan worker thread
    └── ui/
        ├── mod.rs
        ├── sidebar.rs       # My System sidebar
        └── detail.rs        # Resource detail page
```

## Adding a new resource type

1. Add a variant to `ResourceType` in `src/core/resource.rs`
2. Update the `parse()`, `as_str()`, `all()`, and `Display` implementations
3. Add discovery logic in the appropriate backend or discovery module
4. Add reconciliation handling in `reconciliation/scanner.rs`
5. Update CLI commands that filter by type if needed
6. Add tests

## Adding a new backend

1. Define a trait in `src/backends/` (e.g., `package_backend.rs`)
2. Implement the trait for the system interface (e.g., `dnf.rs`)
3. Add the backend to the discovery pipeline in `reconciliation/scanner.rs`
4. Add the backend to the `backends` CLI command display
5. Add tests (parsing tests for CLI output, integration tests for live systems)

## Adding a new relationship type

1. Add a variant to `RelationshipType` in `src/core/relationship.rs`
2. Update the `parse()`, `as_str()`, `all()`, and `Display` implementations
3. Add relationship creation logic in the appropriate scanner or analysis module
4. Update the `why` command display if the relationship should be shown
5. Update the graph traversal if the relationship affects analysis
6. Add tests

## Adding a new CLI command

1. Create a new file in `src/cli/` (e.g., `my_command.rs`)
2. Define the clap args struct
3. Implement the command handler
4. Register the subcommand in `src/cli/mod.rs`
5. Add the match arm in `src/main.rs`
6. Add tests

## Adding a database migration

1. Increment the migration version in `src/storage/migrations.rs`
2. Add a new migration function (e.g., `migration_v2`)
3. Add the migration to the version check in `run()`
4. Test with a fresh database and an existing database
