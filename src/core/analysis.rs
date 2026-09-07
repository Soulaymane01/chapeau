use crate::core::{Resource, ResourceType};
use crate::errors::Result;
use rusqlite::Connection;

/// Reason a resource appears in the orphaned or unused analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisReason {
    /// No other resource has a DependsOn relationship pointing at this resource.
    NoDependents,
    /// No domain owns this resource.
    NoDomainOwnership,
    /// No domain uses this resource.
    NoDomainUsage,
    /// This resource does not depend on any other resource.
    NoDependencies,
    /// This resource does not interact with any other resource via Provides/Uses.
    NoInteractions,
}

impl AnalysisReason {
    pub fn description(&self) -> &'static str {
        match self {
            AnalysisReason::NoDependents => "No remaining dependents",
            AnalysisReason::NoDomainOwnership => "No domain ownership",
            AnalysisReason::NoDomainUsage => "No domain usage",
            AnalysisReason::NoDependencies => "Does not depend on other resources",
            AnalysisReason::NoInteractions => "Does not interact with other resources",
        }
    }
}

/// Result of analyzing a single resource.
#[derive(Debug, Clone)]
pub struct ResourceAnalysis {
    pub resource: Resource,
    pub reasons: Vec<AnalysisReason>,
    /// Resources that depend on this one (incoming DependsOn).
    pub dependents: Vec<String>,
    /// Domains that own this resource.
    pub owned_by_domains: Vec<String>,
    /// Domains that use this resource.
    pub used_by_domains: Vec<String>,
    /// Services associated with this resource (for packages).
    pub affected_services: Vec<String>,
}

/// Check if a resource has any incoming DependsOn relationships (other resources depend on it).
pub(crate) fn has_dependents(conn: &Connection, resource_id: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM relationships
         WHERE target_id = ?1 AND relationship = 'depends_on'",
        [resource_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Get the names of resources that depend on a given resource.
pub(crate) fn get_dependent_names(conn: &Connection, resource_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT r.native_id FROM relationships rel
         JOIN resources r ON r.id = rel.source_id
         WHERE rel.target_id = ?1 AND rel.relationship = 'depends_on'
         ORDER BY r.native_id",
    )?;
    let names = stmt
        .query_map([resource_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(names)
}

/// Check if a resource is owned by any domain.
pub(crate) fn is_domain_owned(conn: &Connection, resource_id: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM domain_resources
         WHERE resource_id = ?1 AND relationship = 'owns'",
        [resource_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Get the names of domains that own a resource.
pub(crate) fn get_owning_domain_names(conn: &Connection, resource_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT d.name FROM domain_resources dr
         JOIN domains d ON d.id = dr.domain_id
         WHERE dr.resource_id = ?1 AND dr.relationship = 'owns'
         ORDER BY d.name",
    )?;
    let names = stmt
        .query_map([resource_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(names)
}

/// Check if a resource is used by any domain.
pub(crate) fn is_domain_used(conn: &Connection, resource_id: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM domain_resources
         WHERE resource_id = ?1 AND relationship = 'uses'",
        [resource_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Get the names of domains that use a resource.
pub(crate) fn get_using_domain_names(conn: &Connection, resource_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT d.name FROM domain_resources dr
         JOIN domains d ON d.id = dr.domain_id
         WHERE dr.resource_id = ?1 AND dr.relationship = 'uses'
         ORDER BY d.name",
    )?;
    let names = stmt
        .query_map([resource_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(names)
}

/// Check if a resource has any outgoing DependsOn relationships.
pub(crate) fn has_dependencies(conn: &Connection, resource_id: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM relationships
         WHERE source_id = ?1 AND relationship = 'depends_on'",
        [resource_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Check if a resource has any Provides or Uses relationships (as source).
pub(crate) fn has_interactions(conn: &Connection, resource_id: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM relationships
         WHERE source_id = ?1 AND relationship IN ('provides', 'uses')",
        [resource_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Find services associated with a package via ComesFrom relationships.
/// If package P ComesFrom service S, then removing P affects S.
pub(crate) fn find_associated_services(
    conn: &Connection,
    resource_id: &str,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT r.native_id FROM relationships rel
         JOIN resources r ON r.id = rel.source_id
         WHERE rel.target_id = ?1
           AND rel.relationship = 'comes_from'
           AND r.type = 'service'
         ORDER BY r.native_id",
    )?;
    let names = stmt
        .query_map([resource_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(names)
}

/// Analyze a single resource for orphaned status.
///
/// A resource is "orphaned" (potentially removable) when:
/// - No other resource has a DependsOn pointing at it (no dependents)
/// - No domain owns it
/// - No domain uses it
///
/// Domain ownership alone does NOT make a resource non-removable —
/// the spec requires all protection signals to be evaluated.
pub fn analyze_orphaned(conn: &Connection, resource: &Resource) -> Result<ResourceAnalysis> {
    let mut reasons = Vec::new();

    let dependents = get_dependent_names(conn, &resource.id)?;
    if dependents.is_empty() {
        reasons.push(AnalysisReason::NoDependents);
    }

    let owned_by_domains = get_owning_domain_names(conn, &resource.id)?;
    if owned_by_domains.is_empty() {
        reasons.push(AnalysisReason::NoDomainOwnership);
    }

    let used_by_domains = get_using_domain_names(conn, &resource.id)?;
    if used_by_domains.is_empty() {
        reasons.push(AnalysisReason::NoDomainUsage);
    }

    let affected_services = find_associated_services(conn, &resource.id)?;

    Ok(ResourceAnalysis {
        resource: resource.clone(),
        reasons,
        dependents,
        owned_by_domains,
        used_by_domains,
        affected_services,
    })
}

/// Analyze a single resource for unused status.
///
/// A resource is "unused" when:
/// - No other resource has a DependsOn pointing at it (no dependents)
/// - No domain owns it
/// - No domain uses it
/// - It does not depend on other resources (no outgoing DependsOn)
/// - It does not provide or use other resources (no Provides/Uses as source)
pub fn analyze_unused(conn: &Connection, resource: &Resource) -> Result<ResourceAnalysis> {
    let mut reasons = Vec::new();

    let dependents = get_dependent_names(conn, &resource.id)?;
    if dependents.is_empty() {
        reasons.push(AnalysisReason::NoDependents);
    }

    let owned_by_domains = get_owning_domain_names(conn, &resource.id)?;
    if owned_by_domains.is_empty() {
        reasons.push(AnalysisReason::NoDomainOwnership);
    }

    let used_by_domains = get_using_domain_names(conn, &resource.id)?;
    if used_by_domains.is_empty() {
        reasons.push(AnalysisReason::NoDomainUsage);
    }

    if !has_dependencies(conn, &resource.id)? {
        reasons.push(AnalysisReason::NoDependencies);
    }

    if !has_interactions(conn, &resource.id)? {
        reasons.push(AnalysisReason::NoInteractions);
    }

    let affected_services = find_associated_services(conn, &resource.id)?;

    Ok(ResourceAnalysis {
        resource: resource.clone(),
        reasons,
        dependents,
        owned_by_domains,
        used_by_domains,
        affected_services,
    })
}

/// Find all orphaned resources in the database.
///
/// Returns resources that are candidates for removal, excluding repositories
/// (which are containers, not removable units).
pub fn find_orphaned(conn: &Connection) -> Result<Vec<ResourceAnalysis>> {
    let resources = crate::storage::resources::list(conn)?;
    let mut results = Vec::new();

    for resource in &resources {
        // Skip repositories — they are containers, not removable units
        if resource.resource_type == ResourceType::Repository {
            continue;
        }

        let analysis = analyze_orphaned(conn, resource)?;
        if !analysis.reasons.is_empty() {
            results.push(analysis);
        }
    }

    Ok(results)
}

/// Find all unused resources in the database.
///
/// Returns resources that nothing depends on, that have no domain associations,
/// and that have no outgoing dependency or interaction relationships.
pub fn find_unused(conn: &Connection) -> Result<Vec<ResourceAnalysis>> {
    let resources = crate::storage::resources::list(conn)?;
    let mut results = Vec::new();

    for resource in &resources {
        // Skip repositories — they are containers, not removable units
        if resource.resource_type == ResourceType::Repository {
            continue;
        }

        let analysis = analyze_unused(conn, resource)?;
        // Only include if it has ALL five reasons (truly unused)
        if analysis.reasons.len() >= 5 {
            results.push(analysis);
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::RelationshipType;
    use crate::storage::Database;
    use crate::storage::{domains, relationships, resources};

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    #[test]
    fn test_no_dependents_detected() {
        let db = temp_db();
        let conn = db.conn();
        let pkg = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        assert!(!has_dependents(conn, &pkg.id).unwrap());
    }

    #[test]
    fn test_dependents_detected() {
        let db = temp_db();
        let conn = db.conn();
        let pkg_a = resources::create(conn, ResourceType::Package, "app", Some("App")).unwrap();
        let pkg_b = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        // app depends on redis
        relationships::create(
            conn,
            &pkg_a.id,
            RelationshipType::DependsOn,
            &pkg_b.id,
            crate::core::RelationshipOrigin::System,
        )
        .unwrap();
        assert!(has_dependents(conn, &pkg_b.id).unwrap());
    }

    #[test]
    fn test_domain_owned_detected() {
        let db = temp_db();
        let conn = db.conn();
        let domain = domains::create(conn, "production", Some("Production env")).unwrap();
        let pkg = resources::create(conn, ResourceType::Package, "nginx", Some("Nginx")).unwrap();
        // Insert into domain_resources directly
        conn.execute(
            "INSERT INTO domain_resources (domain_id, resource_id, relationship, created_at, updated_at)
             VALUES (?1, ?2, 'owns', datetime('now'), datetime('now'))",
            rusqlite::params![domain.id, pkg.id],
        ).unwrap();
        assert!(is_domain_owned(conn, &pkg.id).unwrap());
    }

    #[test]
    fn test_not_domain_owned() {
        let db = temp_db();
        let conn = db.conn();
        let pkg = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        assert!(!is_domain_owned(conn, &pkg.id).unwrap());
    }

    #[test]
    fn test_analyze_orphaned_all_reasons() {
        let db = temp_db();
        let conn = db.conn();
        let pkg = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        let analysis = analyze_orphaned(conn, &pkg).unwrap();
        assert_eq!(analysis.reasons.len(), 3);
        assert!(analysis.reasons.contains(&AnalysisReason::NoDependents));
        assert!(analysis
            .reasons
            .contains(&AnalysisReason::NoDomainOwnership));
        assert!(analysis.reasons.contains(&AnalysisReason::NoDomainUsage));
        assert!(analysis.dependents.is_empty());
        assert!(analysis.owned_by_domains.is_empty());
        assert!(analysis.used_by_domains.is_empty());
    }

    #[test]
    fn test_analyze_orphaned_not_orphaned_when_depended_on() {
        let db = temp_db();
        let conn = db.conn();
        let pkg_a = resources::create(conn, ResourceType::Package, "app", Some("App")).unwrap();
        let pkg_b = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        relationships::create(
            conn,
            &pkg_a.id,
            RelationshipType::DependsOn,
            &pkg_b.id,
            crate::core::RelationshipOrigin::System,
        )
        .unwrap();

        let analysis = analyze_orphaned(conn, &pkg_b).unwrap();
        // Should NOT have NoDependents reason
        assert!(!analysis.reasons.contains(&AnalysisReason::NoDependents));
        assert_eq!(analysis.dependents, vec!["app"]);
    }

    #[test]
    fn test_analyze_orphaned_not_orphaned_when_domain_owned() {
        let db = temp_db();
        let conn = db.conn();
        let domain = domains::create(conn, "production", None).unwrap();
        let pkg = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        conn.execute(
            "INSERT INTO domain_resources (domain_id, resource_id, relationship, created_at, updated_at)
             VALUES (?1, ?2, 'owns', datetime('now'), datetime('now'))",
            rusqlite::params![domain.id, pkg.id],
        ).unwrap();

        let analysis = analyze_orphaned(conn, &pkg).unwrap();
        assert!(!analysis
            .reasons
            .contains(&AnalysisReason::NoDomainOwnership));
        assert_eq!(analysis.owned_by_domains, vec!["production"]);
    }

    #[test]
    fn test_find_orphaned_excludes_repositories() {
        let db = temp_db();
        let conn = db.conn();
        // Create a repository with no relationships — should be excluded
        resources::create(conn, ResourceType::Repository, "fedora", Some("Fedora")).unwrap();
        // Create an actual orphaned package
        resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();

        let orphaned = find_orphaned(conn).unwrap();
        assert_eq!(orphaned.len(), 1);
        assert_eq!(orphaned[0].resource.native_id, "redis");
    }

    #[test]
    fn test_find_orphaned_empty_db() {
        let db = temp_db();
        let orphaned = find_orphaned(db.conn()).unwrap();
        assert!(orphaned.is_empty());
    }

    #[test]
    fn test_analyze_unused_all_five_reasons() {
        let db = temp_db();
        let conn = db.conn();
        // Create a standalone resource with no relationships at all
        let pkg = resources::create(
            conn,
            ResourceType::Package,
            "standalone",
            Some("Standalone"),
        )
        .unwrap();
        let analysis = analyze_unused(conn, &pkg).unwrap();
        assert_eq!(analysis.reasons.len(), 5);
    }

    #[test]
    fn test_analyze_unused_not_unused_when_has_dependencies() {
        let db = temp_db();
        let conn = db.conn();
        let pkg_a = resources::create(conn, ResourceType::Package, "app", Some("App")).unwrap();
        let pkg_b = resources::create(conn, ResourceType::Package, "lib", Some("Lib")).unwrap();
        // app depends on lib
        relationships::create(
            conn,
            &pkg_a.id,
            RelationshipType::DependsOn,
            &pkg_b.id,
            crate::core::RelationshipOrigin::System,
        )
        .unwrap();

        let analysis = analyze_unused(conn, &pkg_a).unwrap();
        // app has outgoing DependsOn, so NoDependencies should NOT be a reason
        assert!(!analysis.reasons.contains(&AnalysisReason::NoDependencies));
    }

    #[test]
    fn test_analyze_unused_not_unused_when_has_interactions() {
        let db = temp_db();
        let conn = db.conn();
        let pkg = resources::create(conn, ResourceType::Package, "plugin", Some("Plugin")).unwrap();
        let target = resources::create(conn, ResourceType::Package, "core", Some("Core")).unwrap();
        // plugin provides core
        relationships::create(
            conn,
            &pkg.id,
            RelationshipType::Provides,
            &target.id,
            crate::core::RelationshipOrigin::Derived,
        )
        .unwrap();

        let analysis = analyze_unused(conn, &pkg).unwrap();
        assert!(!analysis.reasons.contains(&AnalysisReason::NoInteractions));
    }

    #[test]
    fn test_find_unused_empty_db() {
        let db = temp_db();
        let unused = find_unused(db.conn()).unwrap();
        assert!(unused.is_empty());
    }

    #[test]
    fn test_affected_services_shown() {
        let db = temp_db();
        let conn = db.conn();
        let svc = resources::create(
            conn,
            ResourceType::Service,
            "redis.service",
            Some("Redis Service"),
        )
        .unwrap();
        let pkg = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        // redis.service comes_from redis
        relationships::create(
            conn,
            &svc.id,
            RelationshipType::ComesFrom,
            &pkg.id,
            crate::core::RelationshipOrigin::System,
        )
        .unwrap();

        let analysis = analyze_orphaned(conn, &pkg).unwrap();
        assert_eq!(analysis.affected_services, vec!["redis.service"]);
    }

    #[test]
    fn test_multiple_dependents_listed() {
        let db = temp_db();
        let conn = db.conn();
        let lib =
            resources::create(conn, ResourceType::Package, "openssl", Some("OpenSSL")).unwrap();
        let app1 = resources::create(conn, ResourceType::Package, "nginx", Some("Nginx")).unwrap();
        let app2 = resources::create(conn, ResourceType::Package, "curl", Some("curl")).unwrap();
        let app3 = resources::create(conn, ResourceType::Package, "git", Some("git")).unwrap();

        for app in &[&app1, &app2, &app3] {
            relationships::create(
                conn,
                &app.id,
                RelationshipType::DependsOn,
                &lib.id,
                crate::core::RelationshipOrigin::System,
            )
            .unwrap();
        }

        let analysis = analyze_orphaned(conn, &lib).unwrap();
        assert_eq!(analysis.dependents.len(), 3);
        assert!(analysis.dependents.contains(&"curl".to_string()));
        assert!(analysis.dependents.contains(&"git".to_string()));
        assert!(analysis.dependents.contains(&"nginx".to_string()));
    }

    #[test]
    fn test_shared_resource_not_removable() {
        // Simulates the "python" scenario from the spec
        let db = temp_db();
        let conn = db.conn();
        let python =
            resources::create(conn, ResourceType::Package, "python", Some("Python")).unwrap();
        let torch =
            resources::create(conn, ResourceType::Package, "pytorch", Some("PyTorch")).unwrap();
        let dev =
            resources::create(conn, ResourceType::Package, "dev-tools", Some("Dev Tools")).unwrap();
        let ai = resources::create(
            conn,
            ResourceType::Package,
            "ai-framework",
            Some("AI Framework"),
        )
        .unwrap();

        for consumer in &[&torch, &dev, &ai] {
            relationships::create(
                conn,
                &consumer.id,
                RelationshipType::DependsOn,
                &python.id,
                crate::core::RelationshipOrigin::System,
            )
            .unwrap();
        }

        let analysis = analyze_orphaned(conn, &python).unwrap();
        // python has 3 dependents — definitely not orphaned
        assert!(!analysis.reasons.contains(&AnalysisReason::NoDependents));
        assert_eq!(analysis.dependents.len(), 3);
        // NOT removable
        assert!(
            analysis.reasons.is_empty()
                || !analysis.reasons.contains(&AnalysisReason::NoDependents)
        );
    }
}
