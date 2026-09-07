use crate::errors::Result;
use serde::{Deserialize, Serialize};

/// How a resource was installed or introduced to the system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Provenance {
    /// ID of the resource this provenance belongs to.
    pub resource_id: String,
    /// How it was installed (e.g. "dnf", "flatpak", "manual").
    pub installation_method: Option<String>,
    /// ISO-8601 timestamp of installation.
    pub installation_time: Option<String>,
    /// DNF transaction ID, if applicable.
    pub transaction_id: Option<String>,
    /// Source kind (e.g. "repository", "url").
    pub source_type: Option<String>,
    /// Source identifier (e.g. repo name, URL).
    pub source_id: Option<String>,
    /// When this resource was "adopted" into a domain.
    pub adopted_at: Option<String>,
}

impl Provenance {
    /// Create a new Provenance for the given resource.
    pub fn new(resource_id: String) -> Self {
        Self {
            resource_id,
            installation_method: None,
            installation_time: None,
            transaction_id: None,
            source_type: None,
            source_id: None,
            adopted_at: None,
        }
    }

    /// Validate the provenance fields.
    pub fn validate(&self) -> Result<()> {
        if self.resource_id.is_empty() {
            return Err(crate::errors::ChapeauError::Validation(
                "provenance resource_id must not be empty".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_new() {
        let p = Provenance::new("res-1".into());
        assert_eq!(p.resource_id, "res-1");
        assert!(p.installation_method.is_none());
        assert!(p.transaction_id.is_none());
    }

    #[test]
    fn provenance_validate_ok() {
        let p = Provenance::new("res-1".into());
        assert!(p.validate().is_ok());
    }

    #[test]
    fn provenance_validate_empty_resource_id() {
        let mut p = Provenance::new(String::new());
        p.resource_id = String::new();
        assert!(p.validate().is_err());
        assert!(p
            .validate()
            .unwrap_err()
            .to_string()
            .contains("resource_id"));
    }

    #[test]
    fn provenance_serialization() {
        let mut p = Provenance::new("res-1".into());
        p.installation_method = Some("dnf".into());
        p.transaction_id = Some("txn-42".into());
        let json = serde_json::to_string(&p).unwrap();
        let back: Provenance = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }
}
