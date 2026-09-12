use crate::services::analysis::{self, AnalysisKind};
use crate::storage::Database;

pub fn run(db: &Database) -> anyhow::Result<()> {
    let unused = analysis::run(db, AnalysisKind::Unused)?;

    if unused.is_empty() {
        println!("No unused resources found.");
        println!();
        println!("All resources are referenced by other resources, domains, have active relationships, or are intentional roots.");
        return Ok(());
    }

    println!("Unused Resources");
    println!("================");
    println!();
    println!("Note: Intentional resources (roots) are excluded from this analysis.");
    println!();

    for item in &unused {
        let r = &item.resource;
        let label = r.display_name.as_deref().unwrap_or(&r.native_id);

        println!("{}", label);
        println!("  Type: {}", r.resource_type);

        println!();
        println!("  Potentially unused");
        println!();
        println!("  Reason:");

        for reason in &item.reasons {
            println!("    {}", reason.description());
        }

        if !item.affected_services.is_empty() {
            println!();
            println!("  Service:");
            for svc in &item.affected_services {
                println!("    {}", svc);
            }
        }

        println!();
        println!("  Action:");
        println!("    Not removed automatically");
        println!();
    }

    println!("Total: {} potentially unused", unused.len());
    println!();
    println!("Use 'chapeau root add <resource>' to protect a resource from removal analysis.");

    Ok(())
}
