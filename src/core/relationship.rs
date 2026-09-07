use crate::errors::{ChapeauError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// The semantic type of a relationship between two resources.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RelationshipType {
    /// Source requires target to function.
    #[serde(rename = "depends_on")]
    DependsOn,
    /// Source fulfills or supplies target's interface.
    #[serde(rename = "provides")]
    Provides,
    /// Source has authoritative ownership of target.
    #[serde(rename = "owns")]
    Owns,
    /// Source interacts with or utilizes target.
    #[serde(rename = "uses")]
    Uses,
    /// Source was obtained from or produced by target.
    #[serde(rename = "comes_from")]
    ComesFrom,
}

impl RelationshipType {
    /// Return the canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            RelationshipType::DependsOn => "depends_on",
            RelationshipType::Provides => "provides",
            RelationshipType::Owns => "owns",
            RelationshipType::Uses => "uses",
            RelationshipType::ComesFrom => "comes_from",
        }
    }

    /// Parse from a string, returning an error on unknown variants.
    pub fn parse(s: &str) -> Result<Self> {
        Self::from_str(s).map_err(|_| ChapeauError::InvalidRelationshipType(s.to_string()))
    }

    /// All known relationship types.
    pub fn all() -> &'static [RelationshipType] {
        &[
            RelationshipType::DependsOn,
            RelationshipType::Provides,
            RelationshipType::Owns,
            RelationshipType::Uses,
            RelationshipType::ComesFrom,
        ]
    }
}

impl fmt::Display for RelationshipType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RelationshipType {
    type Err = ChapeauError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "depends_on" => Ok(RelationshipType::DependsOn),
            "provides" => Ok(RelationshipType::Provides),
            "owns" => Ok(RelationshipType::Owns),
            "uses" => Ok(RelationshipType::Uses),
            "comes_from" => Ok(RelationshipType::ComesFrom),
            _ => Err(ChapeauError::InvalidRelationshipType(s.to_string())),
        }
    }
}

/// Where a relationship originated.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RelationshipOrigin {
    /// Discovered from system state (e.g. RPM deps, systemd Wants).
    #[serde(rename = "system")]
    System,
    /// Created explicitly by the user.
    #[serde(rename = "user")]
    User,
    /// Computed or inferred by Chapeau (e.g. reverse relationships).
    #[serde(rename = "derived")]
    Derived,
}

impl RelationshipOrigin {
    /// Return the canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            RelationshipOrigin::System => "system",
            RelationshipOrigin::User => "user",
            RelationshipOrigin::Derived => "derived",
        }
    }

    /// Parse from a string, returning an error on unknown variants.
    pub fn parse(s: &str) -> Result<Self> {
        Self::from_str(s)
            .map_err(|_| ChapeauError::Validation(format!("invalid relationship origin: {}", s)))
    }

    /// All known origins.
    pub fn all() -> &'static [RelationshipOrigin] {
        &[
            RelationshipOrigin::System,
            RelationshipOrigin::User,
            RelationshipOrigin::Derived,
        ]
    }
}

impl fmt::Display for RelationshipOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RelationshipOrigin {
    type Err = ChapeauError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "system" => Ok(RelationshipOrigin::System),
            "user" => Ok(RelationshipOrigin::User),
            "derived" => Ok(RelationshipOrigin::Derived),
            _ => Err(ChapeauError::Validation(format!(
                "invalid relationship origin: {}",
                s
            ))),
        }
    }
}

/// A directed relationship between two resources.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Relationship {
    /// Chapeau-internal UUID.
    pub id: String,
    /// ID of the source resource.
    pub source_id: String,
    /// The type of relationship.
    pub relationship_type: RelationshipType,
    /// ID of the target resource.
    pub target_id: String,
    /// Where this relationship was discovered or created.
    pub origin: RelationshipOrigin,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-update timestamp.
    pub updated_at: String,
}

impl Relationship {
    /// Create a new Relationship with the given fields and timestamps set to now.
    pub fn new(
        source_id: String,
        relationship_type: RelationshipType,
        target_id: String,
        origin: RelationshipOrigin,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source_id,
            relationship_type,
            target_id,
            origin,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Validate the relationship fields.
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(ChapeauError::Validation(
                "relationship id must not be empty".into(),
            ));
        }
        if self.source_id.is_empty() {
            return Err(ChapeauError::Validation(
                "relationship source_id must not be empty".into(),
            ));
        }
        if self.target_id.is_empty() {
            return Err(ChapeauError::Validation(
                "relationship target_id must not be empty".into(),
            ));
        }
        if self.source_id == self.target_id {
            return Err(ChapeauError::Validation(
                "relationship source and target must differ".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- RelationshipType tests ---

    #[test]
    fn relationship_type_as_str_roundtrip() {
        for rt in RelationshipType::all() {
            let s = rt.as_str();
            let back = RelationshipType::from_str(s).unwrap();
            assert_eq!(*rt, back);
        }
    }

    #[test]
    fn relationship_type_display() {
        assert_eq!(RelationshipType::DependsOn.to_string(), "depends_on");
        assert_eq!(RelationshipType::Provides.to_string(), "provides");
        assert_eq!(RelationshipType::Owns.to_string(), "owns");
        assert_eq!(RelationshipType::Uses.to_string(), "uses");
        assert_eq!(RelationshipType::ComesFrom.to_string(), "comes_from");
    }

    #[test]
    fn relationship_type_parse_ok() {
        assert_eq!(
            RelationshipType::parse("depends_on").unwrap(),
            RelationshipType::DependsOn
        );
        assert_eq!(
            RelationshipType::parse("comes_from").unwrap(),
            RelationshipType::ComesFrom
        );
    }

    #[test]
    fn relationship_type_parse_err() {
        let err = RelationshipType::parse("bogus").unwrap_err();
        assert!(err.to_string().contains("bogus"));
    }

    #[test]
    fn relationship_type_serialization() {
        let rt = RelationshipType::Owns;
        let json = serde_json::to_string(&rt).unwrap();
        assert_eq!(json, "\"owns\"");
        let back: RelationshipType = serde_json::from_str(&json).unwrap();
        assert_eq!(rt, back);
    }

    #[test]
    fn relationship_type_all_unique() {
        let all = RelationshipType::all();
        let mut strs: Vec<&str> = all.iter().map(|rt| rt.as_str()).collect();
        strs.sort();
        strs.dedup();
        assert_eq!(strs.len(), all.len());
    }

    // --- RelationshipOrigin tests ---

    #[test]
    fn origin_as_str_roundtrip() {
        for o in RelationshipOrigin::all() {
            let s = o.as_str();
            let back = RelationshipOrigin::from_str(s).unwrap();
            assert_eq!(*o, back);
        }
    }

    #[test]
    fn origin_display() {
        assert_eq!(RelationshipOrigin::System.to_string(), "system");
        assert_eq!(RelationshipOrigin::User.to_string(), "user");
        assert_eq!(RelationshipOrigin::Derived.to_string(), "derived");
    }

    #[test]
    fn origin_parse_err() {
        assert!(RelationshipOrigin::parse("bogus").is_err());
    }

    #[test]
    fn origin_serialization() {
        let o = RelationshipOrigin::User;
        let json = serde_json::to_string(&o).unwrap();
        assert_eq!(json, "\"user\"");
        let back: RelationshipOrigin = serde_json::from_str(&json).unwrap();
        assert_eq!(o, back);
    }

    #[test]
    fn origin_all_unique() {
        let all = RelationshipOrigin::all();
        let mut strs: Vec<&str> = all.iter().map(|o| o.as_str()).collect();
        strs.sort();
        strs.dedup();
        assert_eq!(strs.len(), all.len());
    }

    // --- Relationship validation tests ---

    #[test]
    fn relationship_new_sets_timestamps() {
        let r = Relationship::new(
            "src".into(),
            RelationshipType::DependsOn,
            "tgt".into(),
            RelationshipOrigin::System,
        );
        assert!(!r.id.is_empty());
        assert_eq!(r.source_id, "src");
        assert_eq!(r.target_id, "tgt");
        assert_eq!(r.relationship_type, RelationshipType::DependsOn);
        assert_eq!(r.origin, RelationshipOrigin::System);
        assert!(!r.created_at.is_empty());
        assert_eq!(r.created_at, r.updated_at);
    }

    #[test]
    fn relationship_validate_ok() {
        let r = Relationship::new(
            "a".into(),
            RelationshipType::Uses,
            "b".into(),
            RelationshipOrigin::User,
        );
        assert!(r.validate().is_ok());
    }

    #[test]
    fn relationship_validate_empty_id() {
        let mut r = Relationship::new(
            "a".into(),
            RelationshipType::Uses,
            "b".into(),
            RelationshipOrigin::User,
        );
        r.id = String::new();
        assert!(r.validate().is_err());
    }

    #[test]
    fn relationship_validate_empty_source() {
        let mut r = Relationship::new(
            "a".into(),
            RelationshipType::Uses,
            "b".into(),
            RelationshipOrigin::User,
        );
        r.source_id = String::new();
        assert!(r.validate().is_err());
        assert!(r.validate().unwrap_err().to_string().contains("source_id"));
    }

    #[test]
    fn relationship_validate_empty_target() {
        let mut r = Relationship::new(
            "a".into(),
            RelationshipType::Uses,
            "b".into(),
            RelationshipOrigin::User,
        );
        r.target_id = String::new();
        assert!(r.validate().is_err());
        assert!(r.validate().unwrap_err().to_string().contains("target_id"));
    }

    #[test]
    fn relationship_validate_self_referential() {
        let r = Relationship::new(
            "a".into(),
            RelationshipType::DependsOn,
            "a".into(),
            RelationshipOrigin::Derived,
        );
        assert!(r.validate().is_err());
        assert!(r.validate().unwrap_err().to_string().contains("differ"));
    }

    #[test]
    fn relationship_serialization() {
        let r = Relationship::new(
            "src-id".into(),
            RelationshipType::Provides,
            "tgt-id".into(),
            RelationshipOrigin::System,
        );
        let json = serde_json::to_string(&r).unwrap();
        let back: Relationship = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }
}
