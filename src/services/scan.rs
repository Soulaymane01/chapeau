//! Scan orchestration shared by the CLI and the GUI.

use crate::errors::Result;
use crate::reconciliation::scanner;
pub use crate::reconciliation::scanner::{ScanEvent, ScanOutcome};
use crate::storage::Database;

/// Run a full scan, reporting progress via `progress`.
///
/// This is a blocking operation; frontends should call it from a worker
/// thread when they need to stay responsive.
pub fn run(db: &Database, progress: &mut dyn FnMut(ScanEvent)) -> Result<ScanOutcome> {
    scanner::scan_with_progress(db, progress)
}
