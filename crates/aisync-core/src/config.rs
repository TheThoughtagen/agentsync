use std::collections::BTreeMap;
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DefaultsConfig {
    #[serde(default)]
    pub sync_strategy: SyncStrategy,
}

/// Per-tool configuration sections.
///
/// Uses a `BTreeMap<String, ToolConfig>` with `#[serde(flatten)]` so that
/// any `[tools.<name>]` TOML section is accepted without code changes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ToolsConfig {
    #[serde(flatten)]
    tools: BTreeMap<String, ToolConfig>,
}

impl ToolsConfig {
    /// Returns the configuration for a tool by name, if configured.
    pub fn get_tool(&self, name: &str) -> Option<&ToolConfig> {
        self.tools.get(name)
    }

    /// Iterates over all configured tools as `(name, config)` pairs.
    pub fn configured_tools(&self) -> impl Iterator<Item = (&str, &ToolConfig)> {
        self.tools.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Returns whether a tool is enabled.
    ///
    /// Tools must be explicitly listed in aisync.toml to be enabled.
    /// Unconfigured tools are treated as disabled.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.tools.get(name).is_some_and(|tc| tc.enabled)
    }

    /// Adds or replaces a tool configuration entry.
    pub fn set_tool(&mut self, name: String, config: ToolConfig) {
        self.tools.insert(name, config);
    }
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

// SyncStrategy now lives in aisync-types; re-export for backward compatibility.
pub use aisync_types::SyncStrategy;

impl AisyncConfig {
    /// Parse an `AisyncConfig` from a TOML string.
    ///
    /// Returns `ConfigError::UnsupportedVersion` if `schema_version` is not 1.
    #[allow(clippy::should_implement_trait)]
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
        assert!(config.tools.get_tool("claude-code").is_none());
        assert!(config.tools.get_tool("cursor").is_none());
        assert!(config.tools.get_tool("opencode").is_none());
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

        let claude = config.tools.get_tool("claude-code").unwrap();
        assert!(claude.enabled);
        assert_eq!(claude.sync_strategy, Some(SyncStrategy::Symlink));

        let cursor = config.tools.get_tool("cursor").unwrap();
        assert!(cursor.enabled);
        assert_eq!(cursor.sync_strategy, Some(SyncStrategy::Generate));

        let opencode = config.tools.get_tool("opencode").unwrap();
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
        assert!(config.tools.get_tool("claude-code").is_some());
        let claude = config.tools.get_tool("claude-code").unwrap();
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
        let cursor = config.tools.get_tool("cursor").unwrap();
        assert!(cursor.enabled); // defaults to true
    }

    #[test]
    fn test_arbitrary_tool_name() {
        let toml = r#"
schema_version = 1

[tools.windsurf]
enabled = true
sync_strategy = "copy"
"#;
        let config = AisyncConfig::from_str(toml).unwrap();
        let windsurf = config.tools.get_tool("windsurf").unwrap();
        assert!(windsurf.enabled);
        assert_eq!(windsurf.sync_strategy, Some(SyncStrategy::Copy));
    }

    #[test]
    fn test_is_enabled_unconfigured() {
        let config = AisyncConfig::from_str("schema_version = 1").unwrap();
        assert!(!config.tools.is_enabled("nonexistent"));
    }

    #[test]
    fn test_is_enabled_disabled() {
        let toml = r#"
schema_version = 1

[tools.cursor]
enabled = false
"#;
        let config = AisyncConfig::from_str(toml).unwrap();
        assert!(!config.tools.is_enabled("cursor"));
    }

    #[test]
    fn test_set_tool() {
        let mut tools = ToolsConfig::default();
        tools.set_tool(
            "windsurf".to_string(),
            ToolConfig {
                enabled: true,
                sync_strategy: Some(SyncStrategy::Copy),
            },
        );
        let windsurf = tools.get_tool("windsurf").unwrap();
        assert!(windsurf.enabled);
        assert_eq!(windsurf.sync_strategy, Some(SyncStrategy::Copy));
    }

    #[test]
    fn test_configured_tools_iteration() {
        let toml = r#"
schema_version = 1

[tools.claude-code]
enabled = true

[tools.cursor]
enabled = true

[tools.opencode]
enabled = false
"#;
        let config = AisyncConfig::from_str(toml).unwrap();
        let tool_names: Vec<&str> = config.tools.configured_tools().map(|(k, _)| k).collect();
        assert_eq!(tool_names.len(), 3);
        assert!(tool_names.contains(&"claude-code"));
        assert!(tool_names.contains(&"cursor"));
        assert!(tool_names.contains(&"opencode"));
    }

    #[test]
    fn test_round_trip_with_multiple_tools() {
        let toml_input = r#"
schema_version = 1

[defaults]
sync_strategy = "symlink"

[tools.claude-code]
enabled = true
sync_strategy = "symlink"

[tools.cursor]
enabled = true
sync_strategy = "generate"

[tools.opencode]
enabled = false

[tools.windsurf]
enabled = true
sync_strategy = "copy"
"#;
        let config1 = AisyncConfig::from_str(toml_input).unwrap();
        let serialized = config1.to_string_pretty().unwrap();
        let config2 = AisyncConfig::from_str(&serialized).unwrap();
        assert_eq!(config1, config2);
    }
}
