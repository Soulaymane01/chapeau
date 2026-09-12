use crate::core::root::RootSource;
use crate::core::root_policy::{self, PackageDecision, PackageFacts};
use crate::core::{RelationshipOrigin, RelationshipType, ResourceType};
use crate::discovery::packages::{InstallReason, PackageRecord};
use crate::discovery::SystemSnapshot;
use crate::errors::Result;
use crate::storage::Database;
use crate::storage::{history, observations, relationships, resources, roots};
use std::collections::{HashMap, HashSet};

use super::drift::{self, DriftReport};

use crate::backends::dnf::{DnfCliBackend, ResolvedDependency};
use crate::backends::flatpak::FlatpakCliBackend;
use crate::backends::flatpak_backend::FlatpakBackendTrait;
use crate::backends::package_backend::PackageBackend;
use crate::backends::service_backend::ServiceBackend;
use crate::backends::systemd::SystemdDbusBackend;

/// Progress events emitted while a scan runs.
///
/// Frontends use these to show progress; the scan itself does not care who
/// listens or whether anyone does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanEvent {
    /// A backend is about to be queried.
    Discovering { backend: &'static str },
    /// A backend finished and reported this many resources.
    Discovered {
        backend: &'static str,
        resources: usize,
    },
    /// The bulk package dependency query is starting.
    Dependencies,
    /// The dependency query finished with this many edges.
    DependenciesDone { count: usize },
    /// Results are being written to the database.
    Committing,
}

/// Discover the full system state from all available backends.
pub fn discover() -> Result<SystemSnapshot> {
    discover_with_progress(&mut |_| {})
}

/// Discover the full system state, reporting progress per backend.
pub fn discover_with_progress(progress: &mut dyn FnMut(ScanEvent)) -> Result<SystemSnapshot> {
    let mut snap = SystemSnapshot::empty();

    // DNF packages + repositories
    let dnf = DnfCliBackend::new();
    if dnf.is_available() {
        progress(ScanEvent::Discovering {
            backend: "packages",
        });
        snap.packages = dnf.discover_installed()?;
        snap.repositories = dnf.discover_repositories()?;
        snap.sources.dnf = true;
        progress(ScanEvent::Discovered {
            backend: "packages",
            resources: snap.packages.len(),
        });
    }

    // systemd units
    let systemd = SystemdDbusBackend::new();
    if systemd.is_available() {
        progress(ScanEvent::Discovering {
            backend: "services",
        });
        let unit_snap = systemd.discover_units()?;
        snap.units = unit_snap.units;
        snap.sources.systemd = true;
        progress(ScanEvent::Discovered {
            backend: "services",
            resources: snap.units.len(),
        });
    }

    // Flatpak apps + runtimes + remotes
    let flatpak = FlatpakCliBackend::new();
    if flatpak.is_available() {
        progress(ScanEvent::Discovering {
            backend: "flatpaks",
        });
        let fp_snap = flatpak.discover_snapshot()?;
        snap.flatpak_apps = fp_snap.apps;
        snap.flatpak_runtimes = fp_snap.runtimes;
        snap.flatpak_remotes = fp_snap.remotes;
        snap.sources.flatpak = true;
        progress(ScanEvent::Discovered {
            backend: "flatpaks",
            resources: snap.flatpak_apps.len() + snap.flatpak_runtimes.len(),
        });
    }

    snap.snapshot_time = chrono::Utc::now().to_rfc3339();
    Ok(snap)
}

/// The result of a full scan.
#[derive(Debug)]
pub struct ScanOutcome {
    /// The freshly discovered system state.
    pub snapshot: SystemSnapshot,
    /// Differences between the pre-scan model and the discovered state.
    pub drift: DriftReport,
    /// Resources marked absent because a complete discovery no longer saw them.
    pub missing_marked: usize,
}

/// Run a full scan: discover system state, reconcile with DB, and commit atomically.
///
/// Returns the discovered snapshot. All discovered resources, observations, and
/// relationships are persisted within a single atomic transaction.
pub fn scan(db: &Database) -> Result<ScanOutcome> {
    scan_with_progress(db, &mut |_| {})
}

/// Run a full scan, reporting progress events as it advances.
///
/// The callback is invoked on the calling thread; frontends that need to stay
/// responsive should run the whole scan on a worker thread.
pub fn scan_with_progress(
    db: &Database,
    progress: &mut dyn FnMut(ScanEvent),
) -> Result<ScanOutcome> {
    let snapshot = discover_with_progress(progress)?;

    let mut summary = crate::discovery::ScanSummary {
        total_discovered: snapshot.total_count(),
        ..Default::default()
    };

    // Discover the hard package dependency graph before opening the write
    // transaction: it is a pure, bulk DNF5 query that can take a while on a
    // large install.
    let dnf = DnfCliBackend::new();
    let dependencies = if dnf.is_available() {
        progress(ScanEvent::Dependencies);
        let deps = dnf.discover_all_dependencies(&snapshot.packages)?;
        progress(ScanEvent::DependenciesDone { count: deps.len() });
        deps
    } else {
        Vec::new()
    };

    // Classify packages into semantic roles and root candidates before
    // persisting anything. DNF's user-installed flag is only one input here.
    let dependents = build_dependents_map(&dependencies);
    let facts: Vec<PackageFacts> = snapshot.packages.iter().map(package_facts).collect();
    let decisions = root_policy::classify_packages(&facts, &dependents);
    let decision_by_name: HashMap<&str, &PackageDecision> = decisions
        .iter()
        .map(|decision| (decision.name.as_str(), decision))
        .collect();

    db.transaction(|tx| {
        progress(ScanEvent::Committing);

        // --- Drift (computed against pre-scan state, before any writes) ---
        let drift = drift::compute(tx, &snapshot)?;

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

            // Upsert observation, retaining the package metadata used for
            // semantic classification so `chapeau why` can explain it.
            let metadata = decision_by_name.get(pkg.name.as_str()).map(|decision| {
                serde_json::json!({
                    "summary": pkg.summary,
                    "reason": pkg.reason.as_str(),
                    "role": decision.role.label(),
                    "source_rpm": pkg.source_rpm,
                })
                .to_string()
            });
            observations::upsert(
                tx,
                &resource.id,
                Some(true),
                Some(&pkg.version),
                None,
                None,
                None,
                metadata.as_deref(),
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

        // --- Package dependency graph (DependsOn) ---
        // Persist the hard runtime dependencies discovered before the
        // transaction. Both endpoints must exist as Package resources.
        for dep in &dependencies {
            if let Some(src_res) =
                resources::get_by_native(tx, ResourceType::Package, &dep.source_package)?
            {
                if let Some(tgt_res) =
                    resources::get_by_native(tx, ResourceType::Package, &dep.target_package)?
                {
                    if relationships::find_existing(
                        tx,
                        &src_res.id,
                        RelationshipType::DependsOn,
                        &tgt_res.id,
                    )?
                    .is_none()
                    {
                        relationships::create_tx(
                            tx,
                            &src_res.id,
                            RelationshipType::DependsOn,
                            &tgt_res.id,
                            RelationshipOrigin::System,
                        )?;
                        summary.relationships_created += 1;
                    }
                }
            }
        }

        // --- Absence marking ---
        // Only a backend that was actually queried may declare its resources
        // absent; a partial or unavailable discovery never implies loss.
        let mut missing_marked = 0;
        if snapshot.sources.dnf {
            let discovered: HashSet<&str> =
                snapshot.packages.iter().map(|pkg| pkg.name.as_str()).collect();
            missing_marked += mark_undiscovered_absent(tx, ResourceType::Package, &discovered)?;
        }
        if snapshot.sources.systemd {
            let discovered: HashSet<&str> =
                snapshot.units.iter().map(|unit| unit.name.as_str()).collect();
            missing_marked += mark_undiscovered_absent(tx, ResourceType::Service, &discovered)?;
        }
        if snapshot.sources.flatpak {
            let discovered: HashSet<&str> = snapshot
                .flatpak_apps
                .iter()
                .map(|app| app.id.as_str())
                .chain(
                    snapshot
                        .flatpak_runtimes
                        .iter()
                        .map(|runtime| runtime.id.as_str()),
                )
                .collect();
            missing_marked += mark_undiscovered_absent(tx, ResourceType::Flatpak, &discovered)?;
        }

        // --- Root reconciliation ---
        // Build the desired detected-root set: semantically classified
        // packages plus Flatpak applications (runtimes are not roots).
        // Explicit user roots are handled inside reconcile_roots.
        let mut desired: Vec<DesiredRoot> = Vec::new();
        for decision in decisions.iter().filter(|decision| decision.is_root) {
            if let Some(res) =
                resources::get_by_native(tx, ResourceType::Package, &decision.name)?
            {
                desired.push(DesiredRoot {
                    resource_id: res.id,
                    reason: decision.reason_string(),
                });
            }
        }
        for app in &snapshot.flatpak_apps {
            if let Some(res) = resources::get_by_native(tx, ResourceType::Flatpak, &app.id)? {
                desired.push(DesiredRoot {
                    resource_id: res.id,
                    reason: "Flatpak application".to_string(),
                });
            }
        }

        let root_summary = reconcile_roots(tx, &desired)?;
        if root_summary.added > 0 || root_summary.removed > 0 {
            history::insert(
                tx,
                "root.detect",
                None,
                None,
                None,
                Some(&format!(
                    "detected {} new root(s); removed {} stale detected root(s); {} explicit root(s) preserved",
                    root_summary.added, root_summary.removed, root_summary.preserved_explicit
                )),
            )?;
        }

        if !drift.is_empty() {
            history::insert(
                tx,
                "drift.detect",
                None,
                None,
                None,
                Some(&format!("scan detected drift: {}", drift.summary())),
            )?;
        }

        Ok(ScanOutcome {
            snapshot,
            drift,
            missing_marked,
        })
    })
}

/// Mark every resource of `resource_type` that was not part of a complete
/// discovery run as currently absent. Resources and their other semantic state
/// (roots, domains, relationships) are preserved — only the observation's
/// `installed` flag is updated.
///
/// Returns the number of resources whose recorded state changed.
pub(crate) fn mark_undiscovered_absent(
    conn: &rusqlite::Connection,
    resource_type: ResourceType,
    discovered: &HashSet<&str>,
) -> Result<usize> {
    let mut marked = 0;
    for resource in resources::list_by_type(conn, resource_type)? {
        if !discovered.contains(resource.native_id.as_str())
            && observations::mark_absent(conn, &resource.id)?
        {
            marked += 1;
        }
    }
    Ok(marked)
}

/// Build the map of package name → packages that directly depend on it.
fn build_dependents_map(dependencies: &[ResolvedDependency]) -> HashMap<String, Vec<String>> {
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
    for dep in dependencies {
        dependents
            .entry(dep.target_package.clone())
            .or_default()
            .push(dep.source_package.clone());
    }
    dependents
}

/// Convert a discovered package record into classifier facts.
fn package_facts(pkg: &PackageRecord) -> PackageFacts {
    PackageFacts {
        name: pkg.name.clone(),
        summary: pkg.summary.clone(),
        source_rpm: pkg.source_rpm.clone(),
        user_installed: pkg.reason == InstallReason::User,
        has_files: pkg.file_facts.has_files,
        has_executable: pkg.file_facts.has_executable,
        has_desktop_entry: pkg.file_facts.has_desktop_entry,
        has_app_bundle: pkg.file_facts.has_app_bundle,
    }
}

/// A root that should exist after a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DesiredRoot {
    pub resource_id: String,
    pub reason: String,
}

/// Outcome of applying the desired detected-root set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RootReconcileSummary {
    /// Number of newly detected roots.
    pub added: usize,
    /// Number of stale detected roots removed.
    pub removed: usize,
    /// Number of explicit (user/adopted) roots preserved untouched.
    pub preserved_explicit: usize,
}

/// Reconcile the `roots` table against the desired detected-root set.
///
/// Semantics:
/// - Explicit `user`/`adopted` roots are never touched.
/// - Detected roots are fully recomputed from current evidence: any detected
///   root that is no longer desired is removed.
/// - Resources themselves are never deleted here; only the semantic root
///   state is reconciled.
pub(crate) fn reconcile_roots(
    conn: &rusqlite::Connection,
    desired: &[DesiredRoot],
) -> Result<RootReconcileSummary> {
    let desired_ids: HashSet<&str> = desired
        .iter()
        .map(|root| root.resource_id.as_str())
        .collect();

    let mut summary = RootReconcileSummary::default();
    let existing = roots::list(conn)?;

    for root in &existing {
        match root.source {
            RootSource::Detected => {
                if !desired_ids.contains(root.resource_id.as_str()) {
                    roots::delete(conn, &root.resource_id)?;
                    summary.removed += 1;
                }
            }
            RootSource::User | RootSource::Adopted => {
                summary.preserved_explicit += 1;
            }
        }
    }

    for root in desired {
        if roots::get(conn, &root.resource_id)?.is_none() {
            roots::create_tx(
                conn,
                &root.resource_id,
                RootSource::Detected,
                Some(&root.reason),
            )?;
            summary.added += 1;
        }
    }

    Ok(summary)
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
    #[ignore] // Slow: runs real DNF5 queries (~80s for capability resolution)
    fn scan_with_memory_db() {
        let db = Database::open_memory().unwrap();
        let outcome = scan(&db).unwrap();
        // Should have discovered something on a real system.
        assert!(outcome.snapshot.total_count() > 0);

        // Verify resources were persisted.
        let res_list = resources::list(db.conn()).unwrap();
        assert!(!res_list.is_empty());

        // Verify observations were persisted.
        let obs_count = observations::count(db.conn()).unwrap();
        assert!(obs_count > 0);
    }

    #[test]
    #[ignore] // Slow: runs real DNF5 queries (~80s for capability resolution)
    fn scan_is_idempotent() {
        let db = Database::open_memory().unwrap();
        let _outcome1 = scan(&db).unwrap();
        let count1 = resources::count(db.conn()).unwrap();

        // Scan again — should not create new resources.
        let _outcome2 = scan(&db).unwrap();
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

    // ===== Phase 15.1: Root reconciliation =====

    fn desired(resource_id: &str, reason: &str) -> DesiredRoot {
        DesiredRoot {
            resource_id: resource_id.to_string(),
            reason: reason.to_string(),
        }
    }

    #[test]
    fn root_reconcile_adds_detected_roots() {
        let db = Database::open_memory().unwrap();
        let pkg = insert_package(&db, "zsh");

        let summary =
            reconcile_roots(db.conn(), &[desired(&pkg.id, "DNF user-installed package")]).unwrap();

        assert_eq!(summary.added, 1);
        assert_eq!(summary.removed, 0);
        let root = roots::get(db.conn(), &pkg.id).unwrap().unwrap();
        assert_eq!(root.source, RootSource::Detected);
        assert_eq!(root.reason.as_deref(), Some("DNF user-installed package"));
    }

    #[test]
    fn root_reconcile_is_idempotent() {
        let db = Database::open_memory().unwrap();
        let pkg = insert_package(&db, "zsh");
        let desired_roots = [desired(&pkg.id, "DNF user-installed package")];

        let first = reconcile_roots(db.conn(), &desired_roots).unwrap();
        let second = reconcile_roots(db.conn(), &desired_roots).unwrap();

        assert_eq!(first.added, 1);
        assert_eq!(second.added, 0);
        assert_eq!(second.removed, 0);
        assert_eq!(roots::count(db.conn()).unwrap(), 1);
    }

    #[test]
    fn root_reconcile_removes_stale_detected_roots_but_keeps_resource() {
        let db = Database::open_memory().unwrap();
        let stale = insert_package(&db, "xz-devel");
        let fresh = insert_package(&db, "zsh");

        // Simulate a previous scan that over-detected xz-devel.
        roots::create(
            db.conn(),
            &stale.id,
            RootSource::Detected,
            Some("DNF user-installed package"),
        )
        .unwrap();
        roots::create(
            db.conn(),
            &fresh.id,
            RootSource::Detected,
            Some("DNF user-installed package"),
        )
        .unwrap();

        let summary = reconcile_roots(
            db.conn(),
            &[desired(&fresh.id, "semantic root classification")],
        )
        .unwrap();

        assert_eq!(summary.removed, 1);
        assert_eq!(summary.added, 0);

        // The stale semantic root state is gone …
        assert!(roots::get(db.conn(), &stale.id).unwrap().is_none());
        // … but the resource itself is preserved.
        assert!(resources::get(db.conn(), &stale.id).unwrap().is_some());
        // The fresh detected root remains.
        assert!(roots::get(db.conn(), &fresh.id).unwrap().is_some());
    }

    #[test]
    fn root_reconcile_preserves_explicit_user_roots() {
        let db = Database::open_memory().unwrap();
        let explicit = insert_package(&db, "my-tool");
        let detected = insert_package(&db, "zsh");

        roots::create(
            db.conn(),
            &explicit.id,
            RootSource::User,
            Some("explicitly installed"),
        )
        .unwrap();
        roots::create(
            db.conn(),
            &detected.id,
            RootSource::Detected,
            Some("DNF user-installed package"),
        )
        .unwrap();

        // New scan no longer detects either package.
        let summary = reconcile_roots(db.conn(), &[]).unwrap();

        assert_eq!(summary.removed, 1);
        assert_eq!(summary.preserved_explicit, 1);
        let root = roots::get(db.conn(), &explicit.id).unwrap().unwrap();
        assert_eq!(root.source, RootSource::User);
        assert_eq!(root.reason.as_deref(), Some("explicitly installed"));
        assert!(roots::get(db.conn(), &detected.id).unwrap().is_none());
    }

    #[test]
    fn root_reconcile_does_not_downgrade_user_root_to_detected() {
        let db = Database::open_memory().unwrap();
        let pkg = insert_package(&db, "zsh");
        roots::create(
            db.conn(),
            &pkg.id,
            RootSource::User,
            Some("explicitly installed"),
        )
        .unwrap();

        let summary =
            reconcile_roots(db.conn(), &[desired(&pkg.id, "DNF user-installed package")]).unwrap();

        assert_eq!(summary.added, 0);
        assert_eq!(summary.preserved_explicit, 1);
        let root = roots::get(db.conn(), &pkg.id).unwrap().unwrap();
        assert_eq!(root.source, RootSource::User);
    }

    #[test]
    fn root_reconcile_repeated_scans_are_stable() {
        let db = Database::open_memory().unwrap();
        let a = insert_package(&db, "zsh");
        let b = insert_package(&db, "git");

        // First scan detects {zsh, git}.
        reconcile_roots(
            db.conn(),
            &[
                desired(&a.id, "DNF user-installed package"),
                desired(&b.id, "DNF user-installed package"),
            ],
        )
        .unwrap();
        assert_eq!(roots::count(db.conn()).unwrap(), 2);

        // Second scan classifies git as supporting; only zsh remains.
        let summary =
            reconcile_roots(db.conn(), &[desired(&a.id, "semantic root classification")]).unwrap();
        assert_eq!(summary.added, 0);
        assert_eq!(summary.removed, 1);
        assert_eq!(roots::count(db.conn()).unwrap(), 1);

        // Third scan is a no-op.
        let summary =
            reconcile_roots(db.conn(), &[desired(&a.id, "semantic root classification")]).unwrap();
        assert_eq!(summary.added, 0);
        assert_eq!(summary.removed, 0);
        assert_eq!(roots::count(db.conn()).unwrap(), 1);
    }

    #[test]
    fn root_reconcile_preserves_dependency_relationships() {
        let db = Database::open_memory().unwrap();
        let app = insert_package(&db, "postgresql-server");
        let lib = insert_package(&db, "postgresql-private-libs");
        let rel = insert_relationship(&db, &app.id, &lib.id);

        roots::create(
            db.conn(),
            &lib.id,
            RootSource::Detected,
            Some("DNF user-installed package"),
        )
        .unwrap();

        // lib is no longer a root, app is.
        reconcile_roots(
            db.conn(),
            &[desired(&app.id, "semantic root classification")],
        )
        .unwrap();

        // The dependency relationship survives the root reconciliation.
        let rels = relationships::list(db.conn()).unwrap();
        assert!(rels.iter().any(|r| r.id == rel.id));
        // Both resources still exist.
        assert!(resources::get(db.conn(), &app.id).unwrap().is_some());
        assert!(resources::get(db.conn(), &lib.id).unwrap().is_some());
    }

    // ===== Phase 15.2: absence marking =====

    #[test]
    fn mark_undiscovered_absent_marks_only_undiscovered() {
        let db = Database::open_memory().unwrap();
        let present = insert_package(&db, "firefox");
        let absent = insert_package(&db, "vim");
        insert_observation(&db, &present.id);
        insert_observation(&db, &absent.id);

        let discovered: HashSet<&str> = ["firefox"].into_iter().collect();
        let marked =
            mark_undiscovered_absent(db.conn(), ResourceType::Package, &discovered).unwrap();

        assert_eq!(marked, 1);
        let gone = observations::get(db.conn(), &absent.id).unwrap().unwrap();
        assert_eq!(gone.installed, Some(false));
        // Last seen version is preserved for the drift report.
        assert_eq!(gone.version.as_deref(), Some("1.0.0"));
        let present_obs = observations::get(db.conn(), &present.id).unwrap().unwrap();
        assert_eq!(present_obs.installed, Some(true));
    }

    #[test]
    fn mark_undiscovered_absent_is_idempotent() {
        let db = Database::open_memory().unwrap();
        let absent = insert_package(&db, "vim");
        insert_observation(&db, &absent.id);

        let first =
            mark_undiscovered_absent(db.conn(), ResourceType::Package, &HashSet::new()).unwrap();
        let second =
            mark_undiscovered_absent(db.conn(), ResourceType::Package, &HashSet::new()).unwrap();

        assert_eq!(first, 1);
        assert_eq!(second, 0);
    }

    #[test]
    fn explicit_root_survives_absence_and_root_reconciliation() {
        let db = Database::open_memory().unwrap();
        let pkg = insert_package(&db, "postgresql-server");
        insert_observation(&db, &pkg.id);
        roots::create(db.conn(), &pkg.id, RootSource::User, Some("databases")).unwrap();

        // The package disappears from a complete discovery run.
        let marked =
            mark_undiscovered_absent(db.conn(), ResourceType::Package, &HashSet::new()).unwrap();
        assert_eq!(marked, 1);

        // Detected-root reconciliation runs with an empty desired set.
        reconcile_roots(db.conn(), &[]).unwrap();

        // The explicit root and the resource both survive as missing.
        assert!(roots::get(db.conn(), &pkg.id).unwrap().is_some());
        assert!(resources::get(db.conn(), &pkg.id).unwrap().is_some());
        let missing = roots::list_missing(db.conn()).unwrap();
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].resource_id, pkg.id);
    }

    #[test]
    fn detected_root_is_recomputed_away_when_resource_disappears() {
        let db = Database::open_memory().unwrap();
        let pkg = insert_package(&db, "vim");
        insert_observation(&db, &pkg.id);
        roots::create(
            db.conn(),
            &pkg.id,
            RootSource::Detected,
            Some("DNF user-installed package"),
        )
        .unwrap();

        mark_undiscovered_absent(db.conn(), ResourceType::Package, &HashSet::new()).unwrap();
        let summary = reconcile_roots(db.conn(), &[]).unwrap();

        assert_eq!(summary.removed, 1);
        // The detected root is gone, but the resource itself is preserved.
        assert!(roots::get(db.conn(), &pkg.id).unwrap().is_none());
        assert!(resources::get(db.conn(), &pkg.id).unwrap().is_some());
    }

    #[test]
    fn build_dependents_map_groups_by_target() {
        let deps = vec![
            ResolvedDependency {
                source_package: "git".into(),
                target_package: "git-core".into(),
            },
            ResolvedDependency {
                source_package: "git-filter-repo".into(),
                target_package: "git-core".into(),
            },
        ];

        let dependents = build_dependents_map(&deps);
        assert_eq!(
            dependents.get("git-core").unwrap(),
            &vec!["git".to_string(), "git-filter-repo".to_string()]
        );
    }

    #[test]
    fn package_facts_map_discovery_fields() {
        let pkg = PackageRecord {
            name: "zsh".into(),
            version: "5.9-2.fc44".into(),
            arch: "x86_64".into(),
            repository: "fedora".into(),
            reason: InstallReason::User,
            from_repo: Some("fedora".into()),
            install_time: None,
            summary: Some("Powerful interactive shell".into()),
            source_rpm: Some("zsh-5.9-2.fc44.src.rpm".into()),
            file_facts: crate::discovery::packages::PackageFileFacts {
                has_files: true,
                has_executable: true,
                has_desktop_entry: false,
                has_app_bundle: false,
            },
        };

        let facts = package_facts(&pkg);
        assert_eq!(facts.name, "zsh");
        assert!(facts.user_installed);
        assert!(facts.has_executable);
        assert_eq!(facts.source_base().as_deref(), Some("zsh"));
    }
}
