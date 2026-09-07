use crate::core::{RelationshipOrigin, RelationshipType, ResourceType};
use crate::discovery::SystemSnapshot;
use crate::errors::Result;
use crate::storage::Database;
use crate::storage::{history, observations, relationships, resources};
use std::collections::HashSet;

use crate::backends::dnf::DnfCliBackend;
use crate::backends::flatpak::FlatpakCliBackend;
use crate::backends::flatpak_backend::FlatpakBackendTrait;
use crate::backends::package_backend::PackageBackend;
use crate::backends::service_backend::ServiceBackend;
use crate::backends::systemd::SystemdDbusBackend;

/// Discover the full system state from all available backends.
pub fn discover() -> Result<SystemSnapshot> {
    let mut snap = SystemSnapshot::empty();

    // DNF packages + repositories
    let dnf = DnfCliBackend::new();
    if dnf.is_available() {
        snap.packages = dnf.discover_installed()?;
        snap.repositories = dnf.discover_repositories()?;
    }

    // systemd units
    let systemd = SystemdDbusBackend::new();
    if systemd.is_available() {
        let unit_snap = systemd.discover_units()?;
        snap.units = unit_snap.units;
    }

    // Flatpak apps + runtimes + remotes
    let flatpak = FlatpakCliBackend::new();
    if flatpak.is_available() {
        let fp_snap = flatpak.discover_snapshot()?;
        snap.flatpak_apps = fp_snap.apps;
        snap.flatpak_runtimes = fp_snap.runtimes;
        snap.flatpak_remotes = fp_snap.remotes;
    }

    snap.snapshot_time = chrono::Utc::now().to_rfc3339();
    Ok(snap)
}

/// Run a full scan: discover system state, reconcile with DB, and commit atomically.
///
/// Returns the discovered snapshot. All discovered resources, observations, and
/// relationships are persisted within a single atomic transaction.
pub fn scan(db: &Database) -> Result<SystemSnapshot> {
    let snapshot = discover()?;

    let mut summary = crate::discovery::ScanSummary::default();
    summary.total_discovered = snapshot.total_count();

    db.transaction(|tx| {
        // --- Packages ---
        for pkg in &snapshot.packages {
            let native_id = &pkg.name;
            let existing = resources::get_by_native(tx, ResourceType::Package, native_id)?;
            let resource = match existing {
                Some(r) => {
                    summary.updated += 1;
                    r
                }
                None => {
                    let r = resources::create_tx(
                        tx,
                        ResourceType::Package,
                        native_id,
                        Some(&pkg.name),
                    )?;
                    summary.created += 1;
                    *summary.by_type.entry("package".into()).or_insert(0) += 1;
                    r
                }
            };

            // Upsert observation
            observations::upsert(
                tx,
                &resource.id,
                Some(true),
                Some(&pkg.version),
                None,
                None,
                None,
                None,
            )?;
            summary.observations += 1;

            // Relationship: package comes_from repository
            if let Some(ref from_repo) = pkg.from_repo {
                // Find or create the repository resource
                let repo_native_id = from_repo;
                let repo_res =
                    match resources::get_by_native(tx, ResourceType::Repository, repo_native_id)? {
                        Some(r) => r,
                        None => {
                            let r = resources::create_tx(
                                tx,
                                ResourceType::Repository,
                                repo_native_id,
                                Some(repo_native_id),
                            )?;
                            *summary.by_type.entry("repository".into()).or_insert(0) += 1;
                            r
                        }
                    };

                // Only create if not already existing
                if relationships::find_existing(
                    tx,
                    &resource.id,
                    RelationshipType::ComesFrom,
                    &repo_res.id,
                )?
                .is_none()
                {
                    relationships::create_tx(
                        tx,
                        &resource.id,
                        RelationshipType::ComesFrom,
                        &repo_res.id,
                        RelationshipOrigin::System,
                    )?;
                    summary.relationships_created += 1;
                }
            }
        }

        // --- Repositories (from DNF, not already created via comes_from) ---
        for repo in &snapshot.repositories {
            let existing = resources::get_by_native(tx, ResourceType::Repository, &repo.id)?;
            if existing.is_none() {
                resources::create_tx(tx, ResourceType::Repository, &repo.id, Some(&repo.name))?;
                summary.created += 1;
                *summary.by_type.entry("repository".into()).or_insert(0) += 1;
            }
        }

        // --- systemd units ---
        for unit in &snapshot.units {
            let native_id = &unit.name;
            let existing = resources::get_by_native(tx, ResourceType::Service, native_id)?;
            let resource = match existing {
                Some(r) => {
                    summary.updated += 1;
                    r
                }
                None => {
                    let r = resources::create_tx(
                        tx,
                        ResourceType::Service,
                        native_id,
                        Some(&unit.description),
                    )?;
                    summary.created += 1;
                    *summary.by_type.entry("service".into()).or_insert(0) += 1;
                    r
                }
            };

            let active = Some(unit.active_state == crate::discovery::services::ActiveState::Active);
            let failed = Some(unit.active_state == crate::discovery::services::ActiveState::Failed);
            let enabled = unit.unit_file_state.as_ref().map(|s| s.is_enabled());
            observations::upsert(tx, &resource.id, None, None, active, enabled, failed, None)?;
            summary.observations += 1;
        }

        // --- Flatpak apps ---
        for app in &snapshot.flatpak_apps {
            let native_id = &app.id;
            let existing = resources::get_by_native(tx, ResourceType::Flatpak, native_id)?;
            let resource = match existing {
                Some(r) => {
                    summary.updated += 1;
                    r
                }
                None => {
                    let r = resources::create_tx(
                        tx,
                        ResourceType::Flatpak,
                        native_id,
                        Some(&app.name),
                    )?;
                    summary.created += 1;
                    *summary.by_type.entry("flatpak".into()).or_insert(0) += 1;
                    r
                }
            };

            let metadata = serde_json::json!({
                "branch": app.branch,
                "arch": app.arch,
                "installation": app.installation,
                "active_commit": app.active_commit,
            });
            observations::upsert(
                tx,
                &resource.id,
                Some(true),
                Some(&app.version),
                None,
                None,
                None,
                Some(&metadata.to_string()),
            )?;
            summary.observations += 1;

            // Relationship: flatpak comes_from remote
            let remote_native_id = &app.origin;
            let remote_res =
                match resources::get_by_native(tx, ResourceType::Repository, remote_native_id)? {
                    Some(r) => r,
                    None => {
                        let r = resources::create_tx(
                            tx,
                            ResourceType::Repository,
                            remote_native_id,
                            Some(remote_native_id),
                        )?;
                        *summary.by_type.entry("repository".into()).or_insert(0) += 1;
                        r
                    }
                };

            if relationships::find_existing(
                tx,
                &resource.id,
                RelationshipType::ComesFrom,
                &remote_res.id,
            )?
            .is_none()
            {
                relationships::create_tx(
                    tx,
                    &resource.id,
                    RelationshipType::ComesFrom,
                    &remote_res.id,
                    RelationshipOrigin::System,
                )?;
                summary.relationships_created += 1;
            }
        }

        // --- Flatpak runtimes ---
        for runtime in &snapshot.flatpak_runtimes {
            let native_id = &runtime.id;
            let existing = resources::get_by_native(tx, ResourceType::Flatpak, native_id)?;
            let resource = match existing {
                Some(r) => {
                    summary.updated += 1;
                    r
                }
                None => {
                    let r = resources::create_tx(
                        tx,
                        ResourceType::Flatpak,
                        native_id,
                        Some(&runtime.name),
                    )?;
                    summary.created += 1;
                    *summary.by_type.entry("flatpak".into()).or_insert(0) += 1;
                    r
                }
            };

            let metadata = serde_json::json!({
                "branch": runtime.branch,
                "arch": runtime.arch,
                "installation": runtime.installation,
                "active_commit": runtime.active_commit,
                "kind": "runtime",
            });
            observations::upsert(
                tx,
                &resource.id,
                Some(true),
                Some(&runtime.version),
                None,
                None,
                None,
                Some(&metadata.to_string()),
            )?;
            summary.observations += 1;
        }

        // --- Flatpak app → runtime relationships ---
        for app in &snapshot.flatpak_apps {
            if let Some(ref runtime_ref) = app.runtime {
                // runtime_ref is like "org.freedesktop.Platform/x86_64/24.08"
                // Extract the base runtime ID (first path component)
                let runtime_id = runtime_ref.split('/').next().unwrap_or(runtime_ref);

                if let Some(app_res) = resources::get_by_native(tx, ResourceType::Flatpak, &app.id)?
                {
                    if let Some(runtime_res) =
                        resources::get_by_native(tx, ResourceType::Flatpak, runtime_id)?
                    {
                        if relationships::find_existing(
                            tx,
                            &app_res.id,
                            RelationshipType::Uses,
                            &runtime_res.id,
                        )?
                        .is_none()
                        {
                            relationships::create_tx(
                                tx,
                                &app_res.id,
                                RelationshipType::Uses,
                                &runtime_res.id,
                                RelationshipOrigin::System,
                            )?;
                            summary.relationships_created += 1;
                        }
                    }
                }
            }
        }

        Ok(snapshot)
    })
}

/// Summary of what reconciliation removed from the database.
#[derive(Debug, Clone, Default)]
pub struct ReconcileSummary {
    /// Number of stale package resources removed.
    pub stale_packages_removed: usize,
    /// Number of observations cleaned (via cascade).
    pub observations_cleaned: usize,
    /// Number of relationships cleaned (via cascade).
    pub relationships_cleaned: usize,
    /// Number of domain associations cleaned (via cascade).
    pub domain_associations_cleaned: usize,
}

impl std::fmt::Display for ReconcileSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.stale_packages_removed == 0 {
            write!(f, "No stale resources found.")
        } else {
            writeln!(
                f,
                "{} stale package(s) removed.",
                self.stale_packages_removed
            )?;
            write!(
                f,
                "Associated observations, relationships, and domain associations cleaned automatically."
            )
        }
    }
}

/// Reconcile the Chapeau database after a DNF removal operation.
///
/// This is a **targeted** reconciliation: it only reconciles Package resources
/// by re-discovering the full installed package list from DNF and comparing
/// against what the database contains.
///
/// Because we discover ALL installed packages (not a partial set), a package's
/// absence from the discovery result means it is genuinely absent from Fedora.
///
/// # Arguments
/// * `db` - The database to reconcile.
/// * `removed_names` - The package names that were requested for removal (used for history recording).
///
/// # Returns
/// A summary of what was cleaned from the database.
pub fn reconcile_after_removal(
    db: &Database,
    removed_names: &[String],
) -> Result<ReconcileSummary> {
    // 1. Discover the current installed package state from DNF.
    let dnf = DnfCliBackend::new();
    let fresh_packages = if dnf.is_available() {
        dnf.discover_installed()?
    } else {
        // If DNF is not available, we cannot reconcile. Return an empty summary
        // rather than corrupting the database.
        return Ok(ReconcileSummary::default());
    };

    // Build a set of currently installed package names for fast lookup.
    let installed_names: HashSet<&str> =
        fresh_packages.iter().map(|pkg| pkg.name.as_str()).collect();

    // 2. Find all Package resources in the database.
    let db_packages = resources::list_by_type(db.conn(), ResourceType::Package)?;

    // 3. Identify stale packages: in DB but not in the live system.
    let stale: Vec<_> = db_packages
        .iter()
        .filter(|r| !installed_names.contains(r.native_id.as_str()))
        .collect();

    if stale.is_empty() {
        // Nothing to reconcile. Record the successful operation with no cleanup needed.
        history::insert(
            db.conn(),
            "remove",
            Some("package"),
            None,
            None,
            Some(&format!(
                "removal requested for {:?}; no stale resources found",
                removed_names
            )),
        )?;
        return Ok(ReconcileSummary::default());
    }

    // 4. Delete stale packages within a single atomic transaction.
    //    FK cascades handle observations, relationships, domain_resources, provenance.
    let mut summary = ReconcileSummary::default();
    let stale_count = stale.len();

    db.transaction(|tx| {
        // Count current observations and relationships before deletion for summary.
        let obs_before: i64 =
            tx.query_row("SELECT COUNT(*) FROM resource_observations", [], |row| {
                row.get(0)
            })?;
        let rel_before: i64 =
            tx.query_row("SELECT COUNT(*) FROM relationships", [], |row| row.get(0))?;
        let dr_before: i64 = tx.query_row("SELECT COUNT(*) FROM domain_resources", [], |row| {
            row.get(0)
        })?;

        // Delete each stale package (triggers cascade).
        for pkg in &stale {
            resources::delete(tx, &pkg.id)?;
        }

        // Count after deletion to determine what cascaded.
        let obs_after: i64 =
            tx.query_row("SELECT COUNT(*) FROM resource_observations", [], |row| {
                row.get(0)
            })?;
        let rel_after: i64 =
            tx.query_row("SELECT COUNT(*) FROM relationships", [], |row| row.get(0))?;
        let dr_after: i64 = tx.query_row("SELECT COUNT(*) FROM domain_resources", [], |row| {
            row.get(0)
        })?;

        summary.stale_packages_removed = stale_count;
        summary.observations_cleaned = (obs_before - obs_after) as usize;
        summary.relationships_cleaned = (rel_before - rel_after) as usize;
        summary.domain_associations_cleaned = (dr_before - dr_after) as usize;

        // Record the removal in history.
        let stale_names: Vec<&str> = stale.iter().map(|r| r.native_id.as_str()).collect();
        history::insert(
            tx,
            "remove",
            Some("package"),
            None,
            None,
            Some(&format!(
                "removed {:?}; reconciled {} stale package(s) ({})",
                removed_names,
                stale_count,
                stale_names.join(", ")
            )),
        )?;

        Ok(())
    })?;

    Ok(summary)
}

/// Reconcile packages using a pre-built snapshot (for testing or when discovery
/// has already been performed).
///
/// Same logic as `reconcile_after_removal` but takes an explicit list of
/// installed package names instead of querying DNF.
#[cfg(test)]
pub(crate) fn reconcile_with_snapshot(
    db: &Database,
    installed_names: &[String],
) -> Result<ReconcileSummary> {
    let installed: HashSet<&str> = installed_names.iter().map(|s| s.as_str()).collect();

    let db_packages = resources::list_by_type(db.conn(), ResourceType::Package)?;

    let stale: Vec<_> = db_packages
        .iter()
        .filter(|r| !installed.contains(r.native_id.as_str()))
        .collect();

    if stale.is_empty() {
        return Ok(ReconcileSummary::default());
    }

    let stale_count = stale.len();
    let mut summary = ReconcileSummary::default();

    db.transaction(|tx| {
        let obs_before: i64 =
            tx.query_row("SELECT COUNT(*) FROM resource_observations", [], |row| {
                row.get(0)
            })?;
        let rel_before: i64 =
            tx.query_row("SELECT COUNT(*) FROM relationships", [], |row| row.get(0))?;
        let dr_before: i64 = tx.query_row("SELECT COUNT(*) FROM domain_resources", [], |row| {
            row.get(0)
        })?;

        for pkg in &stale {
            resources::delete(tx, &pkg.id)?;
        }

        let obs_after: i64 =
            tx.query_row("SELECT COUNT(*) FROM resource_observations", [], |row| {
                row.get(0)
            })?;
        let rel_after: i64 =
            tx.query_row("SELECT COUNT(*) FROM relationships", [], |row| row.get(0))?;
        let dr_after: i64 = tx.query_row("SELECT COUNT(*) FROM domain_resources", [], |row| {
            row.get(0)
        })?;

        summary.stale_packages_removed = stale_count;
        summary.observations_cleaned = (obs_before - obs_after) as usize;
        summary.relationships_cleaned = (rel_before - rel_after) as usize;
        summary.domain_associations_cleaned = (dr_before - dr_after) as usize;

        Ok(())
    })?;

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_returns_snapshot() {
        let snap = discover().unwrap();
        // We're on a real system, so at least some packages should exist.
        assert!(!snap.packages.is_empty() || !snap.units.is_empty());
    }

    #[test]
    fn scan_with_memory_db() {
        let db = Database::open_memory().unwrap();
        let snap = scan(&db).unwrap();
        // Should have discovered something on a real system.
        assert!(snap.total_count() > 0);

        // Verify resources were persisted.
        let res_list = resources::list(db.conn()).unwrap();
        assert!(!res_list.is_empty());

        // Verify observations were persisted.
        let obs_count = observations::count(db.conn()).unwrap();
        assert!(obs_count > 0);
    }

    #[test]
    fn scan_is_idempotent() {
        let db = Database::open_memory().unwrap();
        let _snap1 = scan(&db).unwrap();
        let count1 = resources::count(db.conn()).unwrap();

        // Scan again — should not create new resources.
        let _snap2 = scan(&db).unwrap();
        let count2 = resources::count(db.conn()).unwrap();

        assert_eq!(count1, count2);
    }

    // ===== Phase 12: Reconciliation Tests =====

    /// Helper: insert a Package resource directly into the DB.
    fn insert_package(db: &Database, name: &str) -> crate::core::Resource {
        resources::create(db.conn(), ResourceType::Package, name, Some(name)).unwrap()
    }

    /// Helper: insert an observation for a resource.
    fn insert_observation(db: &Database, resource_id: &str) {
        observations::upsert(
            db.conn(),
            resource_id,
            Some(true),
            Some("1.0.0"),
            None,
            None,
            None,
            None,
        )
        .unwrap();
    }

    /// Helper: insert a relationship between two resources.
    fn insert_relationship(
        db: &Database,
        source_id: &str,
        target_id: &str,
    ) -> crate::core::Relationship {
        relationships::create(
            db.conn(),
            source_id,
            RelationshipType::DependsOn,
            target_id,
            RelationshipOrigin::User,
        )
        .unwrap()
    }

    /// Helper: insert a domain_resources entry (owns) for a resource.
    fn insert_domain_resource(db: &Database, domain_id: &str, resource_id: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        db.conn()
            .execute(
                "INSERT INTO domain_resources (domain_id, resource_id, relationship, created_at, updated_at)
                 VALUES (?1, ?2, 'owns', ?3, ?4)",
                rusqlite::params![domain_id, resource_id, now, now],
            )
            .unwrap();
    }

    #[test]
    fn reconcile_removes_stale_package() {
        let db = Database::open_memory().unwrap();

        // DB has packages: firefox, vim, git
        let pkg1 = insert_package(&db, "firefox");
        let pkg2 = insert_package(&db, "vim");
        let pkg3 = insert_package(&db, "git");
        insert_observation(&db, &pkg1.id);
        insert_observation(&db, &pkg2.id);
        insert_observation(&db, &pkg3.id);

        // Snapshot says only firefox and git are installed (vim was removed).
        let installed = vec!["firefox".to_string(), "git".to_string()];
        let summary = reconcile_with_snapshot(&db, &installed).unwrap();

        assert_eq!(summary.stale_packages_removed, 1);
        assert_eq!(summary.observations_cleaned, 1);

        // Verify vim is gone, others remain.
        let remaining = resources::list_by_type(db.conn(), ResourceType::Package).unwrap();
        let names: Vec<&str> = remaining.iter().map(|r| r.native_id.as_str()).collect();
        assert!(names.contains(&"firefox"));
        assert!(names.contains(&"git"));
        assert!(!names.contains(&"vim"));
    }

    #[test]
    fn reconcile_removes_multiple_stale_packages() {
        let db = Database::open_memory().unwrap();

        // DB has 4 packages.
        let pkg1 = insert_package(&db, "firefox");
        let pkg2 = insert_package(&db, "vim");
        let pkg3 = insert_package(&db, "git");
        let pkg4 = insert_package(&db, "curl");
        insert_observation(&db, &pkg1.id);
        insert_observation(&db, &pkg2.id);
        insert_observation(&db, &pkg3.id);
        insert_observation(&db, &pkg4.id);

        // Snapshot says only firefox and curl remain (vim, git removed).
        let installed = vec!["firefox".to_string(), "curl".to_string()];
        let summary = reconcile_with_snapshot(&db, &installed).unwrap();

        assert_eq!(summary.stale_packages_removed, 2);
        assert_eq!(summary.observations_cleaned, 2);

        let remaining = resources::list_by_type(db.conn(), ResourceType::Package).unwrap();
        let names: Vec<&str> = remaining.iter().map(|r| r.native_id.as_str()).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"firefox"));
        assert!(names.contains(&"curl"));
    }

    #[test]
    fn reconcile_idempotent_when_no_stale() {
        let db = Database::open_memory().unwrap();

        let pkg1 = insert_package(&db, "firefox");
        let pkg2 = insert_package(&db, "vim");
        insert_observation(&db, &pkg1.id);
        insert_observation(&db, &pkg2.id);

        // Snapshot matches DB exactly.
        let installed = vec!["firefox".to_string(), "vim".to_string()];
        let summary = reconcile_with_snapshot(&db, &installed).unwrap();

        assert_eq!(summary.stale_packages_removed, 0);
        assert_eq!(summary.observations_cleaned, 0);

        // Nothing was deleted.
        let remaining = resources::list_by_type(db.conn(), ResourceType::Package).unwrap();
        assert_eq!(remaining.len(), 2);
    }

    #[test]
    fn reconcile_cleans_associated_data() {
        let db = Database::open_memory().unwrap();

        // Create two packages and a repository.
        let pkg = insert_package(&db, "firefox");
        let repo = resources::create(
            db.conn(),
            ResourceType::Repository,
            "fedora",
            Some("Fedora"),
        )
        .unwrap();
        let _other_pkg = insert_package(&db, "vim");

        // Add observation for the stale package.
        insert_observation(&db, &pkg.id);

        // Add relationship: pkg depends_on repo.
        insert_relationship(&db, &pkg.id, &repo.id);

        // Add domain_resources: a domain owns the package.
        let domain = crate::storage::domains::create(db.conn(), "work", None).unwrap();
        insert_domain_resource(&db, &domain.id, &pkg.id);

        // Snapshot says only vim is installed (firefox removed).
        let installed = vec!["vim".to_string()];
        let summary = reconcile_with_snapshot(&db, &installed).unwrap();

        assert_eq!(summary.stale_packages_removed, 1);
        assert_eq!(summary.observations_cleaned, 1);
        assert_eq!(summary.relationships_cleaned, 1);
        assert_eq!(summary.domain_associations_cleaned, 1);

        // Package is gone.
        let remaining = resources::list_by_type(db.conn(), ResourceType::Package).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].native_id, "vim");

        // Observation is gone.
        let obs = observations::get(db.conn(), &pkg.id).unwrap();
        assert!(obs.is_none());

        // Relationship involving pkg is gone.
        let rels = relationships::list(db.conn()).unwrap();
        for rel in &rels {
            assert_ne!(rel.source_id, pkg.id);
            assert_ne!(rel.target_id, pkg.id);
        }

        // Domain resource entry is gone.
        let dr_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM domain_resources WHERE resource_id = ?1",
                rusqlite::params![pkg.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(dr_count, 0);

        // Domain and repo still exist (only the stale package was removed).
        assert!(crate::storage::domains::get(db.conn(), &domain.id)
            .unwrap()
            .is_some());
        assert!(resources::get(db.conn(), &repo.id).unwrap().is_some());
    }

    #[test]
    fn reconcile_empty_db_returns_zeros() {
        let db = Database::open_memory().unwrap();

        // Empty DB, snapshot has packages — nothing stale to remove.
        let installed = vec!["firefox".to_string(), "vim".to_string()];
        let summary = reconcile_with_snapshot(&db, &installed).unwrap();

        assert_eq!(summary.stale_packages_removed, 0);
        assert_eq!(summary.observations_cleaned, 0);
        assert_eq!(summary.relationships_cleaned, 0);
        assert_eq!(summary.domain_associations_cleaned, 0);
    }

    #[test]
    fn reconcile_empty_snapshot_removes_all() {
        let db = Database::open_memory().unwrap();

        // DB has packages but snapshot says nothing is installed.
        let pkg1 = insert_package(&db, "firefox");
        let pkg2 = insert_package(&db, "vim");
        insert_observation(&db, &pkg1.id);
        insert_observation(&db, &pkg2.id);

        let installed: Vec<String> = vec![];
        let summary = reconcile_with_snapshot(&db, &installed).unwrap();

        assert_eq!(summary.stale_packages_removed, 2);
        assert_eq!(summary.observations_cleaned, 2);

        let remaining = resources::list_by_type(db.conn(), ResourceType::Package).unwrap();
        assert!(remaining.is_empty());
    }

    #[test]
    fn reconcile_preserves_non_package_resources() {
        let db = Database::open_memory().unwrap();

        // Create a package and a repository.
        let pkg = insert_package(&db, "firefox");
        let repo = resources::create(
            db.conn(),
            ResourceType::Repository,
            "fedora",
            Some("Fedora"),
        )
        .unwrap();
        insert_observation(&db, &pkg.id);
        insert_observation(&db, &repo.id);

        // Snapshot says the package is gone but repository is not checked (only packages reconciled).
        let installed: Vec<String> = vec![];
        let summary = reconcile_with_snapshot(&db, &installed).unwrap();

        assert_eq!(summary.stale_packages_removed, 1);

        // Repository still exists — reconciliation only touches Package resources.
        let remaining_repos = resources::list_by_type(db.conn(), ResourceType::Repository).unwrap();
        assert_eq!(remaining_repos.len(), 1);
        assert_eq!(remaining_repos[0].native_id, "fedora");
    }

    #[test]
    fn reconcile_summary_display() {
        let zero = ReconcileSummary::default();
        assert_eq!(zero.to_string(), "No stale resources found.");

        let some = ReconcileSummary {
            stale_packages_removed: 3,
            observations_cleaned: 3,
            relationships_cleaned: 2,
            domain_associations_cleaned: 1,
        };
        let display = some.to_string();
        assert!(display.contains("3 stale package(s) removed."));
        assert!(display.contains("Associated observations, relationships, and domain associations cleaned automatically."));
    }
}
