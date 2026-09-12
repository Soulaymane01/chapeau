use crate::core::root::RootSource;
use crate::core::Resource;
use crate::errors::{ChapeauError, Result};
use crate::storage::{history, resources, roots, Database};

/// Hide a resource from the default My System view.
///
/// Hidden roots are preserved across scans and never re-detected, while the
/// resource itself remains fully tracked and visible in Explore.
pub fn hide(db: &Database, resource_name: &str) -> Result<Resource> {
    let resource = resources::find_by_native_id(db.conn(), resource_name)?
        .ok_or_else(|| ChapeauError::ResourceNotFound(resource_name.to_string()))?;

    match roots::get(db.conn(), &resource.id)? {
        Some(root) if root.source == RootSource::Ignored => {}
        Some(root) => {
            roots::update(
                db.conn(),
                &resource.id,
                RootSource::Ignored,
                Some("hidden from My System"),
            )?;
            history::insert(
                db.conn(),
                "root.hide",
                Some(&resource.resource_type.to_string()),
                Some(&resource.id),
                None,
                Some(&format!(
                    "hid '{}' from My System (was source={})",
                    resource_name, root.source
                )),
            )?;
        }
        None => {
            roots::create(
                db.conn(),
                &resource.id,
                RootSource::Ignored,
                Some("hidden from My System"),
            )?;
            history::insert(
                db.conn(),
                "root.hide",
                Some(&resource.resource_type.to_string()),
                Some(&resource.id),
                None,
                Some(&format!("hid '{}' from My System", resource_name)),
            )?;
        }
    }

    Ok(resource)
}

/// Restore a hidden resource to the default My System view.
///
/// Returns `None` when the resource was not hidden.
pub fn unhide(db: &Database, resource_name: &str) -> Result<Option<Resource>> {
    let resource = resources::find_by_native_id(db.conn(), resource_name)?
        .ok_or_else(|| ChapeauError::ResourceNotFound(resource_name.to_string()))?;

    let Some(root) = roots::get(db.conn(), &resource.id)? else {
        return Ok(None);
    };
    if root.source != RootSource::Ignored {
        return Ok(None);
    }

    roots::delete(db.conn(), &resource.id)?;
    history::insert(
        db.conn(),
        "root.unhide",
        Some(&resource.resource_type.to_string()),
        Some(&resource.id),
        None,
        Some(&format!("unhid '{}' from My System", resource_name)),
    )?;

    Ok(Some(resource))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ResourceType;

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    #[test]
    fn hide_and_unhide_roundtrip() {
        let db = temp_db();
        let package = resources::create(db.conn(), ResourceType::Package, "grubby", None).unwrap();
        roots::create(db.conn(), &package.id, RootSource::Detected, None).unwrap();

        hide(&db, "grubby").unwrap();
        let root = roots::get(db.conn(), &package.id).unwrap().unwrap();
        assert_eq!(root.source, RootSource::Ignored);

        let restored = unhide(&db, "grubby").unwrap();
        assert!(restored.is_some());
        assert!(roots::get(db.conn(), &package.id).unwrap().is_none());
    }

    #[test]
    fn hide_survives_scan_reconciliation() {
        let db = temp_db();
        let package = resources::create(db.conn(), ResourceType::Package, "dnf5", None).unwrap();
        hide(&db, "dnf5").unwrap();

        // Simulate a scan re-detecting the package.
        let summary = crate::reconciliation::scanner::reconcile_roots(
            db.conn(),
            &[crate::reconciliation::scanner::DesiredRoot {
                resource_id: package.id.clone(),
                reason: "DNF user-installed package".to_string(),
            }],
        )
        .unwrap();

        assert_eq!(summary.added, 0);
        assert_eq!(summary.preserved_explicit, 1);
        let root = roots::get(db.conn(), &package.id).unwrap().unwrap();
        assert_eq!(root.source, RootSource::Ignored);
    }
}
