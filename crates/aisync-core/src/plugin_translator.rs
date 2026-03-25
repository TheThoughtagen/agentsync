use std::path::Path;

use crate::error::{AisyncError, ConfigError};
use crate::types::{
    CanonicalPluginManifest, ExportReport, ImportReport, ToolKind,
};
use serde::de::Error as _;

/// Engine for translating plugins between tool-native and canonical formats.
pub struct PluginTranslator;

impl PluginTranslator {
    /// Import a tool-native plugin directory into canonical format.
    ///
    /// Detects the source tool, reads its native config files, and writes
    /// the canonical representation under the plugin directory.
    pub fn import(
        _plugin_path: &Path,
        _source_tool: Option<ToolKind>,
    ) -> Result<ImportReport, AisyncError> {
        todo!("import implementation in Task B")
    }

    /// Export a canonical plugin to one or more tool-native formats.
    ///
    /// Reads `plugin.toml` and the canonical component files, then generates
    /// tool-native output for each requested target tool.
    pub fn export(
        _plugin_path: &Path,
        _targets: &[ToolKind],
    ) -> Result<Vec<ExportReport>, AisyncError> {
        todo!("export implementation in Task C")
    }

    /// Load a `plugin.toml` manifest from the given plugin directory.
    pub fn load_manifest(plugin_path: &Path) -> Result<CanonicalPluginManifest, AisyncError> {
        let manifest_path = plugin_path.join("plugin.toml");
        let raw = std::fs::read_to_string(&manifest_path)
            .map_err(|e| AisyncError::Config(ConfigError::ReadFile(e)))?;
        let manifest: CanonicalPluginManifest =
            toml::from_str(&raw).map_err(|e| AisyncError::Config(ConfigError::Parse(e)))?;
        Ok(manifest)
    }

    /// Save a `plugin.toml` manifest to the given plugin directory.
    pub fn save_manifest(
        plugin_path: &Path,
        manifest: &CanonicalPluginManifest,
    ) -> Result<(), AisyncError> {
        let manifest_path = plugin_path.join("plugin.toml");
        let content = toml::to_string_pretty(manifest)
            .map_err(|e| AisyncError::Config(ConfigError::Parse(toml::de::Error::custom(e.to_string()))))?;
        std::fs::write(&manifest_path, content)
            .map_err(|e| AisyncError::Config(ConfigError::ReadFile(e)))?;
        Ok(())
    }
}

/// Auto-detect which tool created a plugin directory by checking for
/// tool-specific marker files.
///
/// Detection priority:
/// 1. `.claude-plugin/plugin.json` → ClaudeCode
/// 2. `.cursor/` directory → Cursor
/// 3. `opencode.json` or `.opencode/` → OpenCode
fn detect_source_tool(path: &Path) -> Result<ToolKind, AisyncError> {
    // Claude Code: look for .claude-plugin/plugin.json
    if path.join(".claude-plugin/plugin.json").exists() {
        return Ok(ToolKind::ClaudeCode);
    }

    // Cursor: look for .cursor/ directory
    if path.join(".cursor").is_dir() {
        return Ok(ToolKind::Cursor);
    }

    // OpenCode: look for opencode.json or .opencode/ directory
    if path.join("opencode.json").exists() || path.join(".opencode").is_dir() {
        return Ok(ToolKind::OpenCode);
    }

    Err(AisyncError::Config(ConfigError::ReadFile(
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "could not detect source tool in '{}': no known tool markers found",
                path.display()
            ),
        ),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CanonicalPluginManifest, PluginComponents, PluginMetadata};
    use tempfile::TempDir;

    // --- detect_source_tool tests ---

    #[test]
    fn test_detect_claude_code() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude-plugin")).unwrap();
        std::fs::write(dir.path().join(".claude-plugin/plugin.json"), "{}").unwrap();

        let result = detect_source_tool(dir.path()).unwrap();
        assert_eq!(result, ToolKind::ClaudeCode);
    }

    #[test]
    fn test_detect_cursor() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".cursor")).unwrap();

        let result = detect_source_tool(dir.path()).unwrap();
        assert_eq!(result, ToolKind::Cursor);
    }

    #[test]
    fn test_detect_opencode_json() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("opencode.json"), "{}").unwrap();

        let result = detect_source_tool(dir.path()).unwrap();
        assert_eq!(result, ToolKind::OpenCode);
    }

    #[test]
    fn test_detect_opencode_dir() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".opencode")).unwrap();

        let result = detect_source_tool(dir.path()).unwrap();
        assert_eq!(result, ToolKind::OpenCode);
    }

    #[test]
    fn test_detect_unknown_returns_error() {
        let dir = TempDir::new().unwrap();
        let result = detect_source_tool(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_claude_code_takes_priority_over_cursor() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude-plugin")).unwrap();
        std::fs::write(dir.path().join(".claude-plugin/plugin.json"), "{}").unwrap();
        std::fs::create_dir_all(dir.path().join(".cursor")).unwrap();

        let result = detect_source_tool(dir.path()).unwrap();
        assert_eq!(result, ToolKind::ClaudeCode);
    }

    // --- load_manifest / save_manifest tests ---

    #[test]
    fn test_load_manifest() {
        let dir = TempDir::new().unwrap();
        let toml_content = r#"
[metadata]
name = "my-plugin"
version = "0.1.0"
description = "Test plugin"
source_tool = "cursor"

[components]
has_instructions = true
has_rules = true
"#;
        std::fs::write(dir.path().join("plugin.toml"), toml_content).unwrap();

        let manifest = PluginTranslator::load_manifest(dir.path()).unwrap();
        assert_eq!(manifest.metadata.name, "my-plugin");
        assert_eq!(manifest.metadata.version, Some("0.1.0".to_string()));
        assert!(manifest.components.has_instructions);
        assert!(manifest.components.has_rules);
        assert!(!manifest.components.has_hooks);
    }

    #[test]
    fn test_load_manifest_missing_file_returns_error() {
        let dir = TempDir::new().unwrap();
        let result = PluginTranslator::load_manifest(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_save_and_load_manifest_roundtrip() {
        let dir = TempDir::new().unwrap();
        let manifest = CanonicalPluginManifest {
            metadata: PluginMetadata {
                name: "roundtrip-test".to_string(),
                version: Some("2.0.0".to_string()),
                description: Some("Roundtrip test".to_string()),
                source_tool: Some("claude-code".to_string()),
            },
            components: PluginComponents {
                has_instructions: true,
                has_hooks: false,
                has_mcp: true,
                has_rules: false,
                has_commands: true,
                has_skills: false,
                has_agents: true,
            },
        };

        PluginTranslator::save_manifest(dir.path(), &manifest).unwrap();
        let loaded = PluginTranslator::load_manifest(dir.path()).unwrap();

        assert_eq!(loaded.metadata.name, "roundtrip-test");
        assert_eq!(loaded.metadata.version, Some("2.0.0".to_string()));
        assert_eq!(loaded.metadata.description, Some("Roundtrip test".to_string()));
        assert_eq!(loaded.metadata.source_tool, Some("claude-code".to_string()));
        assert!(loaded.components.has_instructions);
        assert!(!loaded.components.has_hooks);
        assert!(loaded.components.has_mcp);
        assert!(!loaded.components.has_rules);
        assert!(loaded.components.has_commands);
        assert!(!loaded.components.has_skills);
        assert!(loaded.components.has_agents);
    }
}
