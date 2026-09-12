use chrono::Utc;
use serde::{Deserialize, Serialize};

/// How a root was established.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RootSource {
    /// User explicitly declared this as a root via `chapeau root add`.
    User,
    /// Chapeau detected this as a candidate root during scan
    /// (DNF user-installed package, Flatpak application).
    Detected,
    /// User adopted an existing resource as a root.
    Adopted,
}

impl std::fmt::Display for RootSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RootSource::User => write!(f, "user"),
            RootSource::Detected => write!(f, "detected"),
            RootSource::Adopted => write!(f, "adopted"),
        }
    }
}

impl std::str::FromStr for RootSource {
    type Err = crate::errors::ChapeauError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "user" => Ok(RootSource::User),
            "detected" => Ok(RootSource::Detected),
            "adopted" => Ok(RootSource::Adopted),
            _ => Err(crate::errors::ChapeauError::Validation(format!(
                "invalid root source: {}",
                s
            ))),
        }
    }
}

/// A root resource — something Chapeau considers intentionally present.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Root {
    /// The resource this root refers to.
    pub resource_id: String,
    /// How this root was established.
    pub source: RootSource,
    /// Optional explanation for why this is a root.
    pub reason: Option<String>,
    /// When this root was created.
    pub created_at: String,
    /// When this root was last updated.
    pub updated_at: String,
}

impl Root {
    /// Create a new Root with current timestamps.
    pub fn new(resource_id: String, source: RootSource, reason: Option<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            resource_id,
            source,
            reason,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_source_display() {
        assert_eq!(RootSource::User.to_string(), "user");
        assert_eq!(RootSource::Detected.to_string(), "detected");
        assert_eq!(RootSource::Adopted.to_string(), "adopted");
    }

    #[test]
    fn test_root_source_parse() {
        assert_eq!("user".parse::<RootSource>().unwrap(), RootSource::User);
        assert_eq!(
            "detected".parse::<RootSource>().unwrap(),
            RootSource::Detected
        );
        assert_eq!(
            "adopted".parse::<RootSource>().unwrap(),
            RootSource::Adopted
        );
        assert!("invalid".parse::<RootSource>().is_err());
    }

    #[test]
    fn test_root_new() {
        let root = Root::new(
            "res-123".to_string(),
            RootSource::User,
            Some("explicitly installed".to_string()),
        );
        assert_eq!(root.resource_id, "res-123");
        assert_eq!(root.source, RootSource::User);
        assert_eq!(root.reason, Some("explicitly installed".to_string()));
        assert!(!root.created_at.is_empty());
        assert_eq!(root.created_at, root.updated_at);
    }
}
