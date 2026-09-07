use crate::errors::{ChapeauError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// The type of a resource, corresponding to the system component it represents.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ResourceType {
    /// An RPM package managed by DNF.
    #[serde(rename = "package")]
    Package,
    /// A Flatpak application.
    #[serde(rename = "flatpak")]
    Flatpak,
    /// A systemd service unit.
    #[serde(rename = "service")]
    Service,
    /// A DNF or RPM repository.
    #[serde(rename = "repository")]
    Repository,
}

impl ResourceType {
    /// Return the canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            ResourceType::Package => "package",
            ResourceType::Flatpak => "flatpak",
            ResourceType::Service => "service",
            ResourceType::Repository => "repository",
        }
    }

    /// Parse from a string, returning an error on unknown variants.
    pub fn parse(s: &str) -> Result<Self> {
        Self::from_str(s).map_err(|_| ChapeauError::InvalidResourceType(s.to_string()))
    }

    /// All known resource types.
    pub fn all() -> &'static [ResourceType] {
        &[
            ResourceType::Package,
            ResourceType::Flatpak,
            ResourceType::Service,
            ResourceType::Repository,
        ]
    }
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ResourceType {
    type Err = ChapeauError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "package" => Ok(ResourceType::Package),
            "flatpak" => Ok(ResourceType::Flatpak),
            "service" => Ok(ResourceType::Service),
            "repository" => Ok(ResourceType::Repository),
            _ => Err(ChapeauError::InvalidResourceType(s.to_string())),
        }
    }
}

/// A resource in the system — any trackable component.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Resource {
    /// Chapeau-internal UUID.
    pub id: String,
    /// The type of resource.
    pub resource_type: ResourceType,
    /// The native identifier in the source system (e.g. RPM name, systemd unit).
    pub native_id: String,
    /// Optional human-readable name.
    pub display_name: Option<String>,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-update timestamp.
    pub updated_at: String,
}

impl Resource {
    /// Create a new Resource with the given fields and timestamps set to now.
    pub fn new(
        resource_type: ResourceType,
        native_id: String,
        display_name: Option<String>,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            resource_type,
            native_id,
            display_name,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Validate the resource fields.
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(ChapeauError::Validation(
                "resource id must not be empty".into(),
            ));
        }
        if self.native_id.is_empty() {
            return Err(ChapeauError::Validation(
                "resource native_id must not be empty".into(),
            ));
        }
        Ok(())
    }

    /// Return the display label: display_name if set, else native_id.
    pub fn label(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.native_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_type_as_str_roundtrip() {
        for rt in ResourceType::all() {
            let s = rt.as_str();
            let back = ResourceType::from_str(s).unwrap();
            assert_eq!(*rt, back);
        }
    }

    #[test]
    fn resource_type_display() {
        assert_eq!(ResourceType::Package.to_string(), "package");
        assert_eq!(ResourceType::Flatpak.to_string(), "flatpak");
        assert_eq!(ResourceType::Service.to_string(), "service");
        assert_eq!(ResourceType::Repository.to_string(), "repository");
    }

    #[test]
    fn resource_type_parse_ok() {
        assert_eq!(
            ResourceType::parse("package").unwrap(),
            ResourceType::Package
        );
        assert_eq!(
            ResourceType::parse("flatpak").unwrap(),
            ResourceType::Flatpak
        );
        assert_eq!(
            ResourceType::parse("service").unwrap(),
            ResourceType::Service
        );
        assert_eq!(
            ResourceType::parse("repository").unwrap(),
            ResourceType::Repository
        );
    }

    #[test]
    fn resource_type_parse_err() {
        let err = ResourceType::parse("bogus").unwrap_err();
        assert!(err.to_string().contains("bogus"));
    }

    #[test]
    fn resource_type_from_str_err() {
        assert!(ResourceType::from_str("unknown").is_err());
    }

    #[test]
    fn resource_type_serialization() {
        let rt = ResourceType::Package;
        let json = serde_json::to_string(&rt).unwrap();
        assert_eq!(json, "\"package\"");
        let back: ResourceType = serde_json::from_str(&json).unwrap();
        assert_eq!(rt, back);
    }

    #[test]
    fn resource_type_all_unique() {
        let all = ResourceType::all();
        let mut strs: Vec<&str> = all.iter().map(|rt| rt.as_str()).collect();
        strs.sort();
        strs.dedup();
        assert_eq!(strs.len(), all.len());
    }

    #[test]
    fn resource_new_sets_timestamps() {
        let r = Resource::new(ResourceType::Service, "sshd.service".into(), None);
        assert!(!r.id.is_empty());
        assert_eq!(r.resource_type, ResourceType::Service);
        assert_eq!(r.native_id, "sshd.service");
        assert!(r.display_name.is_none());
        assert!(!r.created_at.is_empty());
        assert_eq!(r.created_at, r.updated_at);
    }

    #[test]
    fn resource_validate_ok() {
        let r = Resource::new(ResourceType::Package, "bash".into(), Some("Bash".into()));
        assert!(r.validate().is_ok());
    }

    #[test]
    fn resource_validate_empty_id() {
        let mut r = Resource::new(ResourceType::Package, "bash".into(), None);
        r.id = String::new();
        assert!(r.validate().is_err());
        assert!(r.validate().unwrap_err().to_string().contains("id"));
    }

    #[test]
    fn resource_validate_empty_native_id() {
        let mut r = Resource::new(ResourceType::Package, "bash".into(), None);
        r.native_id = String::new();
        assert!(r.validate().is_err());
        assert!(r.validate().unwrap_err().to_string().contains("native_id"));
    }

    #[test]
    fn resource_label_fallback() {
        let r = Resource::new(ResourceType::Package, "vim".into(), None);
        assert_eq!(r.label(), "vim");
    }

    #[test]
    fn resource_label_display_name() {
        let r = Resource::new(
            ResourceType::Package,
            "vim".into(),
            Some("Vim Editor".into()),
        );
        assert_eq!(r.label(), "Vim Editor");
    }

    #[test]
    fn resource_serialization() {
        let r = Resource::new(
            ResourceType::Flatpak,
            "org.mozilla.firefox".into(),
            Some("Firefox".into()),
        );
        let json = serde_json::to_string(&r).unwrap();
        let back: Resource = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn resource_equality() {
        let r1 = Resource::new(ResourceType::Package, "bash".into(), None);
        let mut r2 = r1.clone();
        r2.display_name = Some("Bash".into());
        // Same id means equal (display_name differs but id is the identity).
        assert_eq!(r1.id, r2.id);
    }
}
