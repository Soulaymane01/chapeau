use crate::errors::{ChapeauError, Result};
use serde::{Deserialize, Serialize};

/// A user-defined collection of inventory items.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Group {
    /// Chapeau-internal UUID.
    pub id: String,
    /// Unique human-readable group name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Optional icon name.
    pub icon: Option<String>,
    /// Optional display color.
    pub color: Option<String>,
    /// Ordering hint for the group list.
    pub sort_order: i64,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-update timestamp.
    pub updated_at: String,
}

impl Group {
    pub fn new(name: impl Into<String>, description: Option<String>) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into().trim().to_string(),
            description,
            icon: None,
            color: None,
            sort_order: 0,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(ChapeauError::Validation(
                "group id must not be empty".into(),
            ));
        }
        if self.name.trim().is_empty() {
            return Err(ChapeauError::Validation(
                "group name must not be empty".into(),
            ));
        }
        if self.name.chars().any(char::is_control) {
            return Err(ChapeauError::Validation(
                "group name must not contain control characters".into(),
            ));
        }
        Ok(())
    }
}

impl std::fmt::Display for Group {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

/// A membership row between a group and an item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroupMember {
    pub group_id: String,
    pub item_id: String,
    /// Ordering within the group.
    pub sort_order: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_group_trims_name_and_sets_defaults() {
        let group = Group::new("  Databases  ", Some("DB servers".into()));
        assert!(!group.id.is_empty());
        assert_eq!(group.name, "Databases");
        assert_eq!(group.description.as_deref(), Some("DB servers"));
        assert_eq!(group.sort_order, 0);
        assert!(group.icon.is_none());
        assert!(group.color.is_none());
        assert_eq!(group.created_at, group.updated_at);
    }

    #[test]
    fn validate_rejects_empty_and_control_names() {
        let mut group = Group::new("IDEs", None);
        group.name = "   ".into();
        assert!(group.validate().is_err());

        group.name = "bad\nname".into();
        assert!(group.validate().is_err());
    }

    #[test]
    fn validate_accepts_spaces() {
        let group = Group::new("Web servers", None);
        assert!(group.validate().is_ok());
    }

    #[test]
    fn serialization_roundtrip() {
        let group = Group::new("IDEs", Some("Editors".into()));
        let json = serde_json::to_string(&group).unwrap();
        let back: Group = serde_json::from_str(&json).unwrap();
        assert_eq!(group, back);
    }
}
