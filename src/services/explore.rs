use crate::core::{Observation, RelationshipType, Resource, ResourceType};
use crate::errors::Result;
use crate::storage::{observations, relationships, resources, roots, Database};
use std::collections::{HashMap, HashSet};

/// The resource kinds the Explore views can list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreKind {
    Packages,
    Services,
    Flatpaks,
    Repositories,
}

impl ExploreKind {
    /// Human-readable title for the view.
    pub fn title(self) -> &'static str {
        match self {
            Self::Packages => "Packages",
            Self::Services => "Services",
            Self::Flatpaks => "Flatpaks",
            Self::Repositories => "Repositories",
        }
    }

    fn resource_type(self) -> ResourceType {
        match self {
            Self::Packages => ResourceType::Package,
            Self::Services => ResourceType::Service,
            Self::Flatpaks => ResourceType::Flatpak,
            Self::Repositories => ResourceType::Repository,
        }
    }
}

/// One row in an Explore view.
#[derive(Debug, Clone)]
pub struct ExploreItem {
    pub resource: Resource,
    pub version: Option<String>,
    pub active: Option<bool>,
    pub enabled: Option<bool>,
    pub failed: Option<bool>,
    /// Origin repository or remote, when known.
    pub source: Option<String>,
    /// Flatpak kind/branch summary (e.g. "runtime · stable").
    pub detail: Option<String>,
    /// Number of packages coming from this repository (repositories only).
    pub package_count: Option<usize>,
    pub missing: bool,
    pub is_root: bool,
    /// Whether a normal user would consider this intentional: a root, or a
    /// repository they added themselves (not a Fedora/RPM Fusion base repo).
    pub is_user: bool,
}

/// A complete Explore listing.
#[derive(Debug, Clone)]
pub struct ExploreView {
    pub kind: ExploreKind,
    pub items: Vec<ExploreItem>,
}

/// Build an Explore view from the recorded model.
///
/// This is database-backed and therefore fast and read-only; run a scan to
/// refresh what it shows.
pub fn build(db: &Database, kind: ExploreKind) -> Result<ExploreView> {
    let conn = db.conn();

    let root_ids: HashSet<String> = roots::root_ids(conn)?.into_iter().collect();
    let observation_map: HashMap<String, Observation> = observations::list_all(conn)?
        .into_iter()
        .map(|observation| (observation.resource_id.clone(), observation))
        .collect();
    let resource_map: HashMap<String, Resource> = resources::list(conn)?
        .into_iter()
        .map(|resource| (resource.id.clone(), resource))
        .collect();
    let comes_from = relationships::list_by_type(conn, RelationshipType::ComesFrom)?;

    let mut items = Vec::new();
    for resource in resources::list_by_type(conn, kind.resource_type())? {
        let observation = observation_map.get(&resource.id);

        let source = comes_from
            .iter()
            .find(|rel| rel.source_id == resource.id)
            .and_then(|rel| resource_map.get(&rel.target_id))
            .map(|origin| {
                origin
                    .display_name
                    .clone()
                    .unwrap_or_else(|| origin.native_id.clone())
            });

        let package_count = if kind == ExploreKind::Repositories {
            Some(
                comes_from
                    .iter()
                    .filter(|rel| rel.target_id == resource.id)
                    .count(),
            )
        } else {
            None
        };

        let is_root = root_ids.contains(&resource.id);
        let is_user = if kind == ExploreKind::Repositories {
            !is_system_repository(&resource.native_id)
        } else {
            is_root
        };

        items.push(ExploreItem {
            version: observation.and_then(|obs| obs.version.clone()),
            active: observation.and_then(|obs| obs.active),
            enabled: observation.and_then(|obs| obs.enabled),
            failed: observation.and_then(|obs| obs.failed),
            source,
            detail: observation.and_then(flatpak_detail),
            package_count,
            missing: observation.and_then(|obs| obs.installed) == Some(false),
            is_root,
            is_user,
            resource,
        });
    }

    items.sort_by_key(|item| item_label(item).to_lowercase());

    Ok(ExploreView { kind, items })
}

/// Display label for an item: "Visual Studio Code (code)" when they differ.
pub fn item_label(item: &ExploreItem) -> String {
    match item.resource.display_name.as_deref() {
        Some(name) if name != item.resource.native_id => {
            format!("{} ({})", name, item.resource.native_id)
        }
        _ => item.resource.native_id.clone(),
    }
}

/// Base distribution repositories. Everything else (COPR, vendor repos like
/// docker-ce, code, brave) was added by the user.
fn is_system_repository(native_id: &str) -> bool {
    let id = native_id.to_ascii_lowercase();
    ["fedora", "updates", "rpmfusion"]
        .iter()
        .any(|prefix| id.starts_with(prefix))
}

fn flatpak_detail(observation: &Observation) -> Option<String> {
    let metadata = observation.metadata.as_ref()?;
    let kind = metadata.get("kind").and_then(|value| value.as_str());
    let branch = metadata.get("branch").and_then(|value| value.as_str());

    match (kind, branch) {
        (Some(kind), Some(branch)) => Some(format!("{} · {}", kind, branch)),
        (Some(kind), None) => Some(kind.to_string()),
        (None, Some(branch)) => Some(format!("application · {}", branch)),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{RelationshipOrigin, ResourceType};
    use crate::storage::{observations, resources};

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    fn observe(
        db: &Database,
        resource_id: &str,
        installed: Option<bool>,
        version: Option<&str>,
        active: Option<bool>,
        enabled: Option<bool>,
        metadata: Option<&str>,
    ) {
        observations::upsert(
            db.conn(),
            resource_id,
            installed,
            version,
            active,
            enabled,
            None,
            metadata,
        )
        .unwrap();
    }

    #[test]
    fn packages_include_version_origin_root_and_missing() {
        let db = temp_db();
        let repo = resources::create(db.conn(), ResourceType::Repository, "fedora", None).unwrap();
        let zsh = resources::create(db.conn(), ResourceType::Package, "zsh", Some("Zsh")).unwrap();
        let redis = resources::create(db.conn(), ResourceType::Package, "redis", None).unwrap();
        observe(&db, &zsh.id, Some(true), Some("5.9"), None, None, None);
        observe(&db, &redis.id, Some(false), Some("7.2.5"), None, None, None);
        relationships::create(
            db.conn(),
            &zsh.id,
            RelationshipType::ComesFrom,
            &repo.id,
            RelationshipOrigin::System,
        )
        .unwrap();
        roots::create(db.conn(), &zsh.id, crate::core::RootSource::Detected, None).unwrap();

        let view = build(&db, ExploreKind::Packages).unwrap();
        assert_eq!(view.items.len(), 2);

        let zsh_item = view
            .items
            .iter()
            .find(|item| item.resource.native_id == "zsh")
            .unwrap();
        assert_eq!(zsh_item.version.as_deref(), Some("5.9"));
        assert_eq!(zsh_item.source.as_deref(), Some("fedora"));
        assert!(zsh_item.is_root);
        assert!(!zsh_item.missing);

        let redis_item = view
            .items
            .iter()
            .find(|item| item.resource.native_id == "redis")
            .unwrap();
        assert!(redis_item.missing);
        assert!(!redis_item.is_root);

        // Packages: the intentional view is exactly the root set.
        assert!(zsh_item.is_user);
        assert!(!redis_item.is_user);
    }

    #[test]
    fn repositories_count_incoming_packages() {
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

        let view = build(&db, ExploreKind::Repositories).unwrap();
        assert_eq!(view.items.len(), 1);
        assert_eq!(view.items[0].package_count, Some(3));
    }

    #[test]
    fn repositories_distinguish_user_added_from_base() {
        let db = temp_db();
        for (repo, user) in [
            ("fedora", false),
            ("updates", false),
            ("rpmfusion-free", false),
            ("docker-ce-stable", true),
            ("copr:copr.fedorainfracloud.org:group:project", true),
            ("code", true),
        ] {
            resources::create(db.conn(), ResourceType::Repository, repo, None).unwrap();
            let view = build(&db, ExploreKind::Repositories).unwrap();
            let item = view
                .items
                .iter()
                .find(|item| item.resource.native_id == repo)
                .unwrap();
            assert_eq!(item.is_user, user, "repo {repo}");
        }
    }

    #[test]
    fn services_report_state() {
        let db = temp_db();
        let svc =
            resources::create(db.conn(), ResourceType::Service, "firewalld.service", None).unwrap();
        observe(&db, &svc.id, None, None, Some(true), Some(true), None);

        let view = build(&db, ExploreKind::Services).unwrap();
        assert_eq!(view.items[0].active, Some(true));
        assert_eq!(view.items[0].enabled, Some(true));
        assert!(!view.items[0].missing);
    }

    #[test]
    fn flatpaks_include_kind_branch_and_origin() {
        let db = temp_db();
        let remote =
            resources::create(db.conn(), ResourceType::Repository, "flathub", None).unwrap();
        let app = resources::create(
            db.conn(),
            ResourceType::Flatpak,
            "org.mozilla.firefox",
            Some("Firefox"),
        )
        .unwrap();
        observe(
            &db,
            &app.id,
            Some(true),
            Some("139.0"),
            None,
            None,
            Some(r#"{"branch":"stable","kind":"application"}"#),
        );
        relationships::create(
            db.conn(),
            &app.id,
            RelationshipType::ComesFrom,
            &remote.id,
            RelationshipOrigin::System,
        )
        .unwrap();

        let view = build(&db, ExploreKind::Flatpaks).unwrap();
        let item = &view.items[0];
        assert_eq!(item.version.as_deref(), Some("139.0"));
        assert_eq!(item.detail.as_deref(), Some("application · stable"));
        assert_eq!(item.source.as_deref(), Some("flathub"));
    }
}
