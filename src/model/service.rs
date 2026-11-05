use crate::errors::{ChapeauError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A user-defined command service.
///
/// Custom services always run as the current user. They are deliberately
/// separate from systemd services so that no custom command is ever run
/// through `sudo` or `pkexec`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomServiceDefinition {
    /// The service item this definition belongs to.
    pub item_id: String,
    /// Command used to start the service.
    pub start_command: String,
    /// Command used to stop the service.
    pub stop_command: Option<String>,
    /// Optional command whose exit code reports whether the service is running.
    pub status_command: Option<String>,
    /// Working directory for the commands.
    pub working_dir: Option<String>,
    /// Extra environment variables.
    pub env: BTreeMap<String, String>,
    /// TCP port used for a health check.
    pub health_port: Option<u16>,
    /// HTTP(S) URL used for a health check.
    pub health_url: Option<String>,
    /// File that receives the service's stdout/stderr.
    pub log_path: Option<String>,
    /// Ordering hint within a group.
    pub start_order: i64,
    /// Whether the definition is enabled.
    pub enabled: bool,
}

impl CustomServiceDefinition {
    pub fn new(item_id: impl Into<String>, start_command: impl Into<String>) -> Self {
        Self {
            item_id: item_id.into(),
            start_command: start_command.into(),
            stop_command: None,
            status_command: None,
            working_dir: None,
            env: BTreeMap::new(),
            health_port: None,
            health_url: None,
            log_path: None,
            start_order: 0,
            enabled: true,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.item_id.is_empty() {
            return Err(ChapeauError::Validation(
                "custom service item_id must not be empty".into(),
            ));
        }
        if self.start_command.trim().is_empty() {
            return Err(ChapeauError::Validation(
                "custom service start_command must not be empty".into(),
            ));
        }
        if self
            .working_dir
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(ChapeauError::Validation(
                "custom service working_dir must not be empty".into(),
            ));
        }
        if self
            .log_path
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(ChapeauError::Validation(
                "custom service log_path must not be empty".into(),
            ));
        }
        if let Some(url) = &self.health_url {
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return Err(ChapeauError::Validation(
                    "custom service health_url must start with http:// or https://".into(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_definition_defaults() {
        let def = CustomServiceDefinition::new("item-1", "npm run dev");
        assert_eq!(def.start_command, "npm run dev");
        assert!(def.stop_command.is_none());
        assert!(def.env.is_empty());
        assert!(def.enabled);
        assert_eq!(def.start_order, 0);
    }

    #[test]
    fn validate_rejects_missing_commands() {
        let mut def = CustomServiceDefinition::new("item-1", "   ");
        assert!(def.validate().is_err());

        def.start_command = "start.sh".into();
        def.item_id = String::new();
        assert!(def.validate().is_err());
    }

    #[test]
    fn validate_rejects_bad_health_url() {
        let mut def = CustomServiceDefinition::new("item-1", "start.sh");
        def.health_url = Some("localhost:3000".into());
        assert!(def.validate().is_err());
        def.health_url = Some("http://localhost:3000/health".into());
        assert!(def.validate().is_ok());
    }

    #[test]
    fn serialization_roundtrip() {
        let mut def = CustomServiceDefinition::new("item-1", "start.sh");
        def.stop_command = Some("stop.sh".into());
        def.env.insert("NODE_ENV".into(), "development".into());
        def.health_port = Some(3000);
        let json = serde_json::to_string(&def).unwrap();
        let back: CustomServiceDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(def, back);
    }
}
