use crate::errors::{ChapeauError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Whether an item is a launchable application or a controllable service.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    App,
    Service,
}

impl ItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Service => "service",
        }
    }

    pub fn all() -> &'static [ItemKind] {
        &[ItemKind::App, ItemKind::Service]
    }

    pub fn parse(value: &str) -> Result<Self> {
        value
            .parse()
            .map_err(|_| ChapeauError::Validation(format!("invalid item kind: {value}")))
    }
}

impl fmt::Display for ItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ItemKind {
    type Err = ChapeauError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "app" => Ok(Self::App),
            "service" => Ok(Self::Service),
            _ => Err(ChapeauError::Validation(format!(
                "invalid item kind: {value}"
            ))),
        }
    }
}

/// Where an item was discovered or created.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ItemSource {
    /// A freedesktop `.desktop` entry.
    Desktop,
    /// A Flatpak application.
    Flatpak,
    /// A systemd unit.
    Systemd,
    /// A user-created application entry.
    Manual,
    /// A user-created command service.
    Custom,
}

impl ItemSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Flatpak => "flatpak",
            Self::Systemd => "systemd",
            Self::Manual => "manual",
            Self::Custom => "custom",
        }
    }

    pub fn all() -> &'static [ItemSource] {
        &[
            ItemSource::Desktop,
            ItemSource::Flatpak,
            ItemSource::Systemd,
            ItemSource::Manual,
            ItemSource::Custom,
        ]
    }

    pub fn parse(value: &str) -> Result<Self> {
        value
            .parse()
            .map_err(|_| ChapeauError::Validation(format!("invalid item source: {value}")))
    }
}

impl fmt::Display for ItemSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ItemSource {
    type Err = ChapeauError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "desktop" => Ok(Self::Desktop),
            "flatpak" => Ok(Self::Flatpak),
            "systemd" => Ok(Self::Systemd),
            "manual" => Ok(Self::Manual),
            "custom" => Ok(Self::Custom),
            _ => Err(ChapeauError::Validation(format!(
                "invalid item source: {value}"
            ))),
        }
    }
}

/// Scope for resources that can exist in more than one systemd manager.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    System,
    User,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
        }
    }

    pub fn all() -> &'static [Scope] {
        &[Scope::System, Scope::User]
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Scope {
    type Err = ChapeauError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "system" => Ok(Self::System),
            "user" => Ok(Self::User),
            _ => Err(ChapeauError::Validation(format!("invalid scope: {value}"))),
        }
    }
}

/// A discovered or user-created inventory entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Item {
    /// Chapeau-internal UUID.
    pub id: String,
    /// Whether this is an application or a service.
    pub kind: ItemKind,
    /// Display name.
    pub name: String,
    /// Where the item came from.
    pub source: ItemSource,
    /// Stable key within the source (desktop ID, Flatpak app ID, unit name,
    /// or manual slug).
    pub source_key: String,
    /// systemd manager scope, when applicable.
    pub scope: Option<Scope>,
    /// Version string, when known.
    pub version: Option<String>,
    /// Short description.
    pub description: Option<String>,
    /// Icon name or path.
    pub icon: Option<String>,
    /// Launch/start command or desktop file path, when applicable.
    pub command: Option<String>,
    /// Whether the item is currently present.
    pub installed: bool,
    /// Whether the user pinned it to the top of views.
    pub pinned: bool,
    /// Whether the user hid it.
    pub hidden: bool,
    /// Source-specific metadata.
    pub metadata: serde_json::Value,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-update timestamp.
    pub updated_at: String,
}

impl Item {
    /// Create a new item with a generated ID and current timestamps.
    pub fn new(
        kind: ItemKind,
        name: impl Into<String>,
        source: ItemSource,
        source_key: impl Into<String>,
        scope: Option<Scope>,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            name: name.into(),
            source,
            source_key: source_key.into(),
            scope,
            version: None,
            description: None,
            icon: None,
            command: None,
            installed: true,
            pinned: false,
            hidden: false,
            metadata: serde_json::json!({}),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn is_app(&self) -> bool {
        self.kind == ItemKind::App
    }

    pub fn is_service(&self) -> bool {
        self.kind == ItemKind::Service
    }

    /// Validate the item's required fields and source/kind consistency.
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(ChapeauError::Validation("item id must not be empty".into()));
        }
        if self.name.trim().is_empty() {
            return Err(ChapeauError::Validation(
                "item name must not be empty".into(),
            ));
        }
        if self.source_key.trim().is_empty() {
            return Err(ChapeauError::Validation(
                "item source_key must not be empty".into(),
            ));
        }

        match (self.kind, self.source) {
            (ItemKind::App, ItemSource::Systemd | ItemSource::Custom) => {
                return Err(ChapeauError::Validation(
                    "applications cannot use a service source".into(),
                ));
            }
            (ItemKind::Service, ItemSource::Desktop | ItemSource::Flatpak | ItemSource::Manual) => {
                return Err(ChapeauError::Validation(
                    "services cannot use an application source".into(),
                ));
            }
            _ => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_roundtrip() {
        for kind in ItemKind::all() {
            assert_eq!(ItemKind::from_str(kind.as_str()).unwrap(), *kind);
            assert_eq!(ItemKind::parse(kind.as_str()).unwrap(), *kind);
        }
    }

    #[test]
    fn source_roundtrip() {
        for source in ItemSource::all() {
            assert_eq!(ItemSource::from_str(source.as_str()).unwrap(), *source);
            assert_eq!(ItemSource::parse(source.as_str()).unwrap(), *source);
        }
    }

    #[test]
    fn scope_roundtrip() {
        for scope in Scope::all() {
            assert_eq!(Scope::from_str(scope.as_str()).unwrap(), *scope);
        }
    }

    #[test]
    fn invalid_values_are_rejected() {
        assert!(ItemKind::from_str("bogus").is_err());
        assert!(ItemSource::from_str("bogus").is_err());
        assert!(Scope::from_str("bogus").is_err());
    }

    #[test]
    fn new_item_has_expected_defaults() {
        let item = Item::new(
            ItemKind::App,
            "Visual Studio Code",
            ItemSource::Desktop,
            "code.desktop",
            None,
        );
        assert!(!item.id.is_empty());
        assert!(item.installed);
        assert!(!item.pinned);
        assert!(!item.hidden);
        assert_eq!(item.metadata, serde_json::json!({}));
        assert_eq!(item.created_at, item.updated_at);
    }

    #[test]
    fn validate_rejects_invalid_sources() {
        let mut item = Item::new(
            ItemKind::App,
            "PostgreSQL",
            ItemSource::Systemd,
            "postgresql.service",
            Some(Scope::System),
        );
        assert!(item.validate().is_err());

        item.kind = ItemKind::Service;
        item.source = ItemSource::Desktop;
        assert!(item.validate().is_err());
    }

    #[test]
    fn validate_accepts_valid_combinations() {
        for (kind, source, scope) in [
            (ItemKind::App, ItemSource::Desktop, None),
            (ItemKind::App, ItemSource::Flatpak, None),
            (ItemKind::App, ItemSource::Manual, None),
            (ItemKind::Service, ItemSource::Systemd, Some(Scope::System)),
            (ItemKind::Service, ItemSource::Custom, None),
        ] {
            let item = Item::new(kind, "name", source, "key", scope);
            assert!(item.validate().is_ok(), "{kind}/{source}");
        }
    }

    #[test]
    fn serialization_roundtrip() {
        let mut item = Item::new(
            ItemKind::Service,
            "PostgreSQL",
            ItemSource::Systemd,
            "postgresql.service",
            Some(Scope::System),
        );
        item.version = Some("16.4".into());
        item.metadata = serde_json::json!({"package": "postgresql-server"});
        let json = serde_json::to_string(&item).unwrap();
        let back: Item = serde_json::from_str(&json).unwrap();
        assert_eq!(item, back);
    }
}
