use crate::core::analysis;
use crate::core::{Resource, ResourceType};
use crate::errors::Result;
use rusqlite::Connection;

/// A complete removal plan for a resource.
#[derive(Debug)]
pub struct RemovalPlan {
    /// The resource to be removed.
    pub resource: Resource,

    /// Services that will be affected by this removal.
    pub affected_services: Vec<String>,

    /// Packages that will be removed (the target itself).
    pub packages_to_remove: Vec<String>,

    /// Packages that depend on this resource but are NOT being removed.
    pub packages_still_required: Vec<String>,

    /// Domains that own this resource.
    pub owned_by_domains: Vec<String>,

    /// Domains that use this resource.
    pub used_by_domains: Vec<String>,

    /// Other domains that depend on this resource.
    pub other_domain_dependencies: Vec<String>,

    /// Warnings about the removal.
    pub warnings: Vec<String>,
}

impl RemovalPlan {
    /// Format the removal plan for display.
    pub fn format(&self) -> String {
        let mut output = String::new();

        let label = self
            .resource
            .display_name
            .as_deref()
            .unwrap_or(&self.resource.native_id);

        output.push_str("Removal plan\n");
        output.push_str("============\n");
        output.push_str("\n");
        output.push_str("Resource:\n");
        output.push_str(&format!("  {}\n", label));

        if !self.affected_services.is_empty() {
            output.push_str("\n");
            output.push_str("Will affect:\n");
            for svc in &self.affected_services {
                output.push_str(&format!("  {}\n", svc));
            }
        }

        if !self.packages_to_remove.is_empty() {
            output.push_str("\n");
            output.push_str("Packages to remove:\n");
            for pkg in &self.packages_to_remove {
                output.push_str(&format!("  {}\n", pkg));
            }
        }

        if !self.packages_still_required.is_empty() {
            output.push_str("\n");
            output.push_str("Packages still required elsewhere:\n");
            for pkg in &self.packages_still_required {
                output.push_str(&format!("  {}\n", pkg));
            }
        }

        if !self.other_domain_dependencies.is_empty() {
            output.push_str("\n");
            output.push_str("Other domain dependencies:\n");
            for dep in &self.other_domain_dependencies {
                output.push_str(&format!("  {}\n", dep));
            }
        }

        if !self.warnings.is_empty() {
            output.push_str("\n");
            output.push_str("Warnings:\n");
            for w in &self.warnings {
                output.push_str(&format!("  {}\n", w));
            }
        }

        output
    }
}

/// Build a removal plan for the resource identified by `native_id`.
pub fn plan_removal(conn: &Connection, native_id: &str) -> Result<RemovalPlan> {
    let resource =
        crate::storage::resources::find_by_native_id(conn, native_id)?.ok_or_else(|| {
            crate::errors::ChapeauError::ResourceNotFound(format!(
                "resource '{}' not found",
                native_id
            ))
        })?;

    // Find services that depend on this resource
    let affected_services = analysis::find_associated_services(conn, &resource.id)?;

    // Find all reverse dependencies (who depends on this resource)
    let all_dependents = analysis::get_dependent_names(conn, &resource.id)?;

    // Separate packages still required from the removal target
    let packages_to_remove = vec![resource.native_id.clone()];
    let packages_still_required: Vec<String> = all_dependents
        .iter()
        .filter(|dep| *dep != &resource.native_id)
        .cloned()
        .collect();

    // Domain ownership
    let owned_by_domains = analysis::get_owning_domain_names(conn, &resource.id)?;

    // Domain usage
    let used_by_domains = analysis::get_using_domain_names(conn, &resource.id)?;

    // Other domain dependencies (dependents that are owned by other domains)
    let other_domain_dependencies = find_other_domain_dependencies(conn, &resource.id)?;

    // Warnings
    let mut warnings = Vec::new();

    if resource.resource_type == ResourceType::Package && !affected_services.is_empty() {
        warnings.push(format!(
            "This package has {} associated service(s) that may stop working",
            affected_services.len()
        ));
    }

    if !packages_still_required.is_empty() {
        warnings.push(format!(
            "{} other package(s) depend on this resource",
            packages_still_required.len()
        ));
    }

    if !other_domain_dependencies.is_empty() {
        warnings.push(format!(
            "{} other domain(s) depend on this resource",
            other_domain_dependencies.len()
        ));
    }

    Ok(RemovalPlan {
        resource,
        affected_services,
        packages_to_remove,
        packages_still_required,
        owned_by_domains,
        used_by_domains,
        other_domain_dependencies,
        warnings,
    })
}

/// Find domains that depend on this resource (other than the resource's own domains).
fn find_other_domain_dependencies(conn: &Connection, resource_id: &str) -> Result<Vec<String>> {
    // Get all domains that own or use this resource
    let owned = analysis::get_owning_domain_names(conn, resource_id)?;
    let used = analysis::get_using_domain_names(conn, resource_id)?;

    // For now, return empty - domain dependency analysis will be more sophisticated
    // when we have the full domain graph
    let mut result = Vec::new();
    for domain in &owned {
        if !result.contains(domain) {
            result.push(domain.clone());
        }
    }
    for domain in &used {
        if !result.contains(domain) {
            result.push(domain.clone());
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{RelationshipOrigin, RelationshipType};
    use crate::storage::Database;
    use crate::storage::{domains, relationships, resources};

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    #[test]
    fn test_plan_removal_simple_package() {
        let db = temp_db();
        let conn = db.conn();

        let pkg = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();

        let plan = plan_removal(conn, "redis").unwrap();

        assert_eq!(plan.resource.id, pkg.id);
        assert_eq!(plan.packages_to_remove, vec!["redis"]);
        assert!(plan.packages_still_required.is_empty());
        assert!(plan.affected_services.is_empty());
        assert!(plan.warnings.is_empty());
    }

    #[test]
    fn test_plan_removal_with_dependents() {
        let db = temp_db();
        let conn = db.conn();

        let redis = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        let _app =
            resources::create(conn, ResourceType::Package, "my-app", Some("My App")).unwrap();

        // my-app depends on redis
        relationships::create(
            conn,
            &_app.id,
            RelationshipType::DependsOn,
            &redis.id,
            RelationshipOrigin::User,
        )
        .unwrap();

        let plan = plan_removal(conn, "redis").unwrap();

        assert_eq!(plan.packages_to_remove, vec!["redis"]);
        assert!(plan.packages_still_required.contains(&"my-app".to_string()));
        assert!(!plan.warnings.is_empty());
    }

    #[test]
    fn test_plan_removal_with_service() {
        let db = temp_db();
        let conn = db.conn();

        let redis = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        let svc = resources::create(
            conn,
            ResourceType::Service,
            "redis.service",
            Some("Redis Service"),
        )
        .unwrap();

        // redis.service comes from redis package
        relationships::create(
            conn,
            &svc.id,
            RelationshipType::ComesFrom,
            &redis.id,
            RelationshipOrigin::User,
        )
        .unwrap();

        let plan = plan_removal(conn, "redis").unwrap();

        assert!(plan
            .affected_services
            .contains(&"redis.service".to_string()));
        assert!(!plan.warnings.is_empty());
    }

    #[test]
    fn test_plan_removal_with_domain_ownership() {
        let db = temp_db();
        let conn = db.conn();

        let pkg = resources::create(conn, ResourceType::Package, "redis", Some("Redis")).unwrap();
        let domain =
            domains::create(conn, "infrastructure", Some("Infrastructure systems")).unwrap();

        // infrastructure domain owns redis
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO domain_resources (domain_id, resource_id, relationship, created_at, updated_at) VALUES (?1, ?2, 'owns', ?3, ?4)",
            rusqlite::params![domain.id, pkg.id, now, now],
        ).unwrap();

        let plan = plan_removal(conn, "redis").unwrap();

        assert!(plan
            .owned_by_domains
            .contains(&"infrastructure".to_string()));
    }

    #[test]
    fn test_plan_removal_not_found() {
        let db = temp_db();
        let conn = db.conn();

        let result = plan_removal(conn, "nonexistent");
        assert!(result.is_err());
    }
}
