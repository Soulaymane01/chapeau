use crate::core::analysis;
use crate::storage::Database;

pub fn run(db: &Database) -> anyhow::Result<()> {
    let orphaned = analysis::find_orphaned(db.conn())?;

    if orphaned.is_empty() {
        println!("No orphaned resources found.");
        println!();
        println!("All resources have dependents, domain ownership, or domain usage.");
        return Ok(());
    }

    println!("Orphaned Resources (potentially removable)");
    println!("==========================================");
    println!();

    for item in &orphaned {
        let r = &item.resource;
        let label = r.display_name.as_deref().unwrap_or(&r.native_id);

        println!("{}", label);
        println!("  Type: {}", r.resource_type);

        if item.reasons.is_empty() {
            println!();
            println!("  NOT removable");
            println!();
            continue;
        }

        println!();
        println!("  Potentially removable");
        println!();
        println!("  Reason:");

        for reason in &item.reasons {
            println!("    {}", reason.description());
        }

        if !item.dependents.is_empty() {
            println!();
            println!("  Still required by:");
            for dep in &item.dependents {
                println!("    {}", dep);
            }
        }

        if !item.owned_by_domains.is_empty() {
            println!();
            println!("  Owned by domains:");
            for domain in &item.owned_by_domains {
                println!("    {}", domain);
            }
        }

        if !item.used_by_domains.is_empty() {
            println!();
            println!("  Used by domains:");
            for domain in &item.used_by_domains {
                println!("    {}", domain);
            }
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

    println!("Total: {} potentially removable", orphaned.len());

    Ok(())
}
