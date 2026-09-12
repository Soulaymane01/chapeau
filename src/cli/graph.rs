use crate::errors::Result;
use crate::services::graph::{self, GraphOptions};
use crate::storage::Database;

pub fn run(db: &Database, resource: &str, depth: usize) -> Result<()> {
    let view = graph::neighborhood(
        db,
        resource,
        GraphOptions {
            depth,
            include_dependents: false,
        },
    )?;

    // Keep stdout pure DOT so it can be piped, e.g.:
    //   chapeau graph bash --depth 2 | dot -Tsvg > bash.svg
    print!("{}", view.dot);
    eprintln!(
        "graph: {} nodes, {} edges{}",
        view.nodes,
        view.edges,
        if view.truncated { " (truncated)" } else { "" }
    );
    Ok(())
}
