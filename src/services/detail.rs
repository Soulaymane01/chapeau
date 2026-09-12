use crate::core::root::Root;
use crate::core::{Domain, Observation, RelationshipType, Resource, ResourceType};
use crate::errors::Result;
use crate::storage::Database;
use crate::storage::{domains, observations, relationships, resources, roots};

/// Everything a frontend needs to present one resource.
#[derive(Debug)]
pub struct ResourceDetail {
    pub resource: Resource,
    pub observation: Option<Observation>,
    pub root: Option<Root>,
    pub domains: Vec<(Domain, RelationshipType)>,
    /// Names of every configured domain, for "add to domain" pickers.
    pub all_domains: Vec<String>,
    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,
    pub provenance: Vec<String>,
    pub uses: Vec<String>,
    /// Services this resource provides (packages shipping unit files).
    pub provides: Vec<String>,
    /// Packages that provide this resource (services).
    pub provided_by: Vec<String>,
    /// Number of packages coming from this repository (repositories only).
    pub package_count: Option<usize>,
}

/// A resource's display name, falling back to its native id when the name is
/// missing or blank.
pub fn resource_label(resource: &Resource) -> String {
    resource
        .display_name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(&resource.native_id)
        .to_string()
}

impl ResourceDetail {
    /// Whether the resource is recorded as currently absent.
    pub fn is_missing(&self) -> bool {
        self.observation.as_ref().and_then(|obs| obs.installed) == Some(false)
    }

    /// A recorded observation metadata field (JSON), if present.
    pub fn metadata_str(&self, key: &str) -> Option<&str> {
        self.observation
            .as_ref()
            .and_then(|obs| obs.metadata.as_ref())
            .and_then(|meta| meta.get(key))
            .and_then(|value| value.as_str())
    }
}

/// Gather the detail model for a resource.
pub fn gather(db: &Database, resource: &Resource) -> Result<ResourceDetail> {
    let conn = db.conn();

    let observation = observations::get(conn, &resource.id)?;
    let root = roots::get(conn, &resource.id)?;
    let domain_list = domains::list_for_resource(conn, &resource.id)?;
    let all_domains = domains::list(conn)?
        .into_iter()
        .map(|domain| domain.name)
        .collect();

    let outgoing = relationships::list_from(conn, &resource.id)?;
    let incoming = relationships::list_to(conn, &resource.id)?;

    let mut dependencies = Vec::new();
    for rel in outgoing
        .iter()
        .filter(|rel| rel.relationship_type == RelationshipType::DependsOn)
    {
        if let Some(target) = resources::get(conn, &rel.target_id)? {
            dependencies.push(resource_label(&target));
        }
    }

    let mut dependents = Vec::new();
    for rel in incoming
        .iter()
        .filter(|rel| rel.relationship_type == RelationshipType::DependsOn)
    {
        if let Some(source) = resources::get(conn, &rel.source_id)? {
            dependents.push(resource_label(&source));
        }
    }

    let mut provenance = Vec::new();
    for rel in outgoing
        .iter()
        .filter(|rel| rel.relationship_type == RelationshipType::ComesFrom)
    {
        if let Some(target) = resources::get(conn, &rel.target_id)? {
            provenance.push(resource_label(&target));
        }
    }

    let mut uses = Vec::new();
    for rel in outgoing
        .iter()
        .filter(|rel| rel.relationship_type == RelationshipType::Uses)
    {
        if let Some(target) = resources::get(conn, &rel.target_id)? {
            uses.push(resource_label(&target));
        }
    }

    let mut provides = Vec::new();
    for rel in outgoing
        .iter()
        .filter(|rel| rel.relationship_type == RelationshipType::Provides)
    {
        if let Some(target) = resources::get(conn, &rel.target_id)? {
            provides.push(target.native_id);
        }
    }

    let mut provided_by = Vec::new();
    for rel in incoming
        .iter()
        .filter(|rel| rel.relationship_type == RelationshipType::Provides)
    {
        if let Some(source) = resources::get(conn, &rel.source_id)? {
            provided_by.push(source.native_id);
        }
    }

    let package_count = if resource.resource_type == ResourceType::Repository {
        Some(
            incoming
                .iter()
                .filter(|rel| rel.relationship_type == RelationshipType::ComesFrom)
                .count(),
        )
    } else {
        None
    };

    Ok(ResourceDetail {
        resource: resource.clone(),
        observation,
        root,
        domains: domain_list,
        all_domains,
        dependencies,
        dependents,
        provenance,
        uses,
        provides,
        provided_by,
        package_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::root::RootSource;
    use crate::core::{RelationshipOrigin, RelationshipType};
    use crate::storage::{domains, observations, relationships, resources, roots};

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    #[test]
    fn gather_collects_relationships_and_domains() {
        let db = temp_db();
        let postgres =
            resources::create(db.conn(), ResourceType::Package, "postgresql-server", None).unwrap();
        let lib = resources::create(db.conn(), ResourceType::Package, "libpq", None).unwrap();
        let repo = resources::create(db.conn(), ResourceType::Repository, "fedora", None).unwrap();
        let service =
            resources::create(db.conn(), ResourceType::Service, "postgresql.service", None)
                .unwrap();
        let databases = domains::create(db.conn(), "databases", None).unwrap();

        observations::upsert(
            db.conn(),
            &postgres.id,
            Some(true),
            Some("16.4-1.fc44"),
            None,
            None,
            None,
            Some(r#"{"summary":"PostgreSQL server","reason":"User","role":"application"}"#),
        )
        .unwrap();
        relationships::create(
            db.conn(),
            &postgres.id,
            RelationshipType::DependsOn,
            &lib.id,
            RelationshipOrigin::System,
        )
        .unwrap();
        relationships::create(
            db.conn(),
            &lib.id,
            RelationshipType::DependsOn,
            &postgres.id,
            RelationshipOrigin::System,
        )
        .unwrap();
        relationships::create(
            db.conn(),
            &postgres.id,
            RelationshipType::ComesFrom,
            &repo.id,
            RelationshipOrigin::System,
        )
        .unwrap();
        relationships::create(
            db.conn(),
            &postgres.id,
            RelationshipType::Provides,
            &service.id,
            RelationshipOrigin::System,
        )
        .unwrap();
        db.conn()
            .execute(
                "INSERT INTO domain_resources (domain_id, resource_id, relationship, created_at, updated_at)
                 VALUES (?1, ?2, 'owns', datetime('now'), datetime('now'))",
                rusqlite::params![databases.id, postgres.id],
            )
            .unwrap();
        roots::create(
            db.conn(),
            &postgres.id,
            RootSource::Detected,
            Some("project package"),
        )
        .unwrap();

        let detail = gather(&db, &postgres).unwrap();

        assert_eq!(detail.dependencies, vec!["libpq"]);
        assert_eq!(detail.dependents, vec!["libpq"]);
        assert_eq!(detail.provenance, vec!["fedora"]);
        assert_eq!(detail.provides, vec!["postgresql.service"]);
        assert!(detail.provided_by.is_empty());
        assert_eq!(detail.domains.len(), 1);
        assert_eq!(detail.domains[0].0.name, "databases");
        assert_eq!(detail.domains[0].1, RelationshipType::Owns);
        assert!(detail.root.is_some());
        assert_eq!(detail.metadata_str("summary"), Some("PostgreSQL server"));
        assert!(!detail.is_missing());
        assert_eq!(detail.package_count, None);
    }

    #[test]
    fn gather_counts_repository_packages() {
        let db = temp_db();
        let repo = resources::create(db.conn(), ResourceType::Repository, "fedora", None).unwrap();
        for name in ["zsh", "vim", "git"] {
            let pkg = resources::create(db.conn(), ResourceType::Package, name, None).unwrap();
            relationships::create(
                db.conn(),
                &pkg.id,
                RelationshipType::ComesFrom,
                &repo.id,
                RelationshipOrigin::System,
            )
            .unwrap();
        }

        let detail = gather(&db, &repo).unwrap();
        assert_eq!(detail.package_count, Some(3));
    }

    #[test]
    fn gather_reports_service_providers() {
        let db = temp_db();
        let package =
            resources::create(db.conn(), ResourceType::Package, "postgresql-server", None).unwrap();
        let service =
            resources::create(db.conn(), ResourceType::Service, "postgresql.service", None)
                .unwrap();
        relationships::create(
            db.conn(),
            &package.id,
            RelationshipType::Provides,
            &service.id,
            RelationshipOrigin::System,
        )
        .unwrap();

        let detail = gather(&db, &service).unwrap();
        assert_eq!(detail.provided_by, vec!["postgresql-server"]);
        assert!(detail.provides.is_empty());
    }

    #[test]
    fn missing_state_is_reported() {
        let db = temp_db();
        let pkg = resources::create(db.conn(), ResourceType::Package, "redis", None).unwrap();
        observations::upsert(
            db.conn(),
            &pkg.id,
            Some(false),
            Some("7.2.5"),
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let detail = gather(&db, &pkg).unwrap();
        assert!(detail.is_missing());
        assert_eq!(
            detail.observation.unwrap().version.as_deref(),
            Some("7.2.5")
        );
    }
}
