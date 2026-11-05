use chapeau::storage::Database;
use std::path::PathBuf;

/// Verify that a real legacy database (v3 semantic schema) migrates to the
/// inventory schema without losing the legacy tables.
///
/// This is ignored by default because it needs a copy of a real database:
///
/// ```bash
/// CHAPEAU_LEGACY_DB=~/.local/state/chapeau/backups/chapeau-pre-rework-*.db \
///   cargo test --test legacy_migration_tests -- --ignored
/// ```
#[test]
#[ignore = "requires CHAPEAU_LEGACY_DB pointing at a legacy database"]
fn legacy_database_migrates_to_v4() {
    let source = std::env::var("CHAPEAU_LEGACY_DB")
        .expect("set CHAPEAU_LEGACY_DB to a legacy database copy");
    let source = PathBuf::from(source);
    assert!(source.is_file(), "legacy database not found: {source:?}");

    let dir = tempfile::tempdir().expect("create temp dir");
    let target = dir.path().join("chapeau.db");
    std::fs::copy(&source, &target).expect("copy legacy database");

    let db = Database::open(&target).expect("open and migrate legacy database");

    let version: i64 = db
        .conn()
        .query_row("SELECT MAX(version) FROM _migrations", [], |row| row.get(0))
        .expect("read migration version");
    assert_eq!(version, 4);

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
        let exists: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .expect("query sqlite_master");
        assert_eq!(exists, 1, "missing new table {table}");
    }

    // The legacy tables remain for rollback until migration v5.
    let resources: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM resources", [], |row| row.get(0))
        .expect("read legacy resources");
    assert!(resources > 0, "legacy resources should be preserved");
}
