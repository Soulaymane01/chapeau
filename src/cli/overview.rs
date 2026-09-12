use crate::core::root::RootSource;
use crate::core::{Resource, ResourceType};
use crate::errors::Result;
use crate::storage::Database;
use crate::storage::{domains, relationships, resources, roots};
use std::collections::HashSet;

/// Maximum ungrouped roots listed in the overview when domains exist.
const UNGROUPED_LIMIT: usize = 12;

/// Show the "My System" overview: intentional resources, grouped by domain
/// when domains exist, otherwise grouped by resource type.
pub fn run(db: &Database) -> Result<()> {
    let view = build(db)?;
    print_view(&view);
    Ok(())
}

/// A root as shown in the overview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RootEntry {
    pub resource: Resource,
    pub source: RootSource,
    pub missing: bool,
}

impl RootEntry {
    /// Display label: "Visual Studio Code (code)" when they differ.
    pub fn label(&self) -> String {
        match self.resource.display_name.as_deref() {
            Some(name) if name != self.resource.native_id => {
                format!("{} ({})", name, self.resource.native_id)
            }
            _ => self.resource.native_id.clone(),
        }
    }

    /// Source tag, e.g. `[user]` or `[detected, missing]`.
    pub fn tag(&self) -> String {
        if self.missing {
            format!("[{}, missing]", self.source)
        } else {
            format!("[{}]", self.source)
        }
    }
}

/// One section of the overview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Section {
    pub title: String,
    pub entries: Vec<RootEntry>,
    /// Whether the section is truncated in the default view (the full list
    /// remains available via `chapeau roots`).
    pub collapsed: bool,
}

/// The data behind the overview.
#[derive(Debug, Clone, Default)]
pub(crate) struct Overview {
    pub sections: Vec<Section>,
    pub has_domains: bool,
    pub root_count: usize,
    pub missing_count: usize,
    pub resource_count: usize,
    pub relationship_count: usize,
}

/// Build the overview from the database. Kept separate from printing so the
/// grouping can be tested without capturing stdout.
pub(crate) fn build(db: &Database) -> Result<Overview> {
    let conn = db.conn();

    let missing_ids: HashSet<String> = roots::list_missing(conn)?
        .into_iter()
        .map(|root| root.resource_id)
        .collect();

    let mut entries: Vec<RootEntry> = Vec::new();
    for root in roots::list(conn)? {
        if let Some(resource) = resources::get(conn, &root.resource_id)? {
            entries.push(RootEntry {
                missing: missing_ids.contains(&resource.id),
                resource,
                source: root.source,
            });
        }
    }

    let domain_list = domains::list(conn)?;
    let has_domains = !domain_list.is_empty();

    let sections = if has_domains {
        domain_sections(db, &domain_list, &entries)?
    } else {
        type_sections(&entries)
    };

    Ok(Overview {
        sections,
        has_domains,
        root_count: entries.len(),
        missing_count: entries.iter().filter(|entry| entry.missing).count(),
        resource_count: resources::count(conn)? as usize,
        relationship_count: relationships::count(conn)? as usize,
    })
}

/// Group roots by domain. Roots in no domain fall into a final section.
fn domain_sections(
    db: &Database,
    domain_list: &[crate::core::Domain],
    entries: &[RootEntry],
) -> Result<Vec<Section>> {
    let conn = db.conn();
    let mut grouped: HashSet<String> = HashSet::new();
    let mut sections = Vec::new();

    for domain in domain_list {
        let members = domains::list_resources(conn, &domain.id)?;
        let mut domain_entries: Vec<RootEntry> = Vec::new();
        for (member, _relationship) in &members {
            if let Some(entry) = entries.iter().find(|entry| entry.resource.id == member.id) {
                domain_entries.push(entry.clone());
                grouped.insert(member.id.clone());
            }
        }
        if !domain_entries.is_empty() {
            sort_entries(&mut domain_entries);
            sections.push(Section {
                title: domain.name.clone(),
                entries: domain_entries,
                collapsed: false,
            });
        }
    }

    let mut ungrouped: Vec<RootEntry> = entries
        .iter()
        .filter(|entry| !grouped.contains(&entry.resource.id))
        .cloned()
        .collect();
    if !ungrouped.is_empty() {
        sort_entries(&mut ungrouped);
        sections.push(Section {
            title: "Not in a domain".to_string(),
            entries: ungrouped,
            collapsed: true,
        });
    }

    Ok(sections)
}

/// Group roots by resource type (used before any domains are configured).
fn type_sections(entries: &[RootEntry]) -> Vec<Section> {
    let groups = [
        (ResourceType::Package, "Packages"),
        (ResourceType::Flatpak, "Flatpak"),
        (ResourceType::Service, "Services"),
        (ResourceType::Repository, "Repositories"),
    ];

    let mut sections = Vec::new();
    for (resource_type, title) in groups {
        let mut group: Vec<RootEntry> = entries
            .iter()
            .filter(|entry| entry.resource.resource_type == resource_type)
            .cloned()
            .collect();
        if !group.is_empty() {
            sort_entries(&mut group);
            sections.push(Section {
                title: title.to_string(),
                entries: group,
                collapsed: false,
            });
        }
    }
    sections
}

fn sort_entries(entries: &mut [RootEntry]) {
    entries.sort_by_key(|entry| entry.label().to_lowercase());
}

fn print_view(view: &Overview) {
    println!("My System");
    println!("=========");
    println!();

    if view.root_count == 0 {
        println!("No intentional resources yet.");
        println!("Run 'chapeau scan' to classify installed software into intentional resources.");
        println!("Or add one with 'chapeau root add <resource>'.");
        return;
    }

    for section in &view.sections {
        println!("{} ({})", section.title, section.entries.len());
        let width = section
            .entries
            .iter()
            .map(|entry| entry.label().chars().count())
            .max()
            .unwrap_or(0);
        let shown = if section.collapsed {
            &section.entries[..section.entries.len().min(UNGROUPED_LIMIT)]
        } else {
            &section.entries[..]
        };
        for entry in shown {
            println!("  {:<width$} {}", entry.label(), entry.tag(), width = width);
        }
        if section.collapsed && section.entries.len() > UNGROUPED_LIMIT {
            println!(
                "  ... and {} more (see 'chapeau roots')",
                section.entries.len() - UNGROUPED_LIMIT
            );
        }
        println!();
    }

    let mut summary = format!("{} intentional resources", view.root_count);
    if view.missing_count > 0 {
        summary.push_str(&format!(" ({} missing)", view.missing_count));
    }
    println!(
        "{} · {} tracked resources · {} relationships",
        summary, view.resource_count, view.relationship_count
    );
    println!();

    println!("Inspect:   chapeau show <resource> · chapeau why <resource> · chapeau drift");
    println!("Explore:   chapeau packages --all · chapeau services --all · chapeau flatpaks --all");
    if view.has_domains {
        println!("Organize:  chapeau domains · chapeau domains add <domain> <resource>");
    } else {
        println!(
            "Organize:  chapeau domains create <name> · chapeau domains add <domain> <resource>"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::root::RootSource;
    use crate::storage::{domains, observations, resources, roots};

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    fn add_domain_resource(db: &Database, domain_id: &str, resource_id: &str, relationship: &str) {
        db.conn()
            .execute(
                "INSERT INTO domain_resources (domain_id, resource_id, relationship, created_at, updated_at)
                 VALUES (?1, ?2, ?3, datetime('now'), datetime('now'))",
                rusqlite::params![domain_id, resource_id, relationship],
            )
            .unwrap();
    }

    #[test]
    fn groups_by_type_without_domains() {
        let db = temp_db();
        let zsh = resources::create(db.conn(), ResourceType::Package, "zsh", Some("Zsh")).unwrap();
        let code =
            resources::create(db.conn(), ResourceType::Package, "code", Some("VS Code")).unwrap();
        let firefox = resources::create(
            db.conn(),
            ResourceType::Flatpak,
            "org.mozilla.firefox",
            Some("Firefox"),
        )
        .unwrap();
        roots::create(db.conn(), &zsh.id, RootSource::Detected, None).unwrap();
        roots::create(db.conn(), &code.id, RootSource::User, None).unwrap();
        roots::create(db.conn(), &firefox.id, RootSource::Detected, None).unwrap();

        let view = build(&db).unwrap();

        assert!(!view.has_domains);
        assert_eq!(view.root_count, 3);
        assert_eq!(view.sections.len(), 2);
        assert_eq!(view.sections[0].title, "Packages");
        assert_eq!(view.sections[0].entries.len(), 2);
        assert_eq!(view.sections[1].title, "Flatpak");
        assert_eq!(
            view.sections[1].entries[0].label(),
            "Firefox (org.mozilla.firefox)"
        );
    }

    #[test]
    fn groups_by_domain_when_domains_exist() {
        let db = temp_db();
        let databases = domains::create(db.conn(), "databases", None).unwrap();
        let postgres =
            resources::create(db.conn(), ResourceType::Package, "postgresql-server", None).unwrap();
        let zsh = resources::create(db.conn(), ResourceType::Package, "zsh", None).unwrap();
        roots::create(db.conn(), &postgres.id, RootSource::User, None).unwrap();
        roots::create(db.conn(), &zsh.id, RootSource::Detected, None).unwrap();
        add_domain_resource(&db, &databases.id, &postgres.id, "owns");

        let view = build(&db).unwrap();

        assert!(view.has_domains);
        assert_eq!(view.sections.len(), 2);
        assert_eq!(view.sections[0].title, "databases");
        assert_eq!(
            view.sections[0].entries[0].resource.native_id,
            "postgresql-server"
        );
        assert_eq!(view.sections[1].title, "Not in a domain");
        assert_eq!(view.sections[1].entries[0].resource.native_id, "zsh");
    }

    #[test]
    fn missing_roots_are_tagged_and_counted() {
        let db = temp_db();
        let pkg = resources::create(db.conn(), ResourceType::Package, "redis", None).unwrap();
        roots::create(db.conn(), &pkg.id, RootSource::User, None).unwrap();
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

        let view = build(&db).unwrap();
        assert_eq!(view.missing_count, 1);
        let entry = &view.sections[0].entries[0];
        assert!(entry.missing);
        assert_eq!(entry.tag(), "[user, missing]");
    }

    #[test]
    fn root_in_multiple_domains_is_shown_in_each() {
        let db = temp_db();
        let dev = domains::create(db.conn(), "development", None).unwrap();
        let ai = domains::create(db.conn(), "ai", None).unwrap();
        let python = resources::create(db.conn(), ResourceType::Package, "python3", None).unwrap();
        roots::create(db.conn(), &python.id, RootSource::Detected, None).unwrap();
        add_domain_resource(&db, &dev.id, &python.id, "owns");
        add_domain_resource(&db, &ai.id, &python.id, "uses");

        let view = build(&db).unwrap();
        let titles: Vec<&str> = view.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["ai", "development"]);
        assert_eq!(view.sections[0].entries[0].resource.native_id, "python3");
        assert_eq!(view.sections[1].entries[0].resource.native_id, "python3");
    }
}
