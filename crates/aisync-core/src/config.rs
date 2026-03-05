use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ConfigError;

/// Top-level configuration parsed from aisync.toml.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AisyncConfig {
    pub schema_version: u32,

    #[serde(default)]
    pub defaults: DefaultsConfig,

    #[serde(default)]
    pub tools: ToolsConfig,
}

/// Global default settings that tools inherit from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DefaultsConfig {
    #[serde(default)]
    pub sync_strategy: SyncStrategy,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            sync_strategy: SyncStrategy::default(),
        }
    }
}

/// Per-tool configuration sections.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ToolsConfig {
    #[serde(rename = "claude-code")]
    pub claude_code: Option<ToolConfig>,

    pub cursor: Option<ToolConfig>,

    pub opencode: Option<ToolConfig>,
}

/// Configuration for an individual tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    pub sync_strategy: Option<SyncStrategy>,
}

fn default_true() -> bool {
    true
}

impl ToolConfig {
    /// Returns the effective sync strategy for this tool, falling back to the
    /// global default if no tool-specific override is set.
    pub fn effective_sync_strategy(&self, defaults: &DefaultsConfig) -> SyncStrategy {
        self.sync_strategy.unwrap_or(defaults.sync_strategy)
    }
}

/// Strategy for synchronizing configuration files between tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncStrategy {
    Symlink,
    Copy,
    Generate,
}

impl Default for SyncStrategy {
    fn default() -> Self {
        Self::Symlink
    }
}

impl AisyncConfig {
    /// Parse an `AisyncConfig` from a TOML string.
    ///
    /// Returns `ConfigError::UnsupportedVersion` if `schema_version` is not 1.
    pub fn from_str(s: &str) -> Result<Self, ConfigError> {
        let config: AisyncConfig = toml::from_str(s)?;
        if config.schema_version != 1 {
            return Err(ConfigError::UnsupportedVersion {
                version: config.schema_version,
                expected: 1,
            });
        }
        Ok(config)
    }

    /// Parse an `AisyncConfig` from a file path.
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        Self::from_str(&content)
    }

    /// Serialize the config to a pretty-printed TOML string.
    pub fn to_string_pretty(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"schema_version = 1"#;
        let config = AisyncConfig::from_str(toml).unwrap();
        assert_eq!(config.schema_version, 1);
        assert_eq!(config.defaults.sync_strategy, SyncStrategy::Symlink);
        assert!(config.tools.claude_code.is_none());
        assert!(config.tools.cursor.is_none());
        assert!(config.tools.opencode.is_none());
    }

    #[test]
    fn test_parse_full_config() {
        let toml = r#"
schema_version = 1

[defaults]
sync_strategy = "copy"

[tools.claude-code]
enabled = true
sync_strategy = "symlink"

[tools.cursor]
enabled = true
sync_strategy = "generate"

[tools.opencode]
enabled = false
"#;
        let config = AisyncConfig::from_str(toml).unwrap();
        assert_eq!(config.schema_version, 1);
        assert_eq!(config.defaults.sync_strategy, SyncStrategy::Copy);

        let claude = config.tools.claude_code.as_ref().unwrap();
        assert!(claude.enabled);
        assert_eq!(claude.sync_strategy, Some(SyncStrategy::Symlink));

        let cursor = config.tools.cursor.as_ref().unwrap();
        assert!(cursor.enabled);
        assert_eq!(cursor.sync_strategy, Some(SyncStrategy::Generate));

        let opencode = config.tools.opencode.as_ref().unwrap();
        assert!(!opencode.enabled);
        assert_eq!(opencode.sync_strategy, None);
    }

    #[test]
    fn test_reject_wrong_schema_version() {
        let toml = r#"schema_version = 2"#;
        let err = AisyncConfig::from_str(toml).unwrap_err();
        match err {
            ConfigError::UnsupportedVersion { version, expected } => {
                assert_eq!(version, 2);
                assert_eq!(expected, 1);
            }
            other => panic!("expected UnsupportedVersion, got: {other}"),
        }
    }

    #[test]
    fn test_sync_strategy_override() {
        let defaults = DefaultsConfig {
            sync_strategy: SyncStrategy::Copy,
        };

        // Tool with override
        let tool_with = ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Generate),
        };
        assert_eq!(
            tool_with.effective_sync_strategy(&defaults),
            SyncStrategy::Generate
        );

        // Tool without override -- falls back to default
        let tool_without = ToolConfig {
            enabled: true,
            sync_strategy: None,
        };
        assert_eq!(
            tool_without.effective_sync_strategy(&defaults),
            SyncStrategy::Copy
        );
    }

    #[test]
    fn test_round_trip() {
        let toml_input = r#"
schema_version = 1

[defaults]
sync_strategy = "copy"

[tools.claude-code]
enabled = true
sync_strategy = "symlink"
"#;
        let config1 = AisyncConfig::from_str(toml_input).unwrap();
        let serialized = config1.to_string_pretty().unwrap();
        let config2 = AisyncConfig::from_str(&serialized).unwrap();
        assert_eq!(config1, config2);
    }

    #[test]
    fn test_claude_code_rename() {
        let toml = r#"
schema_version = 1

[tools.claude-code]
enabled = true
"#;
        let config = AisyncConfig::from_str(toml).unwrap();
        assert!(config.tools.claude_code.is_some());
        let claude = config.tools.claude_code.unwrap();
        assert!(claude.enabled);
    }

    #[test]
    fn test_sync_strategy_serializes_lowercase() {
        let config = AisyncConfig {
            schema_version: 1,
            defaults: DefaultsConfig {
                sync_strategy: SyncStrategy::Symlink,
            },
            tools: ToolsConfig::default(),
        };
        let serialized = config.to_string_pretty().unwrap();
        assert!(serialized.contains("\"symlink\""));
    }

    #[test]
    fn test_default_enabled_true() {
        let toml = r#"
schema_version = 1

[tools.cursor]
sync_strategy = "copy"
"#;
        let config = AisyncConfig::from_str(toml).unwrap();
        let cursor = config.tools.cursor.unwrap();
        assert!(cursor.enabled); // defaults to true
    }
}
