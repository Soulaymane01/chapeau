use chapeau::core::graph::SystemGraph;
use chapeau::core::{RelationshipOrigin, RelationshipType, ResourceType};
use chapeau::storage::{
    domains, history, observations, provenance, relationships, resources, Database,
};

fn temp_db() -> Database {
    Database::open_memory().expect("failed to create in-memory database")
}

// ---------- 1. Database creation ----------

#[test]
fn test_database_creation() {
    let db = temp_db();
    let status = db.status().expect("status failed");
    // In-memory databases may return ":memory:" or an empty path.
    assert!(
        status.db_path.contains(":memory:") || status.db_path.is_empty(),
        "expected in-memory path, got: {}",
        status.db_path
    );
    assert!(status.fk_enabled);
}

// ---------- 2. Migration execution ----------

#[test]
fn test_migrations_run_clean() {
    let db = temp_db();
    // Verify all data tables exist and are empty on a fresh database.
    let tables = [
        "domains",
        "resources",
        "domain_resources",
        "relationships",
        "provenance",
        "resource_observations",
        "history",
        "roots",
    ];
    for table in &tables {
        let count: i64 = db
            .conn()
            .query_row(&format!("SELECT COUNT(*) FROM {}", table), [], |r| r.get(0))
            .unwrap_or_else(|e| panic!("table {} missing or inaccessible: {}", table, e));
        assert_eq!(
            count, 0,
            "table {} should be empty on fresh database",
            table
        );
    }
    // All migrations should be recorded on a fresh database (v1..v3).
    let migrations: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        migrations, 3,
        "_migrations should have 3 entries after init"
    );
}

#[test]
fn test_migration_version_tracking() {
    let db = temp_db();
    let version: i64 = db
        .conn()
        .query_row("SELECT MAX(version) FROM _migrations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 3);
}

// ---------- 3. Foreign keys ----------

#[test]
fn test_foreign_keys_enabled() {
    let db = temp_db();
    let fk: bool = db
        .conn()
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert!(fk, "foreign keys must be enabled");
}

#[test]
fn test_foreign_key_constraint_blocks_invalid_reference() {
    let db = temp_db();
    let result = db.conn().execute(
        "INSERT INTO domain_resources (domain_id, resource_id, relationship, created_at, updated_at)
         VALUES ('nonexistent', 'also-nonexistent', 'test', 'now', 'now')",
        [],
    );
    assert!(
        result.is_err(),
        "FK constraint should block insert with nonexistent domain"
    );
}

// ---------- 4. Inserting a domain ----------

#[test]
fn test_insert_domain() {
    let db = temp_db();
    let domain = domains::create(db.conn(), "development", Some("Dev tools")).unwrap();
    assert_eq!(domain.name, "development");
    assert_eq!(domain.description.as_deref(), Some("Dev tools"));
    assert!(!domain.id.is_empty());

    let fetched = domains::get(db.conn(), &domain.id).unwrap().unwrap();
    assert_eq!(fetched.id, domain.id);
    assert_eq!(fetched.name, "development");
}

#[test]
fn test_domain_uniqueness() {
    let db = temp_db();
    domains::create(db.conn(), "dev", None).unwrap();
    let result = domains::create(db.conn(), "dev", None);
    assert!(result.is_err(), "duplicate domain name should fail");
}

// ---------- 5. Inserting a resource ----------

#[test]
fn test_insert_resource() {
    let db = temp_db();
    let res = resources::create(
        db.conn(),
        ResourceType::Package,
        "python3",
        Some("Python 3 interpreter"),
    )
    .unwrap();
    assert_eq!(res.resource_type, ResourceType::Package);
    assert_eq!(res.native_id, "python3");
    assert_eq!(res.display_name.as_deref(), Some("Python 3 interpreter"));

    let fetched = resources::get(db.conn(), &res.id).unwrap().unwrap();
    assert_eq!(fetched.id, res.id);
}

#[test]
fn test_resource_native_id_uniqueness() {
    let db = temp_db();
    resources::create(db.conn(), ResourceType::Package, "vim", None).unwrap();
    let result = resources::create(db.conn(), ResourceType::Package, "vim", None);
    assert!(result.is_err(), "duplicate (type, native_id) should fail");
}

#[test]
fn test_different_types_same_native_id() {
    let db = temp_db();
    resources::create(db.conn(), ResourceType::Package, "firefox", None).unwrap();
    resources::create(db.conn(), ResourceType::Flatpak, "firefox", None).unwrap();

    let pkgs = resources::list_by_type(db.conn(), ResourceType::Package).unwrap();
    let flats = resources::list_by_type(db.conn(), ResourceType::Flatpak).unwrap();
    assert_eq!(pkgs.len(), 1);
    assert_eq!(flats.len(), 1);
}

// ---------- 6. Creating a relationship ----------

#[test]
fn test_create_relationship() {
    let db = temp_db();
    let a = resources::create(db.conn(), ResourceType::Package, "pkg-a", None).unwrap();
    let b = resources::create(db.conn(), ResourceType::Package, "pkg-b", None).unwrap();

    let rel = relationships::create(
        db.conn(),
        &a.id,
        RelationshipType::DependsOn,
        &b.id,
        RelationshipOrigin::System,
    )
    .unwrap();

    assert_eq!(rel.source_id, a.id);
    assert_eq!(rel.target_id, b.id);
    assert_eq!(rel.relationship_type, RelationshipType::DependsOn);
    assert_eq!(rel.origin, RelationshipOrigin::System);

    let fetched = relationships::get(db.conn(), &rel.id).unwrap().unwrap();
    assert_eq!(fetched.id, rel.id);
}

#[test]
fn test_relationship_uniqueness() {
    let db = temp_db();
    let a = resources::create(db.conn(), ResourceType::Package, "x", None).unwrap();
    let b = resources::create(db.conn(), ResourceType::Package, "y", None).unwrap();

    relationships::create(
        db.conn(),
        &a.id,
        RelationshipType::DependsOn,
        &b.id,
        RelationshipOrigin::System,
    )
    .unwrap();
    let result = relationships::create(
        db.conn(),
        &a.id,
        RelationshipType::DependsOn,
        &b.id,
        RelationshipOrigin::Derived,
    );
    assert!(result.is_err(), "duplicate relationship should fail");
}

#[test]
fn test_relationship_cascade_delete() {
    let db = temp_db();
    let a = resources::create(db.conn(), ResourceType::Package, "a", None).unwrap();
    let b = resources::create(db.conn(), ResourceType::Package, "b", None).unwrap();
    let _rel = relationships::create(
        db.conn(),
        &a.id,
        RelationshipType::DependsOn,
        &b.id,
        RelationshipOrigin::User,
    )
    .unwrap();

    assert_eq!(relationships::count(db.conn()).unwrap(), 1);
    resources::delete(db.conn(), &a.id).unwrap();
    assert_eq!(
        relationships::count(db.conn()).unwrap(),
        0,
        "FK cascade should remove relationship"
    );
}

// ---------- 7. Foreign-key deletion behavior ----------

#[test]
fn test_domain_cascade_deletes_domain_resources() {
    let db = temp_db();
    let domain = domains::create(db.conn(), "test-domain", None).unwrap();
    let res = resources::create(db.conn(), ResourceType::Package, "pkg", None).unwrap();

    // Manually insert into domain_resources (the link table).
    db.conn().execute(
        "INSERT INTO domain_resources (domain_id, resource_id, relationship, created_at, updated_at)
         VALUES (?1, ?2, 'assigned', 'now', 'now')",
        rusqlite::params![domain.id, res.id],
    )
    .unwrap();

    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM domain_resources", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);

    // Deleting the domain should cascade to domain_resources.
    domains::delete(db.conn(), &domain.id).unwrap();

    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM domain_resources", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0, "FK cascade should remove domain_resources");
}

#[test]
fn test_resource_cascade_deletes_provenance() {
    let db = temp_db();
    let res = resources::create(db.conn(), ResourceType::Package, "test-pkg", None).unwrap();
    provenance::upsert(
        db.conn(),
        &res.id,
        Some("dnf"),
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    assert!(provenance::get(db.conn(), &res.id).unwrap().is_some());

    resources::delete(db.conn(), &res.id).unwrap();
    assert!(
        provenance::get(db.conn(), &res.id).unwrap().is_none(),
        "FK cascade should remove provenance"
    );
}

#[test]
fn test_resource_cascade_deletes_observations() {
    let db = temp_db();
    let res = resources::create(db.conn(), ResourceType::Service, "myservice", None).unwrap();
    observations::upsert(
        db.conn(),
        &res.id,
        Some(true),
        Some("1.0"),
        None,
        None,
        None,
        None,
    )
    .unwrap();
    assert!(observations::get(db.conn(), &res.id).unwrap().is_some());

    resources::delete(db.conn(), &res.id).unwrap();
    assert!(
        observations::get(db.conn(), &res.id).unwrap().is_none(),
        "FK cascade should remove observations"
    );
}

// ---------- 8. Reopening the database ----------

#[test]
fn test_database_reopen_persists_data() {
    use tempfile::NamedTempFile;

    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_path_buf();

    // Write data.
    {
        let db = Database::open(&path).unwrap();
        domains::create(db.conn(), "persist-test", Some("should survive reopen")).unwrap();
        let res = resources::create(db.conn(), ResourceType::Package, "persist-pkg", None).unwrap();
        relationships::create(
            db.conn(),
            &res.id,
            RelationshipType::Provides,
            &res.id,
            RelationshipOrigin::User,
        )
        .unwrap();
    }

    // Reopen and verify.
    {
        let db = Database::open(&path).unwrap();
        let domains_list = domains::list(db.conn()).unwrap();
        assert_eq!(domains_list.len(), 1);
        assert_eq!(domains_list[0].name, "persist-test");

        let res_list = resources::list(db.conn()).unwrap();
        assert_eq!(res_list.len(), 1);

        let rel_list = relationships::list(db.conn()).unwrap();
        assert_eq!(rel_list.len(), 1);
    }
}

// ---------- Transaction support ----------

#[test]
fn test_transaction_commit() {
    let db = temp_db();
    db.transaction(|tx| {
        domains::create(tx, "committed-domain", None)?;
        Ok(())
    })
    .unwrap();

    let domains_list = domains::list(db.conn()).unwrap();
    assert_eq!(domains_list.len(), 1);
    assert_eq!(domains_list[0].name, "committed-domain");
}

#[test]
fn test_transaction_rollback() {
    let db = temp_db();
    let result: Result<(), chapeau::errors::ChapeauError> = db.transaction(|tx| {
        domains::create(tx, "will-rollback", None)?;
        Err(chapeau::errors::ChapeauError::ResourceNotFound(
            "forced rollback".into(),
        ))
    });
    assert!(result.is_err());

    let domains_list = domains::list(db.conn()).unwrap();
    assert!(
        domains_list.is_empty(),
        "rolled-back transaction should not persist"
    );
}

// ---------- Provenance ----------

#[test]
fn test_provenance_upsert_and_get() {
    let db = temp_db();
    let res = resources::create(db.conn(), ResourceType::Package, "nginx", None).unwrap();

    let prov = provenance::upsert(
        db.conn(),
        &res.id,
        Some("dnf"),
        Some("2025-01-15T10:00:00Z"),
        Some("txn-123"),
        Some("repository"),
        Some("fedora"),
        None,
    )
    .unwrap();

    assert_eq!(prov.installation_method.as_deref(), Some("dnf"));
    assert_eq!(prov.transaction_id.as_deref(), Some("txn-123"));

    let fetched = provenance::get(db.conn(), &res.id).unwrap().unwrap();
    assert_eq!(fetched.source_type.as_deref(), Some("repository"));
}

// ---------- Observations ----------

#[test]
fn test_observation_upsert_and_get() {
    let db = temp_db();
    let res = resources::create(db.conn(), ResourceType::Service, "sshd", None).unwrap();

    let obs = observations::upsert(
        db.conn(),
        &res.id,
        Some(true),
        Some("9.6"),
        Some(true),
        Some(true),
        Some(false),
        None,
    )
    .unwrap();

    assert_eq!(obs.installed, Some(true));
    assert_eq!(obs.version.as_deref(), Some("9.6"));
    assert_eq!(obs.failed, Some(false));
}

// ---------- History ----------

#[test]
fn test_history_insert_and_list() {
    let db = temp_db();
    let entry = history::insert(
        db.conn(),
        "resource_added",
        Some("package"),
        None,
        None,
        Some("added vim"),
    )
    .unwrap();
    assert_eq!(entry.action, "resource_added");
    assert!(entry.id > 0);

    let entries = history::list(db.conn(), 10).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].details.as_deref(), Some("added vim"));
}

// ---------- Counts ----------

#[test]
fn test_counts() {
    let db = temp_db();
    assert_eq!(resources::count(db.conn()).unwrap(), 0);
    assert_eq!(relationships::count(db.conn()).unwrap(), 0);
    assert_eq!(provenance::count(db.conn()).unwrap(), 0);
    assert_eq!(observations::count(db.conn()).unwrap(), 0);
    assert_eq!(history::count(db.conn()).unwrap(), 0);

    resources::create(db.conn(), ResourceType::Package, "a", None).unwrap();
    resources::create(db.conn(), ResourceType::Package, "b", None).unwrap();
    assert_eq!(resources::count(db.conn()).unwrap(), 2);
}

// ========== Phase 8: New Storage Functions ==========

// ---------- resources::find_by_native_id ----------

#[test]
fn test_find_by_native_id_prefers_user_facing_types() {
    let db = temp_db();
    let pkg =
        resources::create(db.conn(), ResourceType::Package, "vim", Some("Vim editor")).unwrap();
    resources::create(db.conn(), ResourceType::Repository, "vim", Some("Vim repo")).unwrap();
    resources::create(db.conn(), ResourceType::Flatpak, "vim", Some("Vim flatpak")).unwrap();

    let found = resources::find_by_native_id(db.conn(), "vim").unwrap();
    assert_eq!(found.unwrap().id, pkg.id, "package wins over other types");
}

#[test]
fn test_find_by_native_id_returns_none_when_missing() {
    let db = temp_db();
    resources::create(db.conn(), ResourceType::Package, "vim", None).unwrap();
    let found = resources::find_by_native_id(db.conn(), "nonexistent").unwrap();
    assert!(found.is_none());
}

// ---------- resources::find_all_by_native_id ----------

#[test]
fn test_find_all_by_native_id_returns_all_matches() {
    let db = temp_db();
    let r1 = resources::create(db.conn(), ResourceType::Package, "firefox", None).unwrap();
    let r2 = resources::create(db.conn(), ResourceType::Flatpak, "firefox", None).unwrap();

    let all = resources::find_all_by_native_id(db.conn(), "firefox").unwrap();
    assert_eq!(all.len(), 2);
    let ids: Vec<&str> = all.iter().map(|r| r.id.as_str()).collect();
    assert!(ids.contains(&r1.id.as_str()));
    assert!(ids.contains(&r2.id.as_str()));
}

#[test]
fn test_find_all_by_native_id_returns_empty_when_missing() {
    let db = temp_db();
    let all = resources::find_all_by_native_id(db.conn(), "nope").unwrap();
    assert!(all.is_empty());
}

// ---------- relationships::neighbors ----------

#[test]
fn test_neighbors_returns_both_incoming_and_outgoing() {
    let db = temp_db();
    let a = resources::create(db.conn(), ResourceType::Package, "a", None).unwrap();
    let b = resources::create(db.conn(), ResourceType::Package, "b", None).unwrap();
    let c = resources::create(db.conn(), ResourceType::Package, "c", None).unwrap();

    // a -> b (DependsOn)
    relationships::create(
        db.conn(),
        &a.id,
        RelationshipType::DependsOn,
        &b.id,
        RelationshipOrigin::System,
    )
    .unwrap();
    // c -> a (DependsOn)
    relationships::create(
        db.conn(),
        &c.id,
        RelationshipType::DependsOn,
        &a.id,
        RelationshipOrigin::User,
    )
    .unwrap();

    let neighbors = relationships::neighbors(db.conn(), &a.id).unwrap();
    assert_eq!(neighbors.len(), 2);

    let as_source: Vec<_> = neighbors.iter().filter(|r| r.source_id == a.id).collect();
    let as_target: Vec<_> = neighbors.iter().filter(|r| r.target_id == a.id).collect();
    assert_eq!(as_source.len(), 1, "a should be source in 1 relationship");
    assert_eq!(as_target.len(), 1, "a should be target in 1 relationship");
}

#[test]
fn test_neighbors_returns_empty_for_isolated_resource() {
    let db = temp_db();
    let a = resources::create(db.conn(), ResourceType::Package, "lonely", None).unwrap();
    let neighbors = relationships::neighbors(db.conn(), &a.id).unwrap();
    assert!(neighbors.is_empty());
}

// ---------- domains::list_resources ----------

#[test]
fn test_domain_list_resources_with_owns_and_uses() {
    let db = temp_db();
    let domain = domains::create(db.conn(), "dev", None).unwrap();
    let pkg_a = resources::create(db.conn(), ResourceType::Package, "gcc", None).unwrap();
    let pkg_b = resources::create(db.conn(), ResourceType::Package, "make", None).unwrap();
    let pkg_c = resources::create(db.conn(), ResourceType::Package, "unrelated", None).unwrap();

    // Use domain_resources table (the proper way to link domains to resources)
    db.conn().execute(
        "INSERT INTO domain_resources (domain_id, resource_id, relationship, created_at, updated_at)
         VALUES (?1, ?2, 'owns', datetime('now'), datetime('now'))",
        rusqlite::params![domain.id, pkg_a.id],
    ).unwrap();
    db.conn().execute(
        "INSERT INTO domain_resources (domain_id, resource_id, relationship, created_at, updated_at)
         VALUES (?1, ?2, 'uses', datetime('now'), datetime('now'))",
        rusqlite::params![domain.id, pkg_b.id],
    ).unwrap();
    // pkg_c is not linked to the domain
    let _ = pkg_c;

    // list_resources queries domain_resources, not the relationships table
    let resources = domains::list_resources(db.conn(), &domain.id).unwrap();
    assert_eq!(resources.len(), 2, "should return both owns and uses");

    let owns: Vec<_> = resources
        .iter()
        .filter(|(_, rt)| *rt == RelationshipType::Owns)
        .collect();
    let uses: Vec<_> = resources
        .iter()
        .filter(|(_, rt)| *rt == RelationshipType::Uses)
        .collect();
    assert_eq!(owns.len(), 1);
    assert_eq!(uses.len(), 1);
    assert_eq!(owns[0].0.native_id, "gcc");
    assert_eq!(uses[0].0.native_id, "make");
}

#[test]
fn test_domain_list_resources_empty_domain() {
    let db = temp_db();
    let domain = domains::create(db.conn(), "empty", None).unwrap();
    let resources = domains::list_resources(db.conn(), &domain.id).unwrap();
    assert!(resources.is_empty());
}

// ---------- domains::list_for_resource ----------

#[test]
fn test_domain_list_for_resource() {
    let db = temp_db();
    let databases = domains::create(db.conn(), "databases", None).unwrap();
    let backend = domains::create(db.conn(), "backend", None).unwrap();
    let other = domains::create(db.conn(), "other", None).unwrap();
    let postgres =
        resources::create(db.conn(), ResourceType::Package, "postgresql-server", None).unwrap();
    let unrelated = resources::create(db.conn(), ResourceType::Package, "zsh", None).unwrap();

    db.conn()
        .execute(
            "INSERT INTO domain_resources (domain_id, resource_id, relationship, created_at, updated_at)
             VALUES (?1, ?2, 'owns', datetime('now'), datetime('now'))",
            rusqlite::params![databases.id, postgres.id],
        )
        .unwrap();
    db.conn()
        .execute(
            "INSERT INTO domain_resources (domain_id, resource_id, relationship, created_at, updated_at)
             VALUES (?1, ?2, 'uses', datetime('now'), datetime('now'))",
            rusqlite::params![backend.id, postgres.id],
        )
        .unwrap();
    db.conn()
        .execute(
            "INSERT INTO domain_resources (domain_id, resource_id, relationship, created_at, updated_at)
             VALUES (?1, ?2, 'owns', datetime('now'), datetime('now'))",
            rusqlite::params![other.id, unrelated.id],
        )
        .unwrap();

    let memberships = domains::list_for_resource(db.conn(), &postgres.id).unwrap();
    assert_eq!(memberships.len(), 2, "postgres belongs to two domains");
    // Ordered by domain name: backend, then databases.
    assert_eq!(memberships[0].0.name, "backend");
    assert_eq!(memberships[0].1, RelationshipType::Uses);
    assert_eq!(memberships[1].0.name, "databases");
    assert_eq!(memberships[1].1, RelationshipType::Owns);

    let none = domains::list_for_resource(db.conn(), "nonexistent").unwrap();
    assert!(none.is_empty());
}

// ---------- domains::add_resource / remove_resource ----------

#[test]
fn test_domain_add_and_remove_resource() {
    let db = temp_db();
    let domain = domains::create(db.conn(), "databases", None).unwrap();
    let pkg =
        resources::create(db.conn(), ResourceType::Package, "postgresql-server", None).unwrap();

    assert!(
        !domains::has_resource(db.conn(), &domain.id, &pkg.id, RelationshipType::Owns).unwrap()
    );
    domains::add_resource(
        db.conn(),
        &domain.id,
        &pkg.id,
        RelationshipType::Owns,
        Some("primary database"),
    )
    .unwrap();

    assert!(domains::has_resource(db.conn(), &domain.id, &pkg.id, RelationshipType::Owns).unwrap());
    assert!(
        !domains::has_resource(db.conn(), &domain.id, &pkg.id, RelationshipType::Uses).unwrap()
    );

    let memberships = domains::list_for_resource(db.conn(), &pkg.id).unwrap();
    assert_eq!(memberships.len(), 1);
    assert_eq!(memberships[0].1, RelationshipType::Owns);

    // Re-adding updates rather than duplicating.
    domains::add_resource(
        db.conn(),
        &domain.id,
        &pkg.id,
        RelationshipType::Owns,
        Some("updated reason"),
    )
    .unwrap();
    assert_eq!(
        domains::list_resources(db.conn(), &domain.id)
            .unwrap()
            .len(),
        1
    );

    assert!(domains::remove_resource(db.conn(), &domain.id, &pkg.id).unwrap());
    assert!(domains::list_for_resource(db.conn(), &pkg.id)
        .unwrap()
        .is_empty());
    assert!(!domains::remove_resource(db.conn(), &domain.id, &pkg.id).unwrap());
}

// ========== Phase 8: SystemGraph Tests ==========

#[test]
fn test_system_graph_from_db_builds_correctly() {
    let db = temp_db();
    let pkg_a =
        resources::create(db.conn(), ResourceType::Package, "a", Some("Package A")).unwrap();
    let pkg_b =
        resources::create(db.conn(), ResourceType::Package, "b", Some("Package B")).unwrap();
    relationships::create(
        db.conn(),
        &pkg_a.id,
        RelationshipType::DependsOn,
        &pkg_b.id,
        RelationshipOrigin::System,
    )
    .unwrap();

    let graph = SystemGraph::from_db(&db).unwrap();
    // 2 resource nodes
    assert_eq!(graph.graph.node_count(), 2);
    // 1 edge
    assert_eq!(graph.graph.edge_count(), 1);
}

#[test]
fn test_system_graph_includes_domains() {
    let db = temp_db();
    let domain = domains::create(db.conn(), "mydomain", None).unwrap();
    let pkg = resources::create(db.conn(), ResourceType::Package, "pkg", None).unwrap();
    // Use domain_resources table for domain-resource association
    db.conn().execute(
        "INSERT INTO domain_resources (domain_id, resource_id, relationship, created_at, updated_at)
         VALUES (?1, ?2, 'owns', datetime('now'), datetime('now'))",
        rusqlite::params![domain.id, pkg.id],
    ).unwrap();

    let graph = SystemGraph::from_db(&db).unwrap();
    // 1 domain + 1 resource = 2 nodes, no edges from relationships table
    assert_eq!(graph.graph.node_count(), 2);
    assert_eq!(graph.graph.edge_count(), 0);
}

#[test]
fn test_system_graph_node_index_lookup() {
    let db = temp_db();
    let pkg = resources::create(db.conn(), ResourceType::Package, "lookup-test", None).unwrap();

    let graph = SystemGraph::from_db(&db).unwrap();
    let idx = graph.node_index(&pkg.id);
    assert!(idx.is_some());
    let idx = idx.unwrap();
    assert_eq!(graph.graph[idx].id, pkg.id);
}

#[test]
fn test_system_graph_node_index_missing_returns_none() {
    let db = temp_db();
    let graph = SystemGraph::from_db(&db).unwrap();
    assert!(graph.node_index("nonexistent-uuid").is_none());
}

#[test]
fn test_system_graph_find_node_by_label() {
    let db = temp_db();
    resources::create(
        db.conn(),
        ResourceType::Package,
        "my-pkg",
        Some("My Package"),
    )
    .unwrap();

    let graph = SystemGraph::from_db(&db).unwrap();
    let idx = graph.find_node("My Package");
    assert!(idx.is_some());
    assert_eq!(graph.graph[idx.unwrap()].label, "My Package");
}

#[test]
fn test_system_graph_find_node_missing() {
    let db = temp_db();
    let graph = SystemGraph::from_db(&db).unwrap();
    assert!(graph.find_node("nothing-here").is_none());
}

#[test]
fn test_system_graph_outgoing_edges() {
    let db = temp_db();
    let a = resources::create(db.conn(), ResourceType::Package, "a", Some("A")).unwrap();
    let b = resources::create(db.conn(), ResourceType::Package, "b", Some("B")).unwrap();
    let c = resources::create(db.conn(), ResourceType::Package, "c", Some("C")).unwrap();
    // a -> b, a -> c
    relationships::create(
        db.conn(),
        &a.id,
        RelationshipType::DependsOn,
        &b.id,
        RelationshipOrigin::System,
    )
    .unwrap();
    relationships::create(
        db.conn(),
        &a.id,
        RelationshipType::Uses,
        &c.id,
        RelationshipOrigin::User,
    )
    .unwrap();

    let graph = SystemGraph::from_db(&db).unwrap();
    let idx_a = graph.node_index(&a.id).unwrap();
    let outgoing = graph.outgoing(idx_a);

    assert_eq!(outgoing.len(), 2);
    let targets: Vec<&str> = outgoing
        .iter()
        .map(|(_, _, label)| label.label.as_str())
        .collect();
    assert!(targets.contains(&"B"));
    assert!(targets.contains(&"C"));
}

#[test]
fn test_system_graph_incoming_edges() {
    let db = temp_db();
    let a = resources::create(db.conn(), ResourceType::Package, "a", Some("A")).unwrap();
    let b = resources::create(db.conn(), ResourceType::Package, "b", Some("B")).unwrap();
    let c = resources::create(db.conn(), ResourceType::Package, "c", Some("C")).unwrap();
    // b -> a, c -> a
    relationships::create(
        db.conn(),
        &b.id,
        RelationshipType::DependsOn,
        &a.id,
        RelationshipOrigin::System,
    )
    .unwrap();
    relationships::create(
        db.conn(),
        &c.id,
        RelationshipType::DependsOn,
        &a.id,
        RelationshipOrigin::System,
    )
    .unwrap();

    let graph = SystemGraph::from_db(&db).unwrap();
    let idx_a = graph.node_index(&a.id).unwrap();
    let incoming = graph.incoming(idx_a);

    assert_eq!(incoming.len(), 2);
    let sources: Vec<&str> = incoming
        .iter()
        .map(|(_, _, label)| label.label.as_str())
        .collect();
    assert!(sources.contains(&"B"));
    assert!(sources.contains(&"C"));
}

#[test]
fn test_system_graph_neighbors_combines_both_directions() {
    let db = temp_db();
    let a = resources::create(db.conn(), ResourceType::Package, "a", Some("A")).unwrap();
    let b = resources::create(db.conn(), ResourceType::Package, "b", Some("B")).unwrap();
    let c = resources::create(db.conn(), ResourceType::Package, "c", Some("C")).unwrap();
    // a -> b
    relationships::create(
        db.conn(),
        &a.id,
        RelationshipType::DependsOn,
        &b.id,
        RelationshipOrigin::System,
    )
    .unwrap();
    // c -> a
    relationships::create(
        db.conn(),
        &c.id,
        RelationshipType::Uses,
        &a.id,
        RelationshipOrigin::User,
    )
    .unwrap();

    let graph = SystemGraph::from_db(&db).unwrap();
    let idx_a = graph.node_index(&a.id).unwrap();
    let neighbors = graph.neighbors(idx_a);

    assert_eq!(neighbors.len(), 2);
    let labels: Vec<&str> = neighbors
        .iter()
        .map(|(_, _, label)| label.label.as_str())
        .collect();
    assert!(labels.contains(&"B"));
    assert!(labels.contains(&"C"));
}

#[test]
fn test_system_graph_empty_database() {
    let db = temp_db();
    let graph = SystemGraph::from_db(&db).unwrap();
    assert_eq!(graph.graph.node_count(), 0);
    assert_eq!(graph.graph.edge_count(), 0);
}

// ========== Phase 9: System Query Tests ==========

#[test]
fn test_list_by_type_returns_only_matching() {
    let db = temp_db();
    resources::create(db.conn(), ResourceType::Package, "gcc", Some("GCC")).unwrap();
    resources::create(db.conn(), ResourceType::Service, "sshd", Some("SSH Daemon")).unwrap();
    resources::create(
        db.conn(),
        ResourceType::Flatpak,
        "org.gnome.Terminal",
        Some("Terminal"),
    )
    .unwrap();
    resources::create(
        db.conn(),
        ResourceType::Repository,
        "fedora",
        Some("Fedora"),
    )
    .unwrap();

    let pkgs = resources::list_by_type(db.conn(), ResourceType::Package).unwrap();
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].native_id, "gcc");

    let svcs = resources::list_by_type(db.conn(), ResourceType::Service).unwrap();
    assert_eq!(svcs.len(), 1);
    assert_eq!(svcs[0].native_id, "sshd");

    let flatpaks = resources::list_by_type(db.conn(), ResourceType::Flatpak).unwrap();
    assert_eq!(flatpaks.len(), 1);
    assert_eq!(flatpaks[0].native_id, "org.gnome.Terminal");

    let repos = resources::list_by_type(db.conn(), ResourceType::Repository).unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].native_id, "fedora");
}

#[test]
fn test_find_by_native_id_finds_across_types() {
    let db = temp_db();
    resources::create(db.conn(), ResourceType::Service, "sshd", Some("SSH Daemon")).unwrap();
    resources::create(db.conn(), ResourceType::Package, "sshd", None).unwrap();

    let found = resources::find_by_native_id(db.conn(), "sshd").unwrap();
    assert!(found.is_some());
    // Returns the first match (order depends on insertion)
    let r = found.unwrap();
    assert_eq!(r.native_id, "sshd");
}

#[test]
fn test_find_by_native_id_missing() {
    let db = temp_db();
    resources::create(db.conn(), ResourceType::Package, "gcc", None).unwrap();
    let found = resources::find_by_native_id(db.conn(), "sshd").unwrap();
    assert!(found.is_none());
}

#[test]
fn test_list_by_type_empty() {
    let db = temp_db();
    let svcs = resources::list_by_type(db.conn(), ResourceType::Service).unwrap();
    assert!(svcs.is_empty());
}

#[test]
fn test_find_all_by_native_id_multiple_types() {
    let db = temp_db();
    resources::create(db.conn(), ResourceType::Service, "httpd", None).unwrap();
    resources::create(db.conn(), ResourceType::Package, "httpd", None).unwrap();
    let all = resources::find_all_by_native_id(db.conn(), "httpd").unwrap();
    assert_eq!(all.len(), 2);
    let types: Vec<ResourceType> = all.iter().map(|r| r.resource_type).collect();
    assert!(types.contains(&ResourceType::Service));
    assert!(types.contains(&ResourceType::Package));
}

#[test]
fn test_relationships_filtered_by_type() {
    let db = temp_db();
    let pkg_a = resources::create(db.conn(), ResourceType::Package, "a", None).unwrap();
    let pkg_b = resources::create(db.conn(), ResourceType::Package, "b", None).unwrap();
    let pkg_c = resources::create(db.conn(), ResourceType::Package, "c", None).unwrap();

    relationships::create(
        db.conn(),
        &pkg_a.id,
        RelationshipType::DependsOn,
        &pkg_b.id,
        RelationshipOrigin::System,
    )
    .unwrap();
    relationships::create(
        db.conn(),
        &pkg_a.id,
        RelationshipType::Owns,
        &pkg_c.id,
        RelationshipOrigin::User,
    )
    .unwrap();

    // Filter by DependsOn
    let deps = relationships::list_by_type(db.conn(), RelationshipType::DependsOn).unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].target_id, pkg_b.id);

    // Filter by Owns
    let owns = relationships::list_by_type(db.conn(), RelationshipType::Owns).unwrap();
    assert_eq!(owns.len(), 1);
    assert_eq!(owns[0].target_id, pkg_c.id);
}

#[test]
fn test_dependents_via_list_to() {
    let db = temp_db();
    let a = resources::create(db.conn(), ResourceType::Package, "liba", Some("Library A")).unwrap();
    let b = resources::create(db.conn(), ResourceType::Package, "b", Some("B")).unwrap();
    let c = resources::create(db.conn(), ResourceType::Package, "c", Some("C")).unwrap();
    // b depends on a
    relationships::create(
        db.conn(),
        &b.id,
        RelationshipType::DependsOn,
        &a.id,
        RelationshipOrigin::System,
    )
    .unwrap();
    // c depends on a
    relationships::create(
        db.conn(),
        &c.id,
        RelationshipType::DependsOn,
        &a.id,
        RelationshipOrigin::System,
    )
    .unwrap();

    // What depends on a? (incoming edges to a)
    let incoming = relationships::list_to(db.conn(), &a.id).unwrap();
    assert_eq!(incoming.len(), 2);
    let sources: Vec<String> = incoming
        .iter()
        .map(|r| {
            resources::get(db.conn(), &r.source_id)
                .unwrap()
                .unwrap()
                .native_id
                .clone()
        })
        .collect();
    assert!(sources.contains(&"b".to_string()));
    assert!(sources.contains(&"c".to_string()));
}

#[test]
fn test_dependencies_via_list_from() {
    let db = temp_db();
    let a = resources::create(db.conn(), ResourceType::Package, "a", Some("A")).unwrap();
    let b = resources::create(db.conn(), ResourceType::Package, "libb", Some("Library B")).unwrap();
    let c = resources::create(db.conn(), ResourceType::Package, "libc", Some("Library C")).unwrap();
    // a depends on b and c
    relationships::create(
        db.conn(),
        &a.id,
        RelationshipType::DependsOn,
        &b.id,
        RelationshipOrigin::System,
    )
    .unwrap();
    relationships::create(
        db.conn(),
        &a.id,
        RelationshipType::DependsOn,
        &c.id,
        RelationshipOrigin::System,
    )
    .unwrap();

    // What does a depend on? (outgoing edges from a)
    let outgoing = relationships::list_from(db.conn(), &a.id).unwrap();
    let deps: Vec<String> = outgoing
        .iter()
        .filter(|r| r.relationship_type == RelationshipType::DependsOn)
        .map(|r| {
            resources::get(db.conn(), &r.target_id)
                .unwrap()
                .unwrap()
                .native_id
                .clone()
        })
        .collect();
    assert_eq!(deps.len(), 2);
    assert!(deps.contains(&"libb".to_string()));
    assert!(deps.contains(&"libc".to_string()));
}

#[test]
fn test_service_resources_listed_by_type() {
    let db = temp_db();
    resources::create(db.conn(), ResourceType::Service, "sshd", Some("SSH Daemon")).unwrap();
    resources::create(
        db.conn(),
        ResourceType::Service,
        "httpd",
        Some("HTTP Daemon"),
    )
    .unwrap();
    resources::create(db.conn(), ResourceType::Package, "gcc", Some("GCC")).unwrap();

    let svcs = resources::list_by_type(db.conn(), ResourceType::Service).unwrap();
    assert_eq!(svcs.len(), 2);
    let svc_ids: Vec<&str> = svcs.iter().map(|s| s.native_id.as_str()).collect();
    assert!(svc_ids.contains(&"sshd"));
    assert!(svc_ids.contains(&"httpd"));
}

#[test]
fn test_repository_resources_listed_by_type() {
    let db = temp_db();
    resources::create(
        db.conn(),
        ResourceType::Repository,
        "fedora",
        Some("Fedora"),
    )
    .unwrap();
    resources::create(
        db.conn(),
        ResourceType::Repository,
        "rpmfusion",
        Some("RPMFusion"),
    )
    .unwrap();
    resources::create(db.conn(), ResourceType::Package, "gcc", Some("GCC")).unwrap();

    let repos = resources::list_by_type(db.conn(), ResourceType::Repository).unwrap();
    assert_eq!(repos.len(), 2);
    let repo_ids: Vec<&str> = repos.iter().map(|r| r.native_id.as_str()).collect();
    assert!(repo_ids.contains(&"fedora"));
    assert!(repo_ids.contains(&"rpmfusion"));
}

#[test]
fn test_package_count_in_repository() {
    let db = temp_db();
    let repo = resources::create(
        db.conn(),
        ResourceType::Repository,
        "fedora",
        Some("Fedora"),
    )
    .unwrap();
    let pkg_a = resources::create(db.conn(), ResourceType::Package, "gcc", None).unwrap();
    let pkg_b = resources::create(db.conn(), ResourceType::Package, "make", None).unwrap();

    relationships::create(
        db.conn(),
        &pkg_a.id,
        RelationshipType::ComesFrom,
        &repo.id,
        RelationshipOrigin::System,
    )
    .unwrap();
    relationships::create(
        db.conn(),
        &pkg_b.id,
        RelationshipType::ComesFrom,
        &repo.id,
        RelationshipOrigin::System,
    )
    .unwrap();

    let rels = relationships::list_to(db.conn(), &repo.id).unwrap();
    let from_repo = rels
        .iter()
        .filter(|r| r.relationship_type == RelationshipType::ComesFrom)
        .count();
    assert_eq!(from_repo, 2);
}
