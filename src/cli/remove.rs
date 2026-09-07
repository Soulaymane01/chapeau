use crate::core::planner;
use crate::reconciliation::scanner;
use crate::storage::history;
use crate::storage::Database;
use std::io::{self, Write};
use std::process::Command;

pub fn run(db: &Database, native_id: &str) -> anyhow::Result<()> {
    let plan = planner::plan_removal(db.conn(), native_id)?;

    print!("{}", plan.format());

    println!("Continue? [y/N] ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() != "y" {
        println!("Aborted.");
        return Ok(());
    }

    println!();
    println!("Removing {}...", native_id);

    // Execute the removal using dnf5
    let output = Command::new("sudo")
        .args(["dnf5", "remove", "-y", &plan.resource.native_id])
        .output()?;

    if output.status.success() {
        // DNF removal succeeded. Now reconcile Chapeau's database with the actual system state.
        match scanner::reconcile_after_removal(db, &[native_id.to_string()]) {
            Ok(summary) => {
                println!("Removal complete.");
                println!();
                println!("  Removed from Fedora: {}", native_id);
                if summary.stale_packages_removed > 0 {
                    println!(
                        "  Stale package records reconciled: {}",
                        summary.stale_packages_removed
                    );
                }
                if summary.relationships_cleaned > 0 {
                    println!(
                        "  Associated relationships cleaned: {}",
                        summary.relationships_cleaned
                    );
                }
                if summary.observations_cleaned > 0 {
                    println!(
                        "  Associated observations cleaned: {}",
                        summary.observations_cleaned
                    );
                }
                if summary.domain_associations_cleaned > 0 {
                    println!(
                        "  Domain associations cleaned: {}",
                        summary.domain_associations_cleaned
                    );
                }
                if summary.stale_packages_removed == 0 {
                    println!("  No stale Chapeau records found.");
                }
            }
            Err(e) => {
                // DNF removal succeeded but reconciliation failed.
                // The Fedora system has changed — we cannot roll that back.
                // Warn the user and suggest manual reconciliation.
                println!("Removal completed successfully on Fedora.");
                println!();
                println!(
                    "Warning: Chapeau could not reconcile its local state: {}",
                    e
                );
                println!();
                println!("Run:");
                println!();
                println!("    chapeau scan");
                println!();
                println!("to reconcile the database with the current system.");

                // Record the partial success in history.
                let _ = history::insert(
                    db.conn(),
                    "remove",
                    Some("package"),
                    None,
                    None,
                    Some(&format!(
                        "dnf removal of '{}' succeeded but reconciliation failed: {}",
                        native_id, e
                    )),
                );
            }
        }
    } else {
        // DNF removal failed. Do NOT modify the Chapeau model.
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Removal failed.");
        eprintln!();
        eprintln!("{}", stderr);
        eprintln!();
        eprintln!("Chapeau state was not modified.");

        // Record the failure in history.
        let _ = history::insert(
            db.conn(),
            "remove_failed",
            Some("package"),
            None,
            None,
            Some(&format!(
                "dnf removal of '{}' failed: {}",
                native_id,
                stderr.lines().next().unwrap_or("unknown error")
            )),
        );
    }

    Ok(())
}
