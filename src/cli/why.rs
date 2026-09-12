use crate::core::graph::NodeType;
use crate::core::graph::SystemGraph;
use crate::core::RelationshipType;
use crate::errors::{ChapeauError, Result};
use crate::storage::Database;
use crate::storage::{observations, resources, roots};

pub fn run(db: &Database, resource_name: &str) -> Result<()> {
    // Find the resource
    let resource = resources::find_by_native_id(db.conn(), resource_name)?
        .ok_or_else(|| ChapeauError::ResourceNotFound(resource_name.to_string()))?;

    // Build graph
    let graph = SystemGraph::from_db(db)?;

    let idx = graph
        .node_index(&resource.id)
        .ok_or_else(|| ChapeauError::ResourceNotFound(resource_name.to_string()))?;

    let node = &graph.graph[idx];

    // Check if this is a root
    let root = roots::get(db.conn(), &resource.id)?;
    let observation = observations::get(db.conn(), &resource.id)?;

    // Package metadata recorded during the last scan (summary, role, reason).
    let metadata = observation.as_ref().and_then(|obs| obs.metadata.as_ref());
    let role = metadata
        .and_then(|meta| meta.get("role"))
        .and_then(|value| value.as_str());
    let dnf_reason = metadata
        .and_then(|meta| meta.get("reason"))
        .and_then(|value| value.as_str());
    let summary = metadata
        .and_then(|meta| meta.get("summary"))
        .and_then(|value| value.as_str());
    let install_date = metadata
        .and_then(|meta| meta.get("install_time"))
        .and_then(|value| value.as_i64())
        .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
        .map(|date| date.format("%Y-%m-%d").to_string());

    println!("{}", node.label);
    if let Some(summary) = summary {
        println!("  {}", summary);
    }
    println!();

    // Root status / classification explanation
    if let Some(ref root) = root {
        let missing = observation.as_ref().and_then(|obs| obs.installed) == Some(false);
        if missing {
            println!("Intentional resource (missing)");
        } else {
            println!("Intentional resource");
        }
        println!("  Source: {}", root.source);
        if let Some(ref reason) = root.reason {
            println!("  Detected from: {}", reason);
        }
        if let Some(reason) = dnf_reason {
            println!("  DNF install reason: {}", reason);
        }
        if let Some(date) = install_date.as_deref() {
            println!("  Installed: {}", date);
        }
        if let Some(role) = role {
            println!("  Package role: {}", role);
        }
        if missing {
            println!();
            println!("This resource is recorded as intentional, but it is not currently");
            println!("present on the system.");
            println!(
                "Reinstall it, or run 'chapeau root remove {}' if it is no longer wanted.",
                resource.native_id
            );
        }
        println!();
    } else {
        let dependents = crate::core::analysis::get_dependent_names(db.conn(), &resource.id)?;
        let is_supporting = role.is_some_and(|role| role != "application");

        if is_supporting {
            println!("Supporting resource");
        } else {
            println!("Observed system resource");
        }
        println!();
        println!("Not shown as an intentional root because:");
        if let Some(role) = role {
            println!("  Package role: {}", role);
        }
        if !dependents.is_empty() {
            println!("  Required by ({}):", dependents.len());
            for dependent in dependents.iter().take(10) {
                println!("    {}", dependent);
            }
            if dependents.len() > 10 {
                println!("    ... and {} more", dependents.len() - 10);
            }
        }
        if role.is_none() && dependents.is_empty() {
            println!("  No root evidence recorded (run 'chapeau scan')");
        }
        println!();
    }

    // Status from observation
    if let Some(obs) = observation {
        println!("Status:");
        if let Some(installed) = obs.installed {
            println!("  installed: {}", if installed { "yes" } else { "no" });
        }
        if let Some(version) = &obs.version {
            println!("  version:   {}", version);
        }
        if let Some(active) = obs.active {
            println!("  active:    {}", if active { "yes" } else { "no" });
        }
        if let Some(enabled) = obs.enabled {
            println!("  enabled:   {}", if enabled { "yes" } else { "no" });
        }
        if let Some(failed) = obs.failed {
            if failed {
                println!("  failed:    yes");
            }
        }
        println!();
    }

    // Provenance: where did this come from?
    let incoming = graph.incoming(idx);
    let comes_from: Vec<_> = incoming
        .iter()
        .filter(|(_, rel, _)| **rel == RelationshipType::ComesFrom)
        .collect();
    if !comes_from.is_empty() {
        println!("Provenance:");
        for (_, _, src) in &comes_from {
            println!("  {}", src.label);
        }
        println!();
    }

    // What does this resource depend on?
    let outgoing = graph.outgoing(idx);
    let dependencies: Vec<_> = outgoing
        .iter()
        .filter(|(_, rel, _)| **rel == RelationshipType::DependsOn)
        .collect();
    if !dependencies.is_empty() {
        println!("Depends on ({} packages):", dependencies.len());
        // Show first 10, then summary
        for (_, _, tgt) in dependencies.iter().take(10) {
            println!("  {}", tgt.label);
        }
        if dependencies.len() > 10 {
            println!("  ... and {} more", dependencies.len() - 10);
        }
        println!();
    }

    // What depends on this resource?
    let incoming_deps: Vec<_> = incoming
        .iter()
        .filter(|(_, rel, _)| **rel == RelationshipType::DependsOn)
        .collect();
    if !incoming_deps.is_empty() {
        println!("Required by ({} resources):", incoming_deps.len());
        // Show first 10, then summary
        for (_, _, src) in incoming_deps.iter().take(10) {
            println!("  {}", src.label);
        }
        if incoming_deps.len() > 10 {
            println!("  ... and {} more", incoming_deps.len() - 10);
        }
        println!();
    }

    // What uses this resource?
    let incoming_uses: Vec<_> = incoming
        .iter()
        .filter(|(_, rel, _)| **rel == RelationshipType::Uses)
        .collect();
    if !incoming_uses.is_empty() {
        println!("Used by:");
        for (_, _, src) in &incoming_uses {
            if src.node_type == NodeType::Domain {
                println!("  {} (domain)", src.label);
            } else {
                println!("  {}", src.label);
            }
        }
        println!();
    }

    // What domains own this resource?
    let incoming_owns: Vec<_> = incoming
        .iter()
        .filter(|(_, rel, _)| **rel == RelationshipType::Owns)
        .collect();
    if !incoming_owns.is_empty() {
        println!("Owned by:");
        for (_, _, src) in &incoming_owns {
            println!("  {} (domain)", src.label);
        }
        println!();
    }

    // What does this resource provide?
    let outgoing_provides: Vec<_> = outgoing
        .iter()
        .filter(|(_, rel, _)| **rel == RelationshipType::Provides)
        .collect();
    if !outgoing_provides.is_empty() {
        println!("Provides:");
        for (_, _, tgt) in &outgoing_provides {
            println!("  {}", tgt.label);
        }
        println!();
    }

    // What does this resource use?
    let outgoing_uses: Vec<_> = outgoing
        .iter()
        .filter(|(_, rel, _)| **rel == RelationshipType::Uses)
        .collect();
    if !outgoing_uses.is_empty() {
        println!("Uses:");
        for (_, _, tgt) in &outgoing_uses {
            if tgt.node_type == NodeType::Domain {
                println!("  {} (domain)", tgt.label);
            } else {
                println!("  {}", tgt.label);
            }
        }
        println!();
    }

    // What domains does this resource belong to?
    let outgoing_owns: Vec<_> = outgoing
        .iter()
        .filter(|(_, rel, _)| **rel == RelationshipType::Owns)
        .collect();
    if !outgoing_owns.is_empty() {
        println!("In domains:");
        for (_, _, tgt) in &outgoing_owns {
            println!("  {}", tgt.label);
        }
        println!();
    }

    Ok(())
}
