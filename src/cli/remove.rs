use crate::errors::Result;
use crate::services::removal::{self, Privilege};
use crate::storage::Database;
use std::io::{self, Write};

pub fn run(db: &Database, native_id: &str) -> Result<()> {
    let plan = removal::plan(db, native_id)?;

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

    let outcome = removal::execute(db, native_id, Privilege::Sudo)?;

    if outcome.command_succeeded {
        match outcome.reconcile {
            Some(Ok(summary)) => {
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
            Some(Err(err)) => {
                // DNF removal succeeded but reconciliation failed.
                // The Fedora system has changed — we cannot roll that back.
                // Warn the user and suggest manual reconciliation.
                println!("Removal completed successfully on Fedora.");
                println!();
                println!(
                    "Warning: Chapeau could not reconcile its local state: {}",
                    err
                );
                println!();
                println!("Run:");
                println!();
                println!("    chapeau scan");
                println!();
                println!("to reconcile the database with the current system.");
            }
            None => {}
        }
    } else {
        // DNF removal failed. Do NOT modify the Chapeau model.
        eprintln!("Removal failed.");
        eprintln!();
        eprintln!("{}", outcome.stderr);
        eprintln!();
        eprintln!("Chapeau state was not modified.");
    }

    Ok(())
}
