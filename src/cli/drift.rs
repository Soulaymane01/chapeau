use crate::errors::Result;
use crate::reconciliation::drift::{self, DriftEntry, DriftReport};
use crate::reconciliation::scanner;
use crate::storage::{resources, Database};

/// Maximum entries printed per section by the `drift` command.
const DETAIL_LIMIT: usize = 20;
/// Maximum entries printed per section when a scan reports drift inline.
const SCAN_LIMIT: usize = 5;

pub fn run(db: &Database) -> Result<()> {
    if resources::count(db.conn())? == 0 {
        println!("Chapeau has no recorded system state yet.");
        println!("Run 'chapeau scan' first to build a baseline.");
        return Ok(());
    }

    println!("Discovering current system state (read-only)...");
    let snapshot = scanner::discover()?;
    let report = drift::compute(db.conn(), &snapshot)?;

    println!();
    if report.is_empty() {
        println!("No drift detected.");
        println!("Chapeau's recorded state matches the live system.");
        return Ok(());
    }

    println!("System Drift ({})", report.summary());
    print_sections(&report, DETAIL_LIMIT);
    println!();
    println!("Run 'chapeau scan' to reconcile Chapeau's knowledge with the current system.");

    Ok(())
}

/// Print a compact drift summary (used by `chapeau scan`).
pub fn print_scan_summary(report: &DriftReport) {
    if report.is_empty() {
        return;
    }
    println!();
    println!("Drift detected since the last scan ({}):", report.summary());
    print_sections(report, SCAN_LIMIT);
}

fn print_sections(report: &DriftReport, limit: usize) {
    print_section("Missing", &report.missing, limit);
    print_section("New", &report.added, limit);
    print_section("Changed", &report.changed, limit);
}

fn print_section(title: &str, entries: &[DriftEntry], limit: usize) {
    if entries.is_empty() {
        return;
    }

    println!();
    println!("{} ({}):", title, entries.len());
    for entry in entries.iter().take(limit) {
        println!("{}", format_entry(entry));
    }
    if entries.len() > limit {
        println!("  ... and {} more", entries.len() - limit);
    }
}

fn format_entry(entry: &DriftEntry) -> String {
    let detail = match (entry.recorded.as_deref(), entry.actual.as_deref()) {
        (Some(recorded), Some(actual)) => format!("{} -> {}", recorded, actual),
        (Some(recorded), None) => format!("last seen {}", recorded),
        (None, Some(actual)) => actual.to_string(),
        (None, None) => String::new(),
    };
    format!("  {:<30} {:<9} {}", entry.name, entry.resource_type, detail)
}
