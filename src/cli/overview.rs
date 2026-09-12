use crate::errors::Result;
use crate::services::overview::{self, Overview, UNGROUPED_LIMIT};
use crate::storage::Database;

/// Show the "My System" overview: intentional resources, grouped by domain
/// when domains exist, otherwise grouped by resource type.
pub fn run(db: &Database) -> Result<()> {
    let view = overview::build(db)?;
    print_view(&view);
    Ok(())
}

fn print_view(view: &Overview) {
    println!("My System");
    println!("=========");
    println!();

    if view.root_count == 0 {
        println!("No intentional resources yet.");
        println!("Run 'chapeau scan' to classify installed software into intentional resources.");
        println!("Or add one with 'chapeau root add <resource>'.");
        return;
    }

    for section in &view.sections {
        println!("{} ({})", section.title, section.entries.len());
        let width = section
            .entries
            .iter()
            .map(|entry| entry.label().chars().count())
            .max()
            .unwrap_or(0);
        let shown = if section.collapsed {
            &section.entries[..section.entries.len().min(UNGROUPED_LIMIT)]
        } else {
            &section.entries[..]
        };
        for entry in shown {
            println!("  {:<width$} {}", entry.label(), entry.tag(), width = width);
        }
        if section.collapsed && section.entries.len() > UNGROUPED_LIMIT {
            println!(
                "  ... and {} more (see 'chapeau roots')",
                section.entries.len() - UNGROUPED_LIMIT
            );
        }
        println!();
    }

    let mut summary = format!("{} intentional resources", view.root_count);
    if view.missing_count > 0 {
        summary.push_str(&format!(" ({} missing)", view.missing_count));
    }
    println!(
        "{} · {} tracked resources · {} relationships",
        summary, view.resource_count, view.relationship_count
    );
    println!();

    println!("Inspect:   chapeau show <resource> · chapeau why <resource> · chapeau drift");
    println!("Explore:   chapeau packages --all · chapeau services --all · chapeau flatpaks --all");
    if view.has_domains {
        println!("Organize:  chapeau domains · chapeau domains add <domain> <resource>");
    } else {
        println!(
            "Organize:  chapeau domains create <name> · chapeau domains add <domain> <resource>"
        );
    }
}
