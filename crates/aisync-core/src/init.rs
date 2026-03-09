use std::path::{Path, PathBuf};

use crate::adapter::{AnyAdapter, DetectionResult, ToolAdapter};
use crate::config::{AisyncConfig, DefaultsConfig, SyncStrategy, ToolConfig, ToolsConfig};
use crate::detection::DetectionEngine;
use crate::error::{AisyncError, InitError};
use crate::types::{RuleMetadata, ToolKind};

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

        // Import existing tool-native rules into .ai/rules/
        Self::import_rules(project_root)?;

        // Import existing Claude Code commands into .ai/commands/
        Self::import_commands(project_root)?;

        Ok(())
    }

    /// Import existing tool-native rule files into canonical .ai/rules/ format.
    ///
    /// Scans Cursor .mdc and Windsurf .md rule directories, translates frontmatter,
    /// and writes canonical rule files. Skips project.mdc/project.md and aisync-* managed files.
    pub fn import_rules(project_root: &Path) -> Result<usize, AisyncError> {
        let rules_dir = project_root.join(".ai/rules");
        std::fs::create_dir_all(&rules_dir)
            .map_err(|e| InitError::ScaffoldFailed(e))?;

        let mut count = 0;

        // Import from .cursor/rules/*.mdc
        let cursor_rules_dir = project_root.join(".cursor/rules");
        if cursor_rules_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&cursor_rules_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "mdc") {
                        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                        // Skip project.mdc and aisync-* managed files
                        if stem == "project" || stem.starts_with("aisync-") {
                            continue;
                        }
                        let raw = std::fs::read_to_string(&path)
                            .map_err(|e| InitError::ImportFailed(format!("read {}: {e}", path.display())))?;
                        let (metadata, content) = parse_cursor_rule(&raw)?;
                        let output = rules_dir.join(format!("{stem}.md"));
                        write_canonical_rule(&output, &metadata, &content)?;
                        count += 1;
                    }
                }
            }
        }

        // Import from .windsurf/rules/*.md
        let windsurf_rules_dir = project_root.join(".windsurf/rules");
        if windsurf_rules_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&windsurf_rules_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "md") {
                        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                        // Skip project.md and aisync-* managed files
                        if stem == "project" || stem.starts_with("aisync-") {
                            continue;
                        }
                        let raw = std::fs::read_to_string(&path)
                            .map_err(|e| InitError::ImportFailed(format!("read {}: {e}", path.display())))?;
                        let (metadata, content) = parse_windsurf_rule(&raw)?;
                        let output = rules_dir.join(format!("{stem}.md"));
                        write_canonical_rule(&output, &metadata, &content)?;
                        count += 1;
                    }
                }
            }
        }

        Ok(count)
    }

    /// Import existing Claude Code command files into canonical .ai/commands/ directory.
    ///
    /// Scans `.claude/commands/` for `*.md` files and copies them to `.ai/commands/`.
    /// Skips files with `aisync-` prefix (managed files). Creates `.ai/commands/` if missing.
    /// Returns the count of imported files.
    pub fn import_commands(project_root: &Path) -> Result<usize, AisyncError> {
        let commands_dir = project_root.join(".ai/commands");
        std::fs::create_dir_all(&commands_dir)
            .map_err(|e| InitError::ScaffoldFailed(e))?;

        let claude_commands_dir = project_root.join(".claude/commands");
        if !claude_commands_dir.is_dir() {
            return Ok(0);
        }

        let mut count = 0;

        if let Ok(entries) = std::fs::read_dir(&claude_commands_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "md") {
                    let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                    // Skip aisync-* managed files
                    if stem.starts_with("aisync-") {
                        continue;
                    }
                    let content = std::fs::read_to_string(&path)
                        .map_err(|e| InitError::ImportFailed(format!("read {}: {e}", path.display())))?;
                    let output = commands_dir.join(format!("{stem}.md"));
                    std::fs::write(&output, content)
                        .map_err(|e| InitError::ScaffoldFailed(e))?;
                    count += 1;
                }
            }
        }

        Ok(count)
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

/// Parse Cursor-format frontmatter (alwaysApply, globs as comma-separated, description)
/// into canonical RuleMetadata plus body content.
fn parse_cursor_rule(raw: &str) -> Result<(RuleMetadata, String), AisyncError> {
    let (yaml_str, body) = split_frontmatter(raw);

    let mut description = None;
    let mut globs = Vec::new();
    let mut always_apply = true;

    for line in yaml_str.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("description:") {
            let val = val.trim().trim_matches('"');
            if !val.is_empty() {
                description = Some(val.to_string());
            }
        } else if let Some(val) = line.strip_prefix("globs:") {
            let val = val.trim();
            let val = val.trim_matches('"');
            if val.starts_with('[') {
                let inner = val.trim_start_matches('[').trim_end_matches(']');
                globs = inner.split(',').map(|s| s.trim().trim_matches('"').to_string()).filter(|s| !s.is_empty()).collect();
            } else {
                globs = val.split(',').map(|s| s.trim().trim_matches('"').to_string()).filter(|s| !s.is_empty()).collect();
            }
        } else if let Some(val) = line.strip_prefix("alwaysApply:") {
            always_apply = val.trim().parse().unwrap_or(true);
        }
    }

    Ok((RuleMetadata { description, globs, always_apply }, body))
}

/// Parse Windsurf-format frontmatter (trigger, globs, description)
/// into canonical RuleMetadata plus body content.
fn parse_windsurf_rule(raw: &str) -> Result<(RuleMetadata, String), AisyncError> {
    let (yaml_str, body) = split_frontmatter(raw);

    let mut description = None;
    let mut globs = Vec::new();
    let mut always_apply = false;
    let mut trigger = String::new();

    for line in yaml_str.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("description:") {
            let val = val.trim().trim_matches('"');
            if !val.is_empty() {
                description = Some(val.to_string());
            }
        } else if let Some(val) = line.strip_prefix("globs:") {
            let val = val.trim().trim_matches('"');
            if val.starts_with('[') {
                let inner = val.trim_start_matches('[').trim_end_matches(']');
                globs = inner.split(',').map(|s| s.trim().trim_matches('"').to_string()).filter(|s| !s.is_empty()).collect();
            } else {
                globs = val.split(',').map(|s| s.trim().trim_matches('"').to_string()).filter(|s| !s.is_empty()).collect();
            }
        } else if let Some(val) = line.strip_prefix("trigger:") {
            trigger = val.trim().to_string();
        }
    }

    // Translate trigger types to canonical always_apply
    match trigger.as_str() {
        "always_on" => always_apply = true,
        "glob" => always_apply = false,
        "model_decision" | "manual" | _ => always_apply = false,
    }

    Ok((RuleMetadata { description, globs, always_apply }, body))
}

/// Split raw file content into (frontmatter_yaml, body).
/// If no frontmatter present, returns empty string for yaml and full content as body.
fn split_frontmatter(raw: &str) -> (String, String) {
    if let Some(after_open) = raw.strip_prefix("---\n") {
        if let Some(rest) = after_open.strip_prefix("---") {
            return (String::new(), rest.trim_start_matches('\n').to_string());
        }
        if let Some(end_idx) = after_open.find("\n---") {
            let yaml_str = after_open[..end_idx].to_string();
            let after_close = &after_open[end_idx + 4..];
            let body = after_close.trim_start_matches('\n').to_string();
            return (yaml_str, body);
        }
    }
    (String::new(), raw.to_string())
}

/// Write a canonical .ai/rules/{name}.md file with YAML frontmatter.
fn write_canonical_rule(path: &Path, metadata: &RuleMetadata, content: &str) -> Result<(), AisyncError> {
    let mut output = String::from("---\n");
    if let Some(ref desc) = metadata.description {
        output.push_str(&format!("description: \"{desc}\"\n"));
    }
    if !metadata.globs.is_empty() {
        let globs_str: Vec<String> = metadata.globs.iter().map(|g| format!("\"{g}\"")).collect();
        output.push_str(&format!("globs: [{}]\n", globs_str.join(", ")));
    }
    output.push_str(&format!("always_apply: {}\n", metadata.always_apply));
    output.push_str("---\n");
    if !content.is_empty() {
        output.push_str(content);
        if !content.ends_with('\n') {
            output.push('\n');
        }
    }

    std::fs::write(path, output)
        .map_err(|e| InitError::ScaffoldFailed(e))?;
    Ok(())
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

    // --- import_rules tests ---

    #[test]
    fn test_import_rules_from_cursor_mdc() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".ai/rules")).unwrap();
        let cursor_rules = dir.path().join(".cursor/rules");
        std::fs::create_dir_all(&cursor_rules).unwrap();

        let mdc = "---\ndescription: Rust coding standards\nglobs: \"*.rs, *.toml\"\nalwaysApply: false\n---\n\nUse snake_case for variables.";
        std::fs::write(cursor_rules.join("rust-rules.mdc"), mdc).unwrap();

        let count = InitEngine::import_rules(dir.path()).unwrap();
        assert_eq!(count, 1);

        let canonical = std::fs::read_to_string(dir.path().join(".ai/rules/rust-rules.md")).unwrap();
        assert!(canonical.contains("description: \"Rust coding standards\""));
        assert!(canonical.contains("globs: [\"*.rs\", \"*.toml\"]"));
        assert!(canonical.contains("always_apply: false"));
        assert!(canonical.contains("Use snake_case for variables."));
    }

    #[test]
    fn test_import_rules_skips_project_mdc() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".ai/rules")).unwrap();
        let cursor_rules = dir.path().join(".cursor/rules");
        std::fs::create_dir_all(&cursor_rules).unwrap();

        let mdc = "---\ndescription: Project\nalwaysApply: true\n---\n\nProject instructions";
        std::fs::write(cursor_rules.join("project.mdc"), mdc).unwrap();

        let count = InitEngine::import_rules(dir.path()).unwrap();
        assert_eq!(count, 0);
        assert!(!dir.path().join(".ai/rules/project.md").exists());
    }

    #[test]
    fn test_import_rules_skips_aisync_prefixed() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".ai/rules")).unwrap();
        let cursor_rules = dir.path().join(".cursor/rules");
        std::fs::create_dir_all(&cursor_rules).unwrap();

        let mdc = "---\ndescription: managed\nalwaysApply: true\n---\nManaged content";
        std::fs::write(cursor_rules.join("aisync-managed.mdc"), mdc).unwrap();

        let count = InitEngine::import_rules(dir.path()).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_import_rules_from_windsurf_md() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".ai/rules")).unwrap();
        let windsurf_rules = dir.path().join(".windsurf/rules");
        std::fs::create_dir_all(&windsurf_rules).unwrap();

        let md = "---\ntrigger: always_on\ndescription: Always active rule\n---\n\nAlways do this.";
        std::fs::write(windsurf_rules.join("always-rule.md"), md).unwrap();

        let count = InitEngine::import_rules(dir.path()).unwrap();
        assert_eq!(count, 1);

        let canonical = std::fs::read_to_string(dir.path().join(".ai/rules/always-rule.md")).unwrap();
        assert!(canonical.contains("always_apply: true"));
        assert!(canonical.contains("Always do this."));
    }

    #[test]
    fn test_import_rules_windsurf_glob_trigger() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".ai/rules")).unwrap();
        let windsurf_rules = dir.path().join(".windsurf/rules");
        std::fs::create_dir_all(&windsurf_rules).unwrap();

        let md = "---\ntrigger: glob\nglobs: \"*.rs, *.toml\"\ndescription: Rust files\n---\n\nRust rule body.";
        std::fs::write(windsurf_rules.join("rust-glob.md"), md).unwrap();

        let count = InitEngine::import_rules(dir.path()).unwrap();
        assert_eq!(count, 1);

        let canonical = std::fs::read_to_string(dir.path().join(".ai/rules/rust-glob.md")).unwrap();
        assert!(canonical.contains("always_apply: false"));
        assert!(canonical.contains("globs: [\"*.rs\", \"*.toml\"]"));
    }

    #[test]
    fn test_import_rules_windsurf_model_decision_trigger() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".ai/rules")).unwrap();
        let windsurf_rules = dir.path().join(".windsurf/rules");
        std::fs::create_dir_all(&windsurf_rules).unwrap();

        let md = "---\ntrigger: model_decision\ndescription: Model decides\n---\n\nModel body.";
        std::fs::write(windsurf_rules.join("model-rule.md"), md).unwrap();

        let count = InitEngine::import_rules(dir.path()).unwrap();
        assert_eq!(count, 1);

        let canonical = std::fs::read_to_string(dir.path().join(".ai/rules/model-rule.md")).unwrap();
        assert!(canonical.contains("always_apply: false"));
    }

    #[test]
    fn test_import_rules_empty_directories() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".ai/rules")).unwrap();
        // No .cursor/rules or .windsurf/rules directories

        let count = InitEngine::import_rules(dir.path()).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_import_rules_mixed_sources() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".ai/rules")).unwrap();

        let cursor_rules = dir.path().join(".cursor/rules");
        std::fs::create_dir_all(&cursor_rules).unwrap();
        let mdc = "---\ndescription: Cursor rule\nalwaysApply: true\n---\nCursor content";
        std::fs::write(cursor_rules.join("cursor-rule.mdc"), mdc).unwrap();

        let windsurf_rules = dir.path().join(".windsurf/rules");
        std::fs::create_dir_all(&windsurf_rules).unwrap();
        let md = "---\ntrigger: always_on\ndescription: Windsurf rule\n---\nWindsurf content";
        std::fs::write(windsurf_rules.join("windsurf-rule.md"), md).unwrap();

        let count = InitEngine::import_rules(dir.path()).unwrap();
        assert_eq!(count, 2);
        assert!(dir.path().join(".ai/rules/cursor-rule.md").exists());
        assert!(dir.path().join(".ai/rules/windsurf-rule.md").exists());
    }

    #[test]
    fn test_import_rules_creates_rules_dir_if_missing() {
        let dir = TempDir::new().unwrap();
        // No .ai/rules directory yet
        let cursor_rules = dir.path().join(".cursor/rules");
        std::fs::create_dir_all(&cursor_rules).unwrap();

        let mdc = "---\ndescription: A rule\nalwaysApply: true\n---\nContent";
        std::fs::write(cursor_rules.join("my-rule.mdc"), mdc).unwrap();

        let count = InitEngine::import_rules(dir.path()).unwrap();
        assert_eq!(count, 1);
        assert!(dir.path().join(".ai/rules/my-rule.md").exists());
    }

    #[test]
    fn test_import_rules_windsurf_skips_project_md() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".ai/rules")).unwrap();
        let windsurf_rules = dir.path().join(".windsurf/rules");
        std::fs::create_dir_all(&windsurf_rules).unwrap();

        let md = "---\ntrigger: always_on\ndescription: Project\n---\nProject instructions";
        std::fs::write(windsurf_rules.join("project.md"), md).unwrap();

        let count = InitEngine::import_rules(dir.path()).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_import_rules_canonical_output_format() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".ai/rules")).unwrap();
        let cursor_rules = dir.path().join(".cursor/rules");
        std::fs::create_dir_all(&cursor_rules).unwrap();

        let mdc = "---\ndescription: Detailed rule\nglobs: \"src/**/*.rs\"\nalwaysApply: false\n---\n\n# Rule Body\n\nDetailed content here.";
        std::fs::write(cursor_rules.join("detailed.mdc"), mdc).unwrap();

        InitEngine::import_rules(dir.path()).unwrap();

        let canonical = std::fs::read_to_string(dir.path().join(".ai/rules/detailed.md")).unwrap();
        // Verify it starts with frontmatter
        assert!(canonical.starts_with("---\n"));
        // Verify frontmatter closes
        let parts: Vec<&str> = canonical.splitn(3, "---").collect();
        assert!(parts.len() >= 3, "should have opening and closing ---");
        // Verify body after frontmatter
        assert!(canonical.contains("# Rule Body"));
        assert!(canonical.contains("Detailed content here."));
    }

    // --- import_commands tests ---

    #[test]
    fn test_import_commands_from_claude() {
        let dir = TempDir::new().unwrap();
        let claude_commands = dir.path().join(".claude/commands");
        std::fs::create_dir_all(&claude_commands).unwrap();
        std::fs::write(claude_commands.join("review.md"), "Review this code carefully.").unwrap();

        let count = InitEngine::import_commands(dir.path()).unwrap();
        assert_eq!(count, 1);

        let imported = std::fs::read_to_string(dir.path().join(".ai/commands/review.md")).unwrap();
        assert_eq!(imported, "Review this code carefully.");
    }

    #[test]
    fn test_import_commands_skips_aisync_prefixed() {
        let dir = TempDir::new().unwrap();
        let claude_commands = dir.path().join(".claude/commands");
        std::fs::create_dir_all(&claude_commands).unwrap();
        std::fs::write(claude_commands.join("aisync-build.md"), "Managed command").unwrap();
        std::fs::write(claude_commands.join("deploy.md"), "Deploy command").unwrap();

        let count = InitEngine::import_commands(dir.path()).unwrap();
        assert_eq!(count, 1);

        assert!(!dir.path().join(".ai/commands/aisync-build.md").exists());
        assert!(dir.path().join(".ai/commands/deploy.md").exists());
    }

    #[test]
    fn test_import_commands_empty_directory() {
        let dir = TempDir::new().unwrap();
        // No .claude/commands directory at all
        let count = InitEngine::import_commands(dir.path()).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_import_commands_creates_dir_if_missing() {
        let dir = TempDir::new().unwrap();
        let claude_commands = dir.path().join(".claude/commands");
        std::fs::create_dir_all(&claude_commands).unwrap();
        std::fs::write(claude_commands.join("test.md"), "Test command").unwrap();

        // .ai/commands/ does not exist yet
        assert!(!dir.path().join(".ai/commands").is_dir());

        let count = InitEngine::import_commands(dir.path()).unwrap();
        assert_eq!(count, 1);
        assert!(dir.path().join(".ai/commands").is_dir());
        assert!(dir.path().join(".ai/commands/test.md").exists());
    }

    #[test]
    fn test_import_commands_skips_non_md_files() {
        let dir = TempDir::new().unwrap();
        let claude_commands = dir.path().join(".claude/commands");
        std::fs::create_dir_all(&claude_commands).unwrap();
        std::fs::write(claude_commands.join("readme.txt"), "Not a command").unwrap();
        std::fs::write(claude_commands.join("script.sh"), "#!/bin/bash").unwrap();
        std::fs::write(claude_commands.join("valid.md"), "A command").unwrap();

        let count = InitEngine::import_commands(dir.path()).unwrap();
        assert_eq!(count, 1);
        assert!(dir.path().join(".ai/commands/valid.md").exists());
        assert!(!dir.path().join(".ai/commands/readme.txt").exists());
    }

    #[test]
    fn test_scaffold_calls_import_commands() {
        let dir = TempDir::new().unwrap();
        let claude_commands = dir.path().join(".claude/commands");
        std::fs::create_dir_all(&claude_commands).unwrap();
        std::fs::write(claude_commands.join("auto-cmd.md"), "Auto imported command").unwrap();

        let options = InitOptions {
            force: false,
            import_from: None,
        };

        InitEngine::scaffold(dir.path(), &[], None, &options).unwrap();

        // scaffold should have called import_commands automatically
        assert!(
            dir.path().join(".ai/commands/auto-cmd.md").exists(),
            "scaffold should automatically import commands"
        );
        let content = std::fs::read_to_string(dir.path().join(".ai/commands/auto-cmd.md")).unwrap();
        assert_eq!(content, "Auto imported command");
    }

    #[test]
    fn test_scaffold_calls_import_rules() {
        let dir = TempDir::new().unwrap();
        let cursor_rules = dir.path().join(".cursor/rules");
        std::fs::create_dir_all(&cursor_rules).unwrap();

        let mdc = "---\ndescription: Auto-imported\nalwaysApply: true\n---\nAuto content";
        std::fs::write(cursor_rules.join("auto-rule.mdc"), mdc).unwrap();

        let options = InitOptions {
            force: false,
            import_from: None,
        };

        InitEngine::scaffold(dir.path(), &[], None, &options).unwrap();

        // scaffold should have called import_rules automatically
        assert!(
            dir.path().join(".ai/rules/auto-rule.md").exists(),
            "scaffold should automatically import rules"
        );
    }
}
