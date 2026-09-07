use crate::core::{RelationshipType, Resource, ResourceType};
use crate::errors::Result;
use crate::storage::Database;
use crate::storage::{relationships, resources};
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::Graph;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct NodeLabel {
    pub id: String,
    pub label: String,
    pub node_type: NodeType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeType {
    Resource(ResourceType),
    Domain,
}

impl NodeLabel {
    pub fn resource(r: &Resource) -> Self {
        Self {
            id: r.id.clone(),
            label: r
                .display_name
                .as_deref()
                .unwrap_or(&r.native_id)
                .to_string(),
            node_type: NodeType::Resource(r.resource_type),
        }
    }

    pub fn domain(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            label: name.to_string(),
            node_type: NodeType::Domain,
        }
    }
}

pub struct SystemGraph {
    pub graph: Graph<NodeLabel, RelationshipType>,
    index_map: HashMap<String, NodeIndex>,
}

impl SystemGraph {
    /// Build a SystemGraph from the SQLite database.
    pub fn from_db(db: &Database) -> Result<Self> {
        let mut graph = Graph::new();
        let mut index_map = HashMap::new();

        // Add all resources as nodes
        let resources_list = resources::list(db.conn())?;
        for r in &resources_list {
            let idx = graph.add_node(NodeLabel::resource(r));
            index_map.insert(r.id.clone(), idx);
        }

        // Add all domains as nodes
        let domains = crate::storage::domains::list(db.conn())?;
        for d in &domains {
            let idx = graph.add_node(NodeLabel::domain(&d.id, &d.name));
            index_map.insert(d.id.clone(), idx);
        }

        // Add all relationships as edges
        let rels = relationships::list(db.conn())?;
        for rel in &rels {
            if let (Some(&src), Some(&tgt)) =
                (index_map.get(&rel.source_id), index_map.get(&rel.target_id))
            {
                graph.add_edge(src, tgt, rel.relationship_type);
            }
        }

        Ok(Self { graph, index_map })
    }

    /// Get the NodeIndex for a resource or domain by its Chapeau UUID.
    pub fn node_index(&self, id: &str) -> Option<NodeIndex> {
        self.index_map.get(id).copied()
    }

    /// Find a node by display name or native_id (for resources).
    pub fn find_node(&self, name: &str) -> Option<NodeIndex> {
        self.graph.node_indices().find(|&idx| {
            let node = &self.graph[idx];
            node.label == name
        })
    }

    /// Get all outgoing edges from a node (what does this node depend on / own / use).
    pub fn outgoing(&self, idx: NodeIndex) -> Vec<(NodeIndex, &RelationshipType, &NodeLabel)> {
        self.graph
            .edges(idx)
            .map(|edge| {
                let target = &self.graph[edge.target()];
                (edge.target(), edge.weight(), target)
            })
            .collect()
    }

    /// Get all incoming edges to a node (what depends on / uses / owns this node).
    pub fn incoming(&self, idx: NodeIndex) -> Vec<(NodeIndex, &RelationshipType, &NodeLabel)> {
        self.graph
            .edges_directed(idx, petgraph::Direction::Incoming)
            .map(|edge| {
                let source = &self.graph[edge.source()];
                (edge.source(), edge.weight(), source)
            })
            .collect()
    }

    /// Get all nodes that reference this node via any relationship.
    pub fn neighbors(&self, idx: NodeIndex) -> Vec<(NodeIndex, &RelationshipType, &NodeLabel)> {
        let mut result = self.outgoing(idx);
        result.extend(self.incoming(idx));
        result
    }
}
