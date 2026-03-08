use std::path::{Path, PathBuf};

use crate::adapter::{AnyAdapter, DetectionResult, ToolAdapter};
use crate::config::{AisyncConfig, DefaultsConfig, SyncStrategy, ToolConfig, ToolsConfig};
use crate::detection::DetectionEngine;
use crate::error::{AisyncError, InitError};
use crate::types::ToolKind;

/// Options controlling init behavior. CLI layer populates these from user choices.
#[derive(Debug, Clone)]
pub struct InitOptions {
    /// Re-init even if .ai/ exists.
    pub force: bool,
    /// Which source to import from (decided by CLI layer).
    pub import_from: Option<ImportChoice>,
}

/// A discovered source of existing instructions.
#[derive(Debug, Clone)]
pub struct ImportSource {
    /// Which tool this content came from.
    pub tool: ToolKind,
    /// The instruction content (with tool-specific formatting stripped).
    pub content: String,
    /// Path to the source file.
    pub source_path: PathBuf,
}

/// User's choice about which import source to use.
#[derive(Debug, Clone)]
pub enum ImportChoice {
    /// Use a specific tool's content.
    UseSource(ToolKind),
    /// Don't import, start fresh.
    Skip,
}

/// Engine for initializing the .ai/ directory structure.
pub struct InitEngine;

impl InitEngine {
    /// Check if already initialized (.ai/ directory exists).
    pub fn is_initialized(project_root: &Path) -> bool {
        project_root.join(".ai").is_dir()
    }

    /// Detect tools in the project (delegates to DetectionEngine).
    pub fn detect_tools(project_root: &Path) -> Result<Vec<DetectionResult>, AisyncError> {
        Ok(DetectionEngine::scan(project_root)?)
    }

    /// Find existing instruction sources from detected tools.
    /// Calls read_instructions() on each detected adapter.
    pub fn find_import_sources(
        project_root: &Path,
        detected: &[DetectionResult],
    ) -> Vec<ImportSource> {
        let mut sources = Vec::new();

        for result in detected {
            let adapter = Self::adapter_for_tool(&result.tool);
            if let Ok(Some(content)) = adapter.read_instructions(project_root) {
                let source_path = project_root.join(adapter.native_instruction_path());
                sources.push(ImportSource {
                    tool: result.tool.clone(),
                    content,
                    source_path,
                });
            }
        }

        sources
    }

    /// Scaffold the .ai/ directory structure.
    /// Creates: .ai/instructions.md, .ai/memory/, .ai/hooks/, .ai/commands/, aisync.toml
    /// If import_content is Some, writes it to .ai/instructions.md.
    /// Otherwise creates empty .ai/instructions.md.
    /// Writes aisync.toml with detected tools enabled (or empty tools if none detected).
    pub fn scaffold(
        project_root: &Path,
        detected_tools: &[DetectionResult],
        import_content: Option<&str>,
        options: &InitOptions,
    ) -> Result<(), AisyncError> {
        let ai_dir = project_root.join(".ai");

        // Check for existing initialization
        if ai_dir.is_dir() && !options.force {
            return Err(InitError::AlreadyInitialized.into());
        }

        // Create directory structure
        let dirs = [
            ai_dir.clone(),
            ai_dir.join("memory"),
            ai_dir.join("hooks"),
            ai_dir.join("commands"),
        ];
        for dir in &dirs {
            std::fs::create_dir_all(dir).map_err(InitError::ScaffoldFailed)?;
        }

        // Write instructions.md
        let instructions_content = import_content.unwrap_or("");
        std::fs::write(ai_dir.join("instructions.md"), instructions_content)
            .map_err(InitError::ScaffoldFailed)?;

        // Build aisync.toml config from detected tools
        let config = Self::build_config(detected_tools);
        let toml_str = config
            .to_string_pretty()
            .map_err(|e| InitError::ImportFailed(format!("failed to serialize config: {e}")))?;
        std::fs::write(project_root.join("aisync.toml"), toml_str)
            .map_err(InitError::ScaffoldFailed)?;

        Ok(())
    }

    /// Get the appropriate adapter for a tool kind.
    fn adapter_for_tool(tool: &ToolKind) -> AnyAdapter {
        AnyAdapter::for_tool(tool).unwrap_or_else(|| {
            // For custom tools, fallback to ClaudeCode adapter until adapter registry exists
            AnyAdapter::ClaudeCode(crate::adapter::ClaudeCodeAdapter)
        })
    }

    /// Build an AisyncConfig from detected tools.
    fn build_config(detected_tools: &[DetectionResult]) -> AisyncConfig {
        let mut tools = ToolsConfig::default();

        for result in detected_tools {
            let adapter = Self::adapter_for_tool(&result.tool);
            let default_strategy = adapter.default_sync_strategy();
            let tool_config = if default_strategy != SyncStrategy::Symlink {
                ToolConfig {
                    enabled: true,
                    sync_strategy: Some(default_strategy),
                }
            } else {
                ToolConfig {
                    enabled: true,
                    sync_strategy: None,
                }
            };

            let key = result.tool.as_str().to_string();
            tools.set_tool(key, tool_config);
        }

        AisyncConfig {
            schema_version: 1,
            defaults: DefaultsConfig {
                sync_strategy: SyncStrategy::Symlink,
            },
            tools,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_scaffold_creates_directory_structure() {
        let dir = TempDir::new().unwrap();
        let options = InitOptions {
            force: false,
            import_from: None,
        };

        InitEngine::scaffold(dir.path(), &[], None, &options).unwrap();

        assert!(dir.path().join(".ai").is_dir());
        assert!(dir.path().join(".ai/instructions.md").exists());
        assert!(dir.path().join(".ai/memory").is_dir());
        assert!(dir.path().join(".ai/hooks").is_dir());
        assert!(dir.path().join(".ai/commands").is_dir());
        assert!(dir.path().join("aisync.toml").exists());
    }

    #[test]
    fn test_scaffold_creates_aisync_toml_with_detected_tools() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();
        std::fs::create_dir_all(dir.path().join(".cursor/rules")).unwrap();

        let detected = DetectionEngine::scan(dir.path()).unwrap();
        let options = InitOptions {
            force: false,
            import_from: None,
        };

        InitEngine::scaffold(dir.path(), &detected, None, &options).unwrap();

        let toml_content = std::fs::read_to_string(dir.path().join("aisync.toml")).unwrap();
        let config = AisyncConfig::from_str(&toml_content).unwrap();
        assert_eq!(config.schema_version, 1);
        assert!(config.tools.get_tool("claude-code").is_some());
        assert!(config.tools.get_tool("claude-code").unwrap().enabled);
        assert!(config.tools.get_tool("cursor").is_some());
        assert!(config.tools.get_tool("cursor").unwrap().enabled);
        // Cursor should always get Generate strategy
        assert_eq!(
            config.tools.get_tool("cursor").unwrap().sync_strategy,
            Some(SyncStrategy::Generate)
        );
    }

    #[test]
    fn test_scaffold_fails_with_already_initialized() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".ai")).unwrap();

        let options = InitOptions {
            force: false,
            import_from: None,
        };

        let result = InitEngine::scaffold(dir.path(), &[], None, &options);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, AisyncError::Init(InitError::AlreadyInitialized)),
            "expected AlreadyInitialized, got: {err:?}"
        );
    }

    #[test]
    fn test_scaffold_succeeds_with_force_when_already_initialized() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".ai")).unwrap();

        let options = InitOptions {
            force: true,
            import_from: None,
        };

        InitEngine::scaffold(dir.path(), &[], None, &options).unwrap();
        assert!(dir.path().join(".ai/instructions.md").exists());
        assert!(dir.path().join("aisync.toml").exists());
    }

    #[test]
    fn test_import_existing_reads_claude_md() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# My Project Rules").unwrap();

        let detected = DetectionEngine::scan(dir.path()).unwrap();
        let sources = InitEngine::find_import_sources(dir.path(), &detected);

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].tool, ToolKind::ClaudeCode);
        assert_eq!(sources[0].content, "# My Project Rules");
    }

    #[test]
    fn test_import_existing_reads_cursor_mdc_strips_frontmatter() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".cursor/rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        let mdc = "---\ndescription: test\nglobs: \"**\"\nalwaysApply: true\n---\n\n# Cursor Instructions";
        std::fs::write(rules_dir.join("project.mdc"), mdc).unwrap();

        let detected = DetectionEngine::scan(dir.path()).unwrap();
        let sources = InitEngine::find_import_sources(dir.path(), &detected);

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].tool, ToolKind::Cursor);
        assert_eq!(sources[0].content, "# Cursor Instructions");
    }

    #[test]
    fn test_import_existing_returns_multiple_sources() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# Claude Rules").unwrap();
        let rules_dir = dir.path().join(".cursor/rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        let mdc = "---\ndescription: test\nglobs: \"**\"\nalwaysApply: true\n---\n\n# Cursor Rules";
        std::fs::write(rules_dir.join("project.mdc"), mdc).unwrap();
        std::fs::write(dir.path().join("opencode.json"), "{}").unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "# OpenCode Rules").unwrap();

        let detected = DetectionEngine::scan(dir.path()).unwrap();
        let sources = InitEngine::find_import_sources(dir.path(), &detected);

        assert_eq!(sources.len(), 3);
        let tools: Vec<ToolKind> = sources.iter().map(|s| s.tool.clone()).collect();
        assert!(tools.contains(&ToolKind::ClaudeCode));
        assert!(tools.contains(&ToolKind::Cursor));
        assert!(tools.contains(&ToolKind::OpenCode));
    }

    #[test]
    fn test_import_existing_returns_empty_when_no_instructions() {
        let dir = TempDir::new().unwrap();
        // Cursor detected via directory, but no project.mdc file
        std::fs::create_dir_all(dir.path().join(".cursor/rules")).unwrap();

        let detected = DetectionEngine::scan(dir.path()).unwrap();
        assert!(!detected.is_empty(), "Cursor should be detected");

        let sources = InitEngine::find_import_sources(dir.path(), &detected);
        assert!(sources.is_empty());
    }

    #[test]
    fn test_scaffold_with_no_tools_creates_empty_tools_config() {
        let dir = TempDir::new().unwrap();
        let options = InitOptions {
            force: false,
            import_from: None,
        };

        InitEngine::scaffold(dir.path(), &[], None, &options).unwrap();

        let toml_content = std::fs::read_to_string(dir.path().join("aisync.toml")).unwrap();
        let config = AisyncConfig::from_str(&toml_content).unwrap();
        assert!(config.tools.get_tool("claude-code").is_none());
        assert!(config.tools.get_tool("cursor").is_none());
        assert!(config.tools.get_tool("opencode").is_none());
    }

    #[test]
    fn test_scaffold_writes_import_content_to_instructions() {
        let dir = TempDir::new().unwrap();
        let options = InitOptions {
            force: false,
            import_from: None,
        };

        InitEngine::scaffold(dir.path(), &[], Some("# Imported content"), &options).unwrap();

        let content = std::fs::read_to_string(dir.path().join(".ai/instructions.md")).unwrap();
        assert_eq!(content, "# Imported content");
    }

    #[test]
    fn test_scaffold_creates_empty_instructions_when_no_import() {
        let dir = TempDir::new().unwrap();
        let options = InitOptions {
            force: false,
            import_from: None,
        };

        InitEngine::scaffold(dir.path(), &[], None, &options).unwrap();

        let content = std::fs::read_to_string(dir.path().join(".ai/instructions.md")).unwrap();
        assert!(content.is_empty() || content.starts_with("# "));
    }

    #[test]
    fn test_is_initialized_true_when_ai_dir_exists() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".ai")).unwrap();
        assert!(InitEngine::is_initialized(dir.path()));
    }

    #[test]
    fn test_is_initialized_false_when_no_ai_dir() {
        let dir = TempDir::new().unwrap();
        assert!(!InitEngine::is_initialized(dir.path()));
    }

    #[test]
    fn test_detect_tools_delegates_to_detection_engine() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();

        let results = InitEngine::detect_tools(dir.path()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool, ToolKind::ClaudeCode);
    }
}
