use crate::core::{Observation, ResourceType};
use crate::errors::{ChapeauError, Result};
use crate::services::detail::{self, ResourceDetail};
use crate::storage::resources;
use crate::storage::Database;

/// Maximum names shown in relationship lists before truncating.
const LIST_LIMIT: usize = 10;

/// Show detailed information about a single resource.
pub fn run(db: &Database, resource_name: &str) -> Result<()> {
    let resource = resources::find_by_native_id(db.conn(), resource_name)?
        .ok_or_else(|| ChapeauError::ResourceNotFound(resource_name.to_string()))?;
    let detail = detail::gather(db, &resource)?;
    print_detail(&detail);
    Ok(())
}

fn print_detail(detail: &ResourceDetail) {
    let resource = &detail.resource;
    let title = resource
        .display_name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(&resource.native_id);

    println!("{}", title);
    if title != resource.native_id {
        println!("  {}", resource.native_id);
    }
    if let Some(summary) = detail.metadata_str("summary") {
        println!("  {}", summary);
    }
    println!();

    let missing = detail.is_missing();

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
    print_name_list("Provides", &detail.provides);
    print_name_list("Provided by", &detail.provided_by);

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
            if let Some(date) = detail.install_date() {
                println!("Installed:    {}", date);
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
            if let Some(branch) = detail.metadata_str("branch") {
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
        None => match detail.metadata_str("role") {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Observation;

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
