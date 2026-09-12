use crate::core::ResourceType;
use crate::errors::Result;
use crate::storage::Database;
use crate::storage::{domains, observations, relationships, resources};
use std::collections::{HashMap, HashSet};

/// Summary of what Chapeau knows about the system.
#[derive(Debug, Clone, Default)]
pub struct StatusSummary {
    pub total_resources: usize,
    pub packages: usize,
    pub services: usize,
    pub flatpaks: usize,
    pub repositories: usize,
    pub domains: usize,
    /// Resources with no relationships at all.
    pub untracked: usize,
    /// Resources recorded as currently absent.
    pub missing: i64,
    pub relationships: usize,
    pub observations: i64,
    pub active_services: usize,
    pub enabled_services: usize,
    pub failed_services: usize,
    pub repository_names: Vec<String>,
}

/// Assemble the status summary from the recorded model.
pub fn summary(db: &Database) -> Result<StatusSummary> {
    let conn = db.conn();

    let resources_list = resources::list(conn)?;
    let domains_list = domains::list(conn)?;
    let relationships_list = relationships::list(conn)?;

    let mut by_type: HashMap<ResourceType, usize> = HashMap::new();
    for resource in &resources_list {
        *by_type.entry(resource.resource_type).or_insert(0) += 1;
    }

    let mut related_ids = HashSet::new();
    for relationship in &relationships_list {
        related_ids.insert(relationship.source_id.clone());
        related_ids.insert(relationship.target_id.clone());
    }
    let untracked = resources_list
        .iter()
        .filter(|resource| !related_ids.contains(&resource.id))
        .count();

    let mut active_services = 0;
    let mut enabled_services = 0;
    let mut failed_services = 0;
    for service in resources::list_by_type(conn, ResourceType::Service)? {
        if let Ok(Some(observation)) = observations::get(conn, &service.id) {
            if observation.active == Some(true) {
                active_services += 1;
            }
            if observation.enabled == Some(true) {
                enabled_services += 1;
            }
            if observation.failed == Some(true) {
                failed_services += 1;
            }
        }
    }

    let repository_names = resources::list_by_type(conn, ResourceType::Repository)?
        .into_iter()
        .map(|repo| repo.native_id)
        .collect();

    Ok(StatusSummary {
        total_resources: resources_list.len(),
        packages: by_type.get(&ResourceType::Package).copied().unwrap_or(0),
        services: by_type.get(&ResourceType::Service).copied().unwrap_or(0),
        flatpaks: by_type.get(&ResourceType::Flatpak).copied().unwrap_or(0),
        repositories: by_type.get(&ResourceType::Repository).copied().unwrap_or(0),
        domains: domains_list.len(),
        untracked,
        missing: observations::count_absent(conn)?,
        relationships: relationships_list.len(),
        observations: observations::count(conn)?,
        active_services,
        enabled_services,
        failed_services,
        repository_names,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{RelationshipOrigin, RelationshipType, ResourceType};
    use crate::storage::{observations, relationships, resources};

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    #[test]
    fn summary_counts_resources_and_services() {
        let db = temp_db();
        let pkg = resources::create(db.conn(), ResourceType::Package, "zsh", None).unwrap();
        let repo = resources::create(db.conn(), ResourceType::Repository, "fedora", None).unwrap();
        let svc =
            resources::create(db.conn(), ResourceType::Service, "firewalld.service", None).unwrap();
        relationships::create(
            db.conn(),
            &pkg.id,
            RelationshipType::ComesFrom,
            &repo.id,
            RelationshipOrigin::System,
        )
        .unwrap();
        observations::upsert(
            db.conn(),
            &pkg.id,
            Some(true),
            Some("5.9"),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        observations::upsert(
            db.conn(),
            &svc.id,
            None,
            None,
            Some(true),
            Some(true),
            Some(false),
            None,
        )
        .unwrap();

        let summary = summary(&db).unwrap();
        assert_eq!(summary.total_resources, 3);
        assert_eq!(summary.packages, 1);
        assert_eq!(summary.services, 1);
        assert_eq!(summary.repositories, 1);
        assert_eq!(summary.relationships, 1);
        assert_eq!(summary.observations, 2);
        assert_eq!(summary.active_services, 1);
        assert_eq!(summary.enabled_services, 1);
        assert_eq!(summary.failed_services, 0);
        assert_eq!(summary.repository_names, vec!["fedora"]);
        // The service has no relationships, so it is untracked.
        assert_eq!(summary.untracked, 1);
    }

    #[test]
    fn summary_counts_missing() {
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

        let summary = summary(&db).unwrap();
        assert_eq!(summary.missing, 1);
    }
}
