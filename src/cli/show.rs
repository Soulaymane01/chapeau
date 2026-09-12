use crate::core::root::Root;
use crate::core::{Domain, Observation, RelationshipType, Resource, ResourceType};
use crate::errors::{ChapeauError, Result};
use crate::storage::Database;
use crate::storage::{domains, observations, relationships, resources, roots};

/// Maximum names shown in relationship lists before truncating.
const LIST_LIMIT: usize = 10;

/// Show detailed information about a single resource.
pub fn run(db: &Database, resource_name: &str) -> Result<()> {
    let resource = resources::find_by_native_id(db.conn(), resource_name)?
        .ok_or_else(|| ChapeauError::ResourceNotFound(resource_name.to_string()))?;
    let detail = gather(db, &resource)?;
    print_detail(&detail);
    Ok(())
}

/// Everything `show` needs about one resource. Separated from printing so the
/// gathering can be tested without capturing stdout.
#[derive(Debug)]
pub(crate) struct ResourceDetail {
    pub resource: Resource,
    pub observation: Option<Observation>,
    pub root: Option<Root>,
    pub domains: Vec<(Domain, RelationshipType)>,
    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,
    pub provenance: Vec<String>,
    pub uses: Vec<String>,
    /// Number of packages coming from this repository (repositories only).
    pub package_count: Option<usize>,
}

pub(crate) fn gather(db: &Database, resource: &Resource) -> Result<ResourceDetail> {
    let conn = db.conn();

    let observation = observations::get(conn, &resource.id)?;
    let root = roots::get(conn, &resource.id)?;
    let domain_list = domains::list_for_resource(conn, &resource.id)?;

    let outgoing = relationships::list_from(conn, &resource.id)?;
    let incoming = relationships::list_to(conn, &resource.id)?;

    let mut dependencies = Vec::new();
    for rel in outgoing
        .iter()
        .filter(|rel| rel.relationship_type == RelationshipType::DependsOn)
    {
        if let Some(target) = resources::get(conn, &rel.target_id)? {
            dependencies.push(target.display_name.unwrap_or(target.native_id));
        }
    }

    let mut dependents = Vec::new();
    for rel in incoming
        .iter()
        .filter(|rel| rel.relationship_type == RelationshipType::DependsOn)
    {
        if let Some(source) = resources::get(conn, &rel.source_id)? {
            dependents.push(source.display_name.unwrap_or(source.native_id));
        }
    }

    let mut provenance = Vec::new();
    for rel in outgoing
        .iter()
        .filter(|rel| rel.relationship_type == RelationshipType::ComesFrom)
    {
        if let Some(target) = resources::get(conn, &rel.target_id)? {
            provenance.push(target.display_name.unwrap_or(target.native_id));
        }
    }

    let mut uses = Vec::new();
    for rel in outgoing
        .iter()
        .filter(|rel| rel.relationship_type == RelationshipType::Uses)
    {
        if let Some(target) = resources::get(conn, &rel.target_id)? {
            uses.push(target.display_name.unwrap_or(target.native_id));
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
        dependencies,
        dependents,
        provenance,
        uses,
        package_count,
    })
}

fn print_detail(detail: &ResourceDetail) {
    let resource = &detail.resource;
    let title = resource
        .display_name
        .as_deref()
        .unwrap_or(&resource.native_id);

    println!("{}", title);
    if title != resource.native_id {
        println!("  {}", resource.native_id);
    }
    if let Some(summary) = metadata_str(detail, "summary") {
        println!("  {}", summary);
    }
    println!();

    let missing = detail.observation.as_ref().and_then(|obs| obs.installed) == Some(false);

    println!("Type:         {}", resource.resource_type);
    print_type_facts(detail, missing);
    if !detail.provenance.is_empty() {
        println!("Origin:       {}", detail.provenance.join(", "));
    }
    if resource.resource_type != ResourceType::Repository {
        print_intent(detail);
    }
    if !detail.domains.is_empty() {
        let memberships: Vec<String> = detail
            .domains
            .iter()
            .map(|(domain, relationship)| format!("{} ({})", domain.name, relationship))
            .collect();
        println!("Domains:      {}", memberships.join(", "));
    }

    if missing && detail.root.is_some() {
        println!();
        println!("Intentional but missing:");
        println!("  This resource is recorded as intentional, but it is not currently");
        println!("  present on the system.");
        println!(
            "  Reinstall it, or run 'chapeau root remove {}' if it is no longer wanted.",
            resource.native_id
        );
    }

    print_name_list("Dependencies", &detail.dependencies);
    print_name_list("Required by", &detail.dependents);
    print_name_list("Uses", &detail.uses);

    println!();
    println!("Inspect further:");
    println!("  chapeau why {}", resource.native_id);
    if !detail.dependencies.is_empty() {
        println!("  chapeau dependencies {}", resource.native_id);
    }
    if !detail.dependents.is_empty() {
        println!("  chapeau dependents {}", resource.native_id);
    }
    if resource.resource_type == ResourceType::Package {
        println!(
            "  chapeau remove {}   (preview removal impact)",
            resource.native_id
        );
    }
}

fn print_type_facts(detail: &ResourceDetail, missing: bool) {
    match detail.resource.resource_type {
        ResourceType::Package => {
            if let Some(version) = detail
                .observation
                .as_ref()
                .and_then(|obs| obs.version.as_deref())
            {
                if missing {
                    println!("Version:      {} (last seen)", version);
                } else {
                    println!("Version:      {}", version);
                }
            }
            println!(
                "Recorded:     {}",
                recorded_state(detail.observation.as_ref())
            );
        }
        ResourceType::Service => {
            println!(
                "State:        {}",
                service_state(detail.observation.as_ref())
            );
        }
        ResourceType::Flatpak => {
            if let Some(version) = detail
                .observation
                .as_ref()
                .and_then(|obs| obs.version.as_deref())
            {
                println!("Version:      {}", version);
            }
            if let Some(branch) = metadata_str(detail, "branch") {
                println!("Branch:       {}", branch);
            }
            println!(
                "Recorded:     {}",
                recorded_state(detail.observation.as_ref())
            );
        }
        ResourceType::Repository => {
            if let Some(count) = detail.package_count {
                println!("Packages:     {}", count);
            }
        }
    }
}

fn print_intent(detail: &ResourceDetail) {
    match &detail.root {
        Some(root) => {
            println!("Intent:       intentional ({})", root.source);
            if let Some(reason) = &root.reason {
                println!("              {}", reason);
            }
        }
        None => match metadata_str(detail, "role") {
            Some(role) => println!("Intent:       not a root (role: {})", role),
            None => println!("Intent:       not a root"),
        },
    }
}

fn print_name_list(title: &str, names: &[String]) {
    if names.is_empty() {
        return;
    }
    println!();
    println!("{} ({}):", title, names.len());
    for name in names.iter().take(LIST_LIMIT) {
        println!("  {}", name);
    }
    if names.len() > LIST_LIMIT {
        println!("  ... and {} more", names.len() - LIST_LIMIT);
    }
}

fn recorded_state(observation: Option<&Observation>) -> String {
    match observation.and_then(|obs| obs.installed) {
        Some(true) => "installed".to_string(),
        Some(false) => "missing — not currently present on the system".to_string(),
        None => "not recorded".to_string(),
    }
}

fn service_state(observation: Option<&Observation>) -> String {
    let Some(observation) = observation else {
        return "unknown".to_string();
    };

    let mut parts = Vec::new();
    match observation.active {
        Some(true) => parts.push("active"),
        Some(false) => parts.push("inactive"),
        None => {}
    }
    match observation.enabled {
        Some(true) => parts.push("enabled"),
        Some(false) => parts.push("disabled"),
        None => {}
    }
    if observation.failed == Some(true) {
        parts.push("failed");
    }
    if parts.is_empty() {
        "unknown".to_string()
    } else {
        parts.join(", ")
    }
}

fn metadata_str<'a>(detail: &'a ResourceDetail, key: &str) -> Option<&'a str> {
    detail
        .observation
        .as_ref()
        .and_then(|obs| obs.metadata.as_ref())
        .and_then(|meta| meta.get(key))
        .and_then(|value| value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::root::RootSource;
    use crate::core::{RelationshipOrigin, RelationshipType};
    use crate::storage::{domains, observations, relationships, resources};

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
        assert_eq!(detail.domains.len(), 1);
        assert_eq!(detail.domains[0].0.name, "databases");
        assert_eq!(detail.domains[0].1, RelationshipType::Owns);
        assert!(detail.root.is_some());
        assert_eq!(metadata_str(&detail, "summary"), Some("PostgreSQL server"));
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
    fn recorded_state_reports_missing() {
        let mut missing = Observation::new("res".into());
        missing.installed = Some(false);
        assert!(recorded_state(Some(&missing)).starts_with("missing"));
        assert_eq!(recorded_state(None), "not recorded");
    }

    #[test]
    fn service_state_formats_fields() {
        let mut obs = Observation::new("svc".into());
        obs.active = Some(true);
        obs.enabled = Some(false);
        assert_eq!(service_state(Some(&obs)), "active, disabled");

        obs.failed = Some(true);
        assert_eq!(service_state(Some(&obs)), "active, disabled, failed");
        assert_eq!(service_state(None), "unknown");
    }
}
