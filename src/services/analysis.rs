use crate::core::analysis as core_analysis;
pub use crate::core::analysis::{AnalysisReason, ResourceAnalysis};
use crate::errors::Result;
use crate::storage::Database;

/// Which cleanup analysis to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisKind {
    Orphaned,
    Unused,
}

impl AnalysisKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::Orphaned => "Potentially orphaned",
            Self::Unused => "Potentially unused",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Orphaned => "Resources with no dependents and no domain ownership or usage.",
            Self::Unused => {
                "Resources with no dependents, no domain involvement and no dependencies."
            }
        }
    }
}

/// Run a cleanup analysis. These are heuristics, not proof that a resource
/// is safe to delete.
pub fn run(db: &Database, kind: AnalysisKind) -> Result<Vec<ResourceAnalysis>> {
    match kind {
        AnalysisKind::Orphaned => core_analysis::find_orphaned(db.conn()),
        AnalysisKind::Unused => core_analysis::find_unused(db.conn()),
    }
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
    fn orphaned_includes_unreferenced_resources() {
        let db = temp_db();
        resources::create(db.conn(), ResourceType::Package, "lonely", None).unwrap();

        let orphaned = run(&db, AnalysisKind::Orphaned).unwrap();
        assert!(orphaned
            .iter()
            .any(|entry| entry.resource.native_id == "lonely"));
    }

    #[test]
    fn unused_is_a_subset_of_orphaned_checks() {
        let db = temp_db();
        resources::create(db.conn(), ResourceType::Package, "empty", None).unwrap();

        let unused = run(&db, AnalysisKind::Unused).unwrap();
        assert!(unused
            .iter()
            .any(|entry| entry.resource.native_id == "empty"));
        assert!(unused[0]
            .reasons
            .iter()
            .any(|reason| reason.description().contains("Does not depend")));
    }
}
