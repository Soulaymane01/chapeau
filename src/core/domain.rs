use crate::errors::Result;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A logical grouping of resources (e.g. "desktop", "development", "media").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Domain {
    /// Chapeau-internal UUID.
    pub id: String,
    /// Human-readable domain name (unique).
    pub name: String,
    /// Optional description of what this domain covers.
    pub description: Option<String>,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-update timestamp.
    pub updated_at: String,
}

impl Domain {
    /// Create a new Domain with the given fields and timestamps set to now.
    pub fn new(name: String, description: Option<String>) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Validate the domain fields.
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(crate::errors::ChapeauError::Validation(
                "domain id must not be empty".into(),
            ));
        }
        if self.name.is_empty() {
            return Err(crate::errors::ChapeauError::Validation(
                "domain name must not be empty".into(),
            ));
        }
        if self.name.contains(char::is_whitespace) {
            return Err(crate::errors::ChapeauError::Validation(
                "domain name must not contain whitespace".into(),
            ));
        }
        Ok(())
    }
}

impl fmt::Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_new_sets_timestamps() {
        let d = Domain::new("desktop".into(), Some("Desktop environment".into()));
        assert!(!d.id.is_empty());
        assert_eq!(d.name, "desktop");
        assert_eq!(d.description.as_deref(), Some("Desktop environment"));
        assert!(!d.created_at.is_empty());
        assert_eq!(d.created_at, d.updated_at);
    }

    #[test]
    fn domain_validate_ok() {
        let d = Domain::new("development".into(), None);
        assert!(d.validate().is_ok());
    }

    #[test]
    fn domain_validate_empty_id() {
        let mut d = Domain::new("test".into(), None);
        d.id = String::new();
        assert!(d.validate().is_err());
    }

    #[test]
    fn domain_validate_empty_name() {
        let mut d = Domain::new("test".into(), None);
        d.name = String::new();
        assert!(d.validate().is_err());
    }

    #[test]
    fn domain_validate_whitespace_name() {
        let d = Domain::new("my domain".into(), None);
        assert!(d.validate().is_err());
        assert!(d.validate().unwrap_err().to_string().contains("whitespace"));
    }

    #[test]
    fn domain_display() {
        let d = Domain::new("media".into(), None);
        assert_eq!(d.to_string(), "media");
    }

    #[test]
    fn domain_serialization() {
        let d = Domain::new("dev".into(), Some("Dev tools".into()));
        let json = serde_json::to_string(&d).unwrap();
        let back: Domain = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }
}
