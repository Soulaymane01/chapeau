use crate::core::ResourceType;
use crate::errors::Result;
use crate::storage::Database;
use crate::storage::{observations, resources};

pub fn run(db: &Database) -> Result<()> {
    let services = resources::list_by_type(db.conn(), ResourceType::Service)?;

    if services.is_empty() {
        println!("No services in the Chapeau model.");
        println!("Run 'chapeau scan' to discover system services.");
        return Ok(());
    }

    println!("Services ({} total):", services.len());

    let mut active = Vec::new();
    let mut failed = Vec::new();
    let mut other = Vec::new();

    for svc in &services {
        let obs = observations::get(db.conn(), &svc.id).ok().flatten();
        let is_active = obs.as_ref().and_then(|o| o.active).unwrap_or(false);
        let is_failed = obs.as_ref().and_then(|o| o.failed).unwrap_or(false);

        if is_failed {
            failed.push(svc);
        } else if is_active {
            active.push(svc);
        } else {
            other.push(svc);
        }
    }

    if !active.is_empty() {
        println!();
        println!("Active ({}):", active.len());
        for svc in &active {
            let label = svc.display_name.as_deref().unwrap_or(&svc.native_id);
            let obs = observations::get(db.conn(), &svc.id).ok().flatten();
            let enabled = obs
                .as_ref()
                .and_then(|o| o.enabled)
                .map(|e| if e { "enabled" } else { "disabled" })
                .unwrap_or("unknown");
            println!("  {:<40} {}", svc.native_id, enabled);
            if label != &svc.native_id {
                println!("    {}", label);
            }
        }
    }

    if !failed.is_empty() {
        println!();
        println!("Failed ({}):", failed.len());
        for svc in &failed {
            println!("  {}", svc.native_id);
            if let Some(ref desc) = svc.display_name {
                if desc != &svc.native_id {
                    println!("    {}", desc);
                }
            }
        }
    }

    if !other.is_empty() {
        println!();
        println!("Inactive ({}):", other.len());
        for svc in &other {
            println!("  {}", svc.native_id);
        }
    }

    Ok(())
}
