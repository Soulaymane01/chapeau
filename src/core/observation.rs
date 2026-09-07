use crate::errors::Result;
use serde::{Deserialize, Serialize};

/// A point-in-time observation of a resource's live state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Observation {
    /// ID of the resource being observed.
    pub resource_id: String,
    /// ISO-8601 timestamp of the observation.
    pub observed_at: String,
    /// Whether the resource is currently installed.
    pub installed: Option<bool>,
    /// Version string, if known.
    pub version: Option<String>,
    /// Whether the resource is currently active (e.g. running service).
    pub active: Option<bool>,
    /// Whether the resource is enabled (e.g. enabled systemd unit).
    pub enabled: Option<bool>,
    /// Whether the resource is in a failed state.
    pub failed: Option<bool>,
    /// Freeform metadata (JSON object).
    pub metadata: Option<serde_json::Value>,
}

impl Observation {
    /// Create a new Observation with the current timestamp.
    pub fn new(resource_id: String) -> Self {
        Self {
            resource_id,
            observed_at: chrono::Utc::now().to_rfc3339(),
            installed: None,
            version: None,
            active: None,
            enabled: None,
            failed: None,
            metadata: None,
        }
    }

    /// Validate the observation fields.
    pub fn validate(&self) -> Result<()> {
        if self.resource_id.is_empty() {
            return Err(crate::errors::ChapeauError::Validation(
                "observation resource_id must not be empty".into(),
            ));
        }
        if self.observed_at.is_empty() {
            return Err(crate::errors::ChapeauError::Validation(
                "observation observed_at must not be empty".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_new() {
        let o = Observation::new("res-1".into());
        assert_eq!(o.resource_id, "res-1");
        assert!(!o.observed_at.is_empty());
        assert!(o.installed.is_none());
        assert!(o.version.is_none());
    }

    #[test]
    fn observation_validate_ok() {
        let o = Observation::new("res-1".into());
        assert!(o.validate().is_ok());
    }

    #[test]
    fn observation_validate_empty_resource_id() {
        let mut o = Observation::new(String::new());
        o.resource_id = String::new();
        assert!(o.validate().is_err());
        assert!(o
            .validate()
            .unwrap_err()
            .to_string()
            .contains("resource_id"));
    }

    #[test]
    fn observation_validate_empty_observed_at() {
        let mut o = Observation::new("res-1".into());
        o.observed_at = String::new();
        assert!(o.validate().is_err());
        assert!(o
            .validate()
            .unwrap_err()
            .to_string()
            .contains("observed_at"));
    }

    #[test]
    fn observation_serialization() {
        let mut o = Observation::new("res-1".into());
        o.installed = Some(true);
        o.version = Some("1.2.3".into());
        o.active = Some(true);
        o.metadata = Some(serde_json::json!({"pid": 1234}));
        let json = serde_json::to_string(&o).unwrap();
        let back: Observation = serde_json::from_str(&json).unwrap();
        assert_eq!(o, back);
    }
}
