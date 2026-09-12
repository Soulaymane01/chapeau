use crate::core::{Domain, RelationshipType, Resource};
use crate::errors::{ChapeauError, Result};
use crate::storage::{domains, history, resources, Database};

/// A domain with its current resource memberships.
#[derive(Debug, Clone)]
pub struct DomainSummary {
    pub domain: Domain,
    pub resources: Vec<(Resource, RelationshipType)>,
}

/// List domains with their memberships, ordered by domain name.
pub fn list(db: &Database) -> Result<Vec<DomainSummary>> {
    let conn = db.conn();
    let mut summaries = Vec::new();
    for domain in domains::list(conn)? {
        let members = domains::list_resources(conn, &domain.id)?;
        summaries.push(DomainSummary {
            domain,
            resources: members,
        });
    }
    Ok(summaries)
}

/// Create a domain. Fails if the name is already taken.
pub fn create(db: &Database, name: &str, description: Option<&str>) -> Result<Domain> {
    if domains::get_by_name(db.conn(), name)?.is_some() {
        return Err(ChapeauError::Validation(format!(
            "domain '{name}' already exists"
        )));
    }

    let domain = domains::create(db.conn(), name, description)?;
    history::insert(
        db.conn(),
        "domain.create",
        None,
        None,
        Some(&domain.id),
        Some(&format!("created domain '{name}'")),
    )?;
    Ok(domain)
}

/// Delete a domain. Memberships cascade; resources are never touched.
pub fn delete(db: &Database, name: &str) -> Result<Domain> {
    let domain = domains::get_by_name(db.conn(), name)?
        .ok_or_else(|| ChapeauError::DomainNotFound(name.to_string()))?;

    domains::delete(db.conn(), &domain.id)?;
    history::insert(
        db.conn(),
        "domain.delete",
        None,
        None,
        None,
        Some(&format!("deleted domain '{name}'")),
    )?;
    Ok(domain)
}

/// Associate a resource with a domain ('owns' or 'uses').
pub fn add_resource(
    db: &Database,
    domain_name: &str,
    resource_name: &str,
    relationship: RelationshipType,
    reason: Option<&str>,
) -> Result<(Domain, Resource)> {
    let domain = domains::get_by_name(db.conn(), domain_name)?
        .ok_or_else(|| ChapeauError::DomainNotFound(domain_name.to_string()))?;
    let resource = resources::find_by_native_id(db.conn(), resource_name)?
        .ok_or_else(|| ChapeauError::ResourceNotFound(resource_name.to_string()))?;

    domains::add_resource(db.conn(), &domain.id, &resource.id, relationship, reason)?;
    history::insert(
        db.conn(),
        "domain.add",
        Some(resource.resource_type.as_str()),
        Some(&resource.id),
        Some(&domain.id),
        Some(&format!(
            "added '{}' to domain '{}' ({})",
            resource_name, domain_name, relationship
        )),
    )?;

    Ok((domain, resource))
}

/// Remove a resource's association with a domain.
///
/// Returns the domain, the resource, and the relationship types that were
/// present before removal.
pub fn remove_resource(
    db: &Database,
    domain_name: &str,
    resource_name: &str,
) -> Result<(Domain, Resource, Vec<RelationshipType>)> {
    let domain = domains::get_by_name(db.conn(), domain_name)?
        .ok_or_else(|| ChapeauError::DomainNotFound(domain_name.to_string()))?;
    let resource = resources::find_by_native_id(db.conn(), resource_name)?
        .ok_or_else(|| ChapeauError::ResourceNotFound(resource_name.to_string()))?;

    let present: Vec<RelationshipType> = domains::list_for_resource(db.conn(), &resource.id)?
        .into_iter()
        .filter(|(member_domain, _)| member_domain.id == domain.id)
        .map(|(_, relationship)| relationship)
        .collect();

    domains::remove_resource(db.conn(), &domain.id, &resource.id)?;
    history::insert(
        db.conn(),
        "domain.remove",
        Some(resource.resource_type.as_str()),
        Some(&resource.id),
        Some(&domain.id),
        Some(&format!(
            "removed '{}' from domain '{}' ({})",
            resource_name,
            domain_name,
            present
                .iter()
                .map(|relationship| relationship.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    )?;

    Ok((domain, resource, present))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ResourceType;
    use crate::storage::resources;

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    #[test]
    fn create_add_list_and_remove() {
        let db = temp_db();
        let domain = create(&db, "databases", Some("Database servers")).unwrap();
        assert_eq!(domain.name, "databases");

        let pkg =
            resources::create(db.conn(), ResourceType::Package, "postgresql-server", None).unwrap();
        let (added_domain, added_resource) = add_resource(
            &db,
            "databases",
            "postgresql-server",
            RelationshipType::Owns,
            Some("primary database"),
        )
        .unwrap();
        assert_eq!(added_domain.id, domain.id);
        assert_eq!(added_resource.id, pkg.id);

        let summaries = list(&db).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].resources.len(), 1);
        assert_eq!(summaries[0].resources[0].1, RelationshipType::Owns);

        let (_, _, present) = remove_resource(&db, "databases", "postgresql-server").unwrap();
        assert_eq!(present, vec![RelationshipType::Owns]);
        assert!(list(&db).unwrap()[0].resources.is_empty());
    }

    #[test]
    fn duplicate_create_is_rejected() {
        let db = temp_db();
        create(&db, "development", None).unwrap();
        let error = create(&db, "development", None).unwrap_err();
        assert!(error.to_string().contains("already exists"));
    }

    #[test]
    fn delete_domain_preserves_resources() {
        let db = temp_db();
        create(&db, "ai", None).unwrap();
        let pkg = resources::create(db.conn(), ResourceType::Package, "ollama", None).unwrap();
        add_resource(&db, "ai", "ollama", RelationshipType::Uses, None).unwrap();

        delete(&db, "ai").unwrap();

        assert!(list(&db).unwrap().is_empty());
        assert!(resources::get(db.conn(), &pkg.id).unwrap().is_some());
    }

    #[test]
    fn missing_domain_and_resource_are_reported() {
        let db = temp_db();
        assert!(create(&db, "data", None).is_ok());
        assert!(add_resource(&db, "data", "does-not-exist", RelationshipType::Uses, None).is_err());
        assert!(add_resource(&db, "missing", "anything", RelationshipType::Uses, None).is_err());
        assert!(delete(&db, "missing").is_err());
    }

    #[test]
    fn mutations_are_recorded_in_history() {
        let db = temp_db();
        create(&db, "work", None).unwrap();
        let pkg = resources::create(db.conn(), ResourceType::Package, "code", None).unwrap();
        add_resource(&db, "work", "code", RelationshipType::Owns, None).unwrap();
        remove_resource(&db, "work", "code").unwrap();
        delete(&db, "work").unwrap();

        let actions: Vec<String> = history::list(db.conn(), 10)
            .unwrap()
            .into_iter()
            .map(|entry| entry.action)
            .collect();
        assert!(actions.contains(&"domain.create".to_string()));
        assert!(actions.contains(&"domain.add".to_string()));
        assert!(actions.contains(&"domain.remove".to_string()));
        assert!(actions.contains(&"domain.delete".to_string()));
        // Resources are untouched by domain deletion.
        assert!(resources::get(db.conn(), &pkg.id).unwrap().is_some());
    }
}
