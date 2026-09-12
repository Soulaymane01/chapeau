use crate::backends::dnf::DnfCliBackend;
use crate::backends::package_backend::PackageBackend;
use crate::core::planner::{self, RemovalPlan};
use crate::errors::Result;
use crate::reconciliation::scanner::{self, ReconcileSummary};
use crate::storage::{history, Database};
use std::process::Command;

/// How a removal obtains privilege.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Privilege {
    /// Terminal `sudo` (CLI).
    Sudo,
    /// Graphical polkit prompt (GUI).
    Pkexec,
}

impl Privilege {
    pub(crate) fn program(self) -> &'static str {
        match self {
            Self::Sudo => "sudo",
            Self::Pkexec => "pkexec",
        }
    }
}

/// Result of executing a removal.
#[derive(Debug)]
pub struct RemovalOutcome {
    /// Whether the package-manager command succeeded.
    pub command_succeeded: bool,
    /// Captured stderr when the command failed.
    pub stderr: String,
    /// Reconciliation result when the command succeeded.
    pub reconcile: Option<Result<ReconcileSummary>>,
}

/// Build the impact plan for removing a resource. Read-only.
pub fn plan(db: &Database, resource_name: &str) -> Result<RemovalPlan> {
    planner::plan_removal(db.conn(), resource_name)
}

/// Remove a package through the package backend, then reconcile Chapeau's
/// state with Fedora.
///
/// The OS command failing never modifies Chapeau's model; a successful OS
/// command that fails to reconcile is reported to the caller, which should
/// tell the user to run a scan.
pub fn execute(db: &Database, resource_name: &str, privilege: Privilege) -> Result<RemovalOutcome> {
    let backend = DnfCliBackend::new();
    let argv = backend.removal_argv(resource_name);
    let output = Command::new(privilege.program()).args(&argv).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        history::insert(
            db.conn(),
            "remove_failed",
            Some("package"),
            None,
            None,
            Some(&format!(
                "removal of '{}' failed: {}",
                resource_name,
                stderr.lines().next().unwrap_or("unknown error")
            )),
        )?;
        return Ok(RemovalOutcome {
            command_succeeded: false,
            stderr,
            reconcile: None,
        });
    }

    // The OS change succeeded. Bring Chapeau's model back in sync.
    let reconcile = scanner::reconcile_after_removal(db, &[resource_name.to_string()]);
    if let Err(err) = &reconcile {
        let _ = history::insert(
            db.conn(),
            "remove",
            Some("package"),
            None,
            None,
            Some(&format!(
                "removal of '{}' succeeded but reconciliation failed: {}",
                resource_name, err
            )),
        );
    }

    Ok(RemovalOutcome {
        command_succeeded: true,
        stderr: String::new(),
        reconcile: Some(reconcile),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ResourceType;
    use crate::storage::resources;

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    #[test]
    fn plan_reports_missing_resources() {
        let db = temp_db();
        assert!(plan(&db, "does-not-exist").is_err());
    }

    #[test]
    fn removal_argv_comes_from_the_package_backend() {
        let backend = DnfCliBackend::new();
        assert_eq!(
            backend.removal_argv("redis"),
            vec!["dnf5", "remove", "-y", "redis"]
        );
    }

    #[test]
    fn plan_finds_a_real_resource() {
        let db = temp_db();
        resources::create(db.conn(), ResourceType::Package, "redis", Some("Redis")).unwrap();
        let plan = plan(&db, "redis").unwrap();
        assert_eq!(plan.resource.native_id, "redis");
    }
}
