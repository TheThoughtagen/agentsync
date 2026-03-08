use std::path::Path;

use crate::adapter::{AnyAdapter, DetectionResult, ToolAdapter};
use crate::config::AisyncConfig;
use crate::error::{AisyncError, InitError};
use crate::types::ToolKind;

/// Engine for discovering unconfigured tools and adding them to the project config.
pub struct AddToolEngine;

impl AddToolEngine {
    /// Discover tools that are detected on disk but not yet configured in aisync.toml.
    ///
    /// Uses `get_tool().is_none()` to check for unconfigured tools (NOT `!is_enabled()`),
    /// because unconfigured-is-enabled semantics mean `is_enabled()` returns true for absent tools.
    /// Tools with `enabled = false` are still "configured" and will NOT appear in results.
    pub fn discover_unconfigured(
        config: &AisyncConfig,
        project_root: &Path,
    ) -> Result<Vec<DetectionResult>, AisyncError> {
        let detected = crate::detection::DetectionEngine::scan(project_root)?;
        Ok(detected
            .into_iter()
            .filter(|d| config.tools.get_tool(d.tool.as_str()).is_none())
            .collect())
    }

    /// Add the specified tools to the project config and write the updated aisync.toml.
    ///
    /// For each tool, looks up the adapter's default sync strategy. If the default is
    /// `Symlink` (the global default), `sync_strategy` is set to `None` to keep the
    /// TOML output clean. Otherwise, `sync_strategy` is set to `Some(strategy)`.
    pub fn add_tools(
        config: &mut AisyncConfig,
        tools: &[ToolKind],
        project_root: &Path,
    ) -> Result<(), AisyncError> {
        use crate::config::{SyncStrategy, ToolConfig};

        for tool in tools {
            if let Some(adapter) = AnyAdapter::for_tool(tool) {
                let default_strategy = adapter.default_sync_strategy();
                let sync_strategy = if default_strategy == SyncStrategy::Symlink {
                    None
                } else {
                    Some(default_strategy)
                };
                let tool_config = ToolConfig {
                    enabled: true,
                    sync_strategy,
                };
                config
                    .tools
                    .set_tool(tool.as_str().to_string(), tool_config);
            }
        }

        let toml_str = config
            .to_string_pretty()
            .map_err(|e| AisyncError::Init(InitError::ImportFailed(format!("{e}"))))?;

        std::fs::write(project_root.join("aisync.toml"), toml_str)
            .map_err(|e| AisyncError::Init(InitError::ScaffoldFailed(e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AisyncConfig, DefaultsConfig, SyncStrategy, ToolConfig, ToolsConfig};
    use tempfile::TempDir;

    fn minimal_config() -> AisyncConfig {
        AisyncConfig {
            schema_version: 1,
            defaults: DefaultsConfig::default(),
            tools: ToolsConfig::default(),
        }
    }

    fn config_with_claude() -> AisyncConfig {
        let mut config = minimal_config();
        config.tools.set_tool(
            "claude-code".to_string(),
            ToolConfig {
                enabled: true,
                sync_strategy: None,
            },
        );
        config
    }

    // ---- discover_unconfigured tests ----

    #[test]
    fn test_discover_unconfigured_returns_only_unconfigured_tools() {
        let dir = TempDir::new().unwrap();
        // Claude markers
        std::fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();
        // Cursor markers
        std::fs::create_dir_all(dir.path().join(".cursor/rules")).unwrap();

        let config = config_with_claude();
        let unconfigured = AddToolEngine::discover_unconfigured(&config, dir.path()).unwrap();

        // Claude is configured, so only cursor should appear
        let tools: Vec<&ToolKind> = unconfigured.iter().map(|d| &d.tool).collect();
        assert!(
            !tools.contains(&&ToolKind::ClaudeCode),
            "claude-code should not appear (already configured)"
        );
        assert!(
            tools.contains(&&ToolKind::Cursor),
            "cursor should appear (not configured)"
        );
    }

    #[test]
    fn test_discover_unconfigured_all_configured_returns_empty() {
        let dir = TempDir::new().unwrap();
        // Claude markers
        std::fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();
        // Cursor markers
        std::fs::create_dir_all(dir.path().join(".cursor/rules")).unwrap();

        let mut config = config_with_claude();
        config.tools.set_tool(
            "cursor".to_string(),
            ToolConfig {
                enabled: true,
                sync_strategy: None,
            },
        );

        let unconfigured = AddToolEngine::discover_unconfigured(&config, dir.path()).unwrap();
        assert!(
            unconfigured.is_empty(),
            "all detected tools are configured, expected empty"
        );
    }

    #[test]
    fn test_discover_unconfigured_disabled_tools_not_returned() {
        let dir = TempDir::new().unwrap();
        // Claude markers
        std::fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();
        // Cursor markers
        std::fs::create_dir_all(dir.path().join(".cursor/rules")).unwrap();

        // cursor is configured but disabled
        let mut config = config_with_claude();
        config.tools.set_tool(
            "cursor".to_string(),
            ToolConfig {
                enabled: false,
                sync_strategy: None,
            },
        );

        let unconfigured = AddToolEngine::discover_unconfigured(&config, dir.path()).unwrap();
        // cursor is configured (just disabled), so it should NOT appear
        let tools: Vec<&ToolKind> = unconfigured.iter().map(|d| &d.tool).collect();
        assert!(
            !tools.contains(&&ToolKind::Cursor),
            "disabled but configured cursor should not appear as unconfigured"
        );
    }

    // ---- add_tools tests ----

    #[test]
    fn test_add_tools_windsurf_has_generate_strategy() {
        let dir = TempDir::new().unwrap();
        // Write initial aisync.toml
        let mut config = config_with_claude();
        let toml_str = config.to_string_pretty().unwrap();
        std::fs::write(dir.path().join("aisync.toml"), &toml_str).unwrap();

        AddToolEngine::add_tools(&mut config, &[ToolKind::Windsurf], dir.path()).unwrap();

        // Re-read config from disk
        let written = AisyncConfig::from_file(&dir.path().join("aisync.toml")).unwrap();
        let windsurf = written.tools.get_tool("windsurf").unwrap();
        assert!(windsurf.enabled);
        assert_eq!(
            windsurf.sync_strategy,
            Some(SyncStrategy::Generate),
            "windsurf default is Generate"
        );
    }

    #[test]
    fn test_add_tools_claude_code_has_no_sync_strategy() {
        let dir = TempDir::new().unwrap();
        let mut config = minimal_config();
        let toml_str = config.to_string_pretty().unwrap();
        std::fs::write(dir.path().join("aisync.toml"), &toml_str).unwrap();

        AddToolEngine::add_tools(&mut config, &[ToolKind::ClaudeCode], dir.path()).unwrap();

        let written = AisyncConfig::from_file(&dir.path().join("aisync.toml")).unwrap();
        let claude = written.tools.get_tool("claude-code").unwrap();
        assert!(claude.enabled);
        assert_eq!(
            claude.sync_strategy, None,
            "claude-code default is Symlink, so sync_strategy should be None"
        );
    }

    #[test]
    fn test_add_tools_preserves_existing_sections() {
        let dir = TempDir::new().unwrap();
        let mut config = config_with_claude();
        let toml_str = config.to_string_pretty().unwrap();
        std::fs::write(dir.path().join("aisync.toml"), &toml_str).unwrap();

        AddToolEngine::add_tools(&mut config, &[ToolKind::Windsurf], dir.path()).unwrap();

        let written = AisyncConfig::from_file(&dir.path().join("aisync.toml")).unwrap();
        // Original claude-code section should still be present
        assert!(
            written.tools.get_tool("claude-code").is_some(),
            "existing claude-code section should be preserved"
        );
        // New windsurf section should exist
        assert!(
            written.tools.get_tool("windsurf").is_some(),
            "new windsurf section should exist"
        );
    }

    #[test]
    fn test_add_tools_nonexistent_dir_returns_io_error() {
        let mut config = minimal_config();
        let result = AddToolEngine::add_tools(
            &mut config,
            &[ToolKind::Windsurf],
            Path::new("/nonexistent/path/xyz"),
        );
        assert!(result.is_err(), "should fail when aisync.toml path is invalid");
        let err = result.unwrap_err();
        assert!(
            matches!(err, AisyncError::Init(InitError::ScaffoldFailed(_))),
            "expected ScaffoldFailed, got: {err:?}"
        );
    }
}
