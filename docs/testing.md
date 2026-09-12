# Chapeau Testing

## Running tests

```bash
# Run all tests
cargo test

# Run tests in a specific module
cargo test reconciliation::scanner::tests

# Run a specific test
cargo test reconcile_removes_stale_package

# Show test output
cargo test -- --nocapture
```

## Test organization

Tests are organized into two categories:

### Unit tests (inline)

Located inside source files as `#[cfg(test)] mod tests` blocks. These test internal module logic using in-memory SQLite databases.

| Module | File | Test count |
|--------|------|------------|
| `core::resource` | `src/core/resource.rs` | 15 |
| `core::relationship` | `src/core/relationship.rs` | 18 |
| `core::domain` | `src/core/domain.rs` | 7 |
| `core::observation` | `src/core/observation.rs` | 5 |
| `core::provenance` | `src/core/provenance.rs` | 4 |
| `core::analysis` | `src/core/analysis.rs` | 16 |
| `core::planner` | `src/core/planner.rs` | 5 |
| `backends::dnf` | `src/backends/dnf.rs` | 5 |
| `discovery::services` | `src/discovery/services.rs` | 19 |
| `discovery::flatpaks` | `src/discovery/flatpaks.rs` | 19 |
| `reconciliation::scanner` | `src/reconciliation/scanner.rs` | 24 |
| `reconciliation::drift` | `src/reconciliation/drift.rs` | 10 |
| `storage::roots` | `src/storage/roots.rs` | 12 |
| `storage::observations` | `src/storage/observations.rs` | 4 |
| `cli::overview` | `src/cli/overview.rs` | 4 |
| `cli::show` | `src/cli/show.rs` | 4 |

### Integration tests

Located in `tests/`. These test cross-module functionality and parsing of real-world output.

| File | Test count | Description |
|------|------------|-------------|
| `tests/cli_tests.rs` | 3 | Error display and type roundtrips |
| `tests/dnf_integration_tests.rs` | 7 | Live DNF5 backend tests (skip if unavailable) |
| `tests/dnf_parsing_tests.rs` | 38 | DNF5 output parsing with hardcoded data |
| `tests/storage_tests.rs` | 54 | Database CRUD, FK cascades, domain associations, graph operations |

## Test categories

### Parsing tests

Test the parsing of CLI output from DNF5, Flatpak, and systemd. These use hardcoded test data and do not require a live system.

Example: `parse_repoquery_single_package` tests parsing a tab-separated DNF5 repoquery output line.

### Serialization tests

Test JSON serialization roundtrips for structs and enums.

Example: `resource_type_serialization` tests that `ResourceType` serializes to JSON and parses back correctly.

### Validation tests

Test input validation for domain models.

Example: `resource_validate_empty_id` tests that an empty ID fails validation.

### Database CRUD tests

Test create, read, update, and delete operations for all entity types. These use in-memory SQLite databases.

Example: `test_insert_resource` tests creating a resource and retrieving it.

### Foreign key cascade tests

Test that deleting a resource cascades to related tables.

Example: `test_relationship_cascade_delete` tests that deleting a resource also deletes its relationships.

### Graph operation tests

Test the SystemGraph's node lookup, edge traversal, and neighbor queries.

Example: `test_system_graph_outgoing_edges` tests that outgoing edges are correctly identified.

### Analysis tests

Test orphaned and unused resource detection logic.

Example: `test_analyze_orphaned_all_reasons` tests that an isolated package has all three orphan reasons.

### Reconciliation tests

Test post-removal stale resource cleanup.

Example: `reconcile_removes_stale_package` tests that a package no longer in the system is removed from the database.

### Live integration tests

Test the DNF5 backend against a real system. These tests are skipped if DNF5 is not available.

Example: `test_discover_installed_packages` tests that `discover_installed()` returns a non-empty list with valid fields.

## Safe testing practices

### In-memory databases

All unit tests use `Database::open_memory()` which creates a temporary in-memory SQLite database. This ensures:

- Tests do not read or modify the real Chapeau database
- Tests are isolated from each other
- Tests leave no residue on the filesystem

### No real system changes

Tests should never:
- Run `sudo dnf5 remove`
- Modify systemd services
- Install or remove Flatpak applications
- Write to the real database at `~/.local/state/chapeau/chapeau.db`

### DNF integration test safety

The DNF integration tests in `tests/dnf_integration_tests.rs` only:
- Query DNF5 for information (read-only)
- Do not install, remove, or modify packages

They are automatically skipped if DNF5 is not available on the system.

## Test helpers

Common test helpers are defined in each test module:

- `temp_db()` / `Database::open_memory()` — Creates an in-memory database
- `insert_package()` — Inserts a test package resource
- `insert_observation()` — Inserts a test observation
- `insert_relationship()` — Inserts a test relationship
- `insert_domain_resource()` — Inserts a domain-resource association
- `dnf5_available()` — Checks if DNF5 is installed (for skip logic)

## Interpreting test output

When running `cargo test`, the output shows:

```
test result: ok. 124 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The raw count may appear higher than the actual number of unique tests because `lib` and `bin` test targets compile overlapping test modules. The actual unique test count is the sum of all distinct test functions.

As of the current implementation, there are approximately 231 unique tests across all modules.
