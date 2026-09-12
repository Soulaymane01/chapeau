//! Read-only drift detection shared by the CLI and the GUI.

use crate::errors::Result;
pub use crate::reconciliation::drift::{DriftEntry, DriftReport};
use crate::reconciliation::{drift, scanner};
use crate::storage::Database;

/// Discover the live system and compare it against the recorded model.
///
/// Read-only: nothing is written to the database.
pub fn detect(db: &Database) -> Result<DriftReport> {
    let snapshot = scanner::discover()?;
    drift::compute(db.conn(), &snapshot)
}
