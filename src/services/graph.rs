use crate::core::{RelationshipType, Resource};
use crate::errors::{ChapeauError, Result};
use crate::storage::{relationships, resources, roots, Database};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Write;
use std::process::{Command, Stdio};

/// Maximum nodes rendered in a neighborhood graph.
const MAX_NODES: usize = 200;

/// Options for building a dependency neighborhood.
#[derive(Debug, Clone, Copy)]
pub struct GraphOptions {
    /// How many dependency hops to follow from the root.
    pub depth: usize,
    /// Also follow reverse dependencies (what depends on the root).
    pub include_dependents: bool,
}

impl Default for GraphOptions {
    fn default() -> Self {
        Self {
            depth: 2,
            include_dependents: false,
        }
    }
}

/// A rendered neighborhood: its Graphviz source plus size counts.
#[derive(Debug, Clone)]
pub struct GraphView {
    pub dot: String,
    pub nodes: usize,
    pub edges: usize,
    pub truncated: bool,
}

/// Build the dependency neighborhood of a resource as Graphviz DOT.
///
/// Only `DependsOn` edges are followed; the graph is capped at
/// [`MAX_NODES`] to keep layouts readable.
pub fn neighborhood(
    db: &Database,
    resource_name: &str,
    options: GraphOptions,
) -> Result<GraphView> {
    let conn = db.conn();

    let start = resources::find_by_native_id(conn, resource_name)?
        .ok_or_else(|| ChapeauError::ResourceNotFound(resource_name.to_string()))?;
    let root_ids: HashSet<String> = roots::root_ids(conn)?.into_iter().collect();

    let mut nodes: HashMap<String, Resource> = HashMap::new();
    let mut depth_of: HashMap<String, usize> = HashMap::new();
    let mut edges: Vec<(String, String)> = Vec::new();
    let mut seen_edges: HashSet<(String, String)> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut truncated = false;

    nodes.insert(start.id.clone(), start.clone());
    depth_of.insert(start.id.clone(), 0);
    queue.push_back(start.id.clone());

    while let Some(id) = queue.pop_front() {
        let depth = depth_of.get(&id).copied().unwrap_or(0);
        if depth >= options.depth {
            continue;
        }

        let mut related: Vec<String> = Vec::new();
        for relationship in relationships::list_from(conn, &id)? {
            if relationship.relationship_type == RelationshipType::DependsOn {
                related.push(relationship.target_id);
            }
        }
        if options.include_dependents {
            for relationship in relationships::list_to(conn, &id)? {
                if relationship.relationship_type == RelationshipType::DependsOn {
                    related.push(relationship.source_id);
                }
            }
        }

        for related_id in related {
            let key = (id.clone(), related_id.clone());
            if seen_edges.insert(key) {
                edges.push((id.clone(), related_id.clone()));
            }
            if nodes.contains_key(&related_id) {
                continue;
            }
            if nodes.len() >= MAX_NODES {
                truncated = true;
                continue;
            }
            if let Some(resource) = resources::get(conn, &related_id)? {
                nodes.insert(related_id.clone(), resource);
                depth_of.insert(related_id.clone(), depth + 1);
                queue.push_back(related_id);
            }
        }
    }

    let dot = render_dot(&start, &nodes, &edges, &root_ids);
    Ok(GraphView {
        dot,
        nodes: nodes.len(),
        edges: edges.len(),
        truncated,
    })
}

/// Render DOT to PNG bytes using the `dot` binary.
pub fn render_png(dot: &str) -> Result<Vec<u8>> {
    let mut child = Command::new("dot")
        .arg("-Tpng")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| {
            ChapeauError::Backend(
                "graphviz 'dot' is not installed; install graphviz to view graphs".to_string(),
            )
        })?;

    child
        .stdin
        .as_mut()
        .expect("stdin was piped")
        .write_all(dot.as_bytes())?;

    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(ChapeauError::Backend(format!(
            "graphviz failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}

fn render_dot(
    start: &Resource,
    nodes: &HashMap<String, Resource>,
    edges: &[(String, String)],
    root_ids: &HashSet<String>,
) -> String {
    let mut dot = String::from("digraph chapeau {\n");
    dot.push_str("  rankdir=LR;\n");
    dot.push_str("  bgcolor=\"transparent\";\n");
    dot.push_str("  node [shape=box, style=\"rounded,filled\", fontname=\"sans\", fillcolor=\"#f6f5f4\", color=\"#c0bfbc\"];\n");
    dot.push_str("  edge [color=\"#77767b\", fontname=\"sans\", fontsize=10];\n\n");

    let mut ids: Vec<&String> = nodes.keys().collect();
    ids.sort_by_key(|id| nodes[*id].native_id.to_lowercase());

    for id in ids {
        let resource = &nodes[id];
        let label = resource
            .display_name
            .as_deref()
            .unwrap_or(&resource.native_id);
        let label = if label == resource.native_id {
            resource.native_id.clone()
        } else {
            format!("{label}\\n({})", resource.native_id)
        };

        let is_start = resource.id == start.id;
        let is_root = root_ids.contains(&resource.id);

        let mut attributes = format!("label=\"{}\"", escape_dot(&label));
        if is_start {
            attributes.push_str(", fillcolor=\"#3584e4\", fontcolor=\"white\", color=\"#1c71d8\"");
        } else if is_root {
            attributes.push_str(", penwidth=2");
        }
        dot.push_str(&format!(
            "  \"{}\" [{}];\n",
            escape_dot(&resource.id),
            attributes
        ));
    }

    dot.push('\n');
    for (source, target) in edges {
        dot.push_str(&format!(
            "  \"{}\" -> \"{}\";\n",
            escape_dot(source),
            escape_dot(target)
        ));
    }

    dot.push_str("}\n");
    dot
}

fn escape_dot(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{RelationshipOrigin, RelationshipType, ResourceType};
    use crate::storage::resources;

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    fn link(db: &Database, source: &str, target: &str) {
        relationships::create(
            db.conn(),
            source,
            RelationshipType::DependsOn,
            target,
            RelationshipOrigin::System,
        )
        .unwrap();
    }

    #[test]
    fn neighborhood_follows_dependencies_to_depth() {
        let db = temp_db();
        let app = resources::create(db.conn(), ResourceType::Package, "app", None).unwrap();
        let lib = resources::create(db.conn(), ResourceType::Package, "lib", None).unwrap();
        let base = resources::create(db.conn(), ResourceType::Package, "base", None).unwrap();
        let deep = resources::create(db.conn(), ResourceType::Package, "deep", None).unwrap();
        link(&db, &app.id, &lib.id);
        link(&db, &lib.id, &base.id);
        link(&db, &base.id, &deep.id);

        let view = neighborhood(
            &db,
            "app",
            GraphOptions {
                depth: 1,
                include_dependents: false,
            },
        )
        .unwrap();
        assert_eq!(view.nodes, 2);
        assert_eq!(view.edges, 1);
        assert!(view.dot.contains("app"));
        assert!(view.dot.contains("lib"));
        assert!(!view.dot.contains("base"));

        let view = neighborhood(&db, "app", GraphOptions::default()).unwrap();
        assert_eq!(view.nodes, 3);
        assert_eq!(view.edges, 2);
    }

    #[test]
    fn neighborhood_can_include_dependents() {
        let db = temp_db();
        let a = resources::create(db.conn(), ResourceType::Package, "a", None).unwrap();
        let b = resources::create(db.conn(), ResourceType::Package, "b", None).unwrap();
        let c = resources::create(db.conn(), ResourceType::Package, "c", None).unwrap();
        link(&db, &a.id, &b.id);
        link(&db, &c.id, &b.id);

        let view = neighborhood(
            &db,
            "b",
            GraphOptions {
                depth: 1,
                include_dependents: true,
            },
        )
        .unwrap();
        assert_eq!(view.nodes, 3);
        assert_eq!(view.edges, 2);
    }

    #[test]
    fn missing_resource_is_reported() {
        let db = temp_db();
        assert!(neighborhood(&db, "nope", GraphOptions::default()).is_err());
    }

    #[test]
    fn dot_escapes_quotes() {
        assert_eq!(escape_dot(r#"a"b\c"#), r#"a\"b\\c"#);
    }
}
