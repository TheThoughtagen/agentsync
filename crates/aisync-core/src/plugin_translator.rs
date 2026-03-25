use std::collections::BTreeMap;
use std::path::Path;

use crate::adapters::cursor::{event_name_from_cursor, translate_matcher_from_cursor};
use crate::error::{AisyncError, ConfigError};
use crate::hooks::HookEngine;
use crate::mcp::McpEngine;
use crate::types::{
    CanonicalPluginManifest, ComponentKind, ExportReport, HookGroup, HookHandler, HooksConfig,
    ImportReport, PluginComponents, PluginMetadata, ToolKind,
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
        source_path: &Path,
        source_tool: Option<ToolKind>,
    ) -> Result<ImportReport, AisyncError> {
        let tool = match source_tool {
            Some(t) => t,
            None => detect_source_tool(source_path)?,
        };

        match tool {
            ToolKind::ClaudeCode => Self::import_claude_code(source_path),
            ToolKind::Cursor => Self::import_cursor(source_path),
            ToolKind::OpenCode => Self::import_opencode(source_path),
            _ => Err(AisyncError::Config(ConfigError::ReadFile(
                std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    format!("import not supported for tool: {}", tool),
                ),
            ))),
        }
    }

    // ---------------------------------------------------------------
    // Claude Code import
    // ---------------------------------------------------------------

    fn import_claude_code(source_path: &Path) -> Result<ImportReport, AisyncError> {
        let io_err = |e| AisyncError::Config(ConfigError::ReadFile(e));
        let mut components_imported = Vec::new();
        let components_skipped = Vec::new();
        let mut components = PluginComponents::default();

        // 1. Read plugin.json for metadata
        let plugin_json_path = source_path.join(".claude-plugin/plugin.json");
        let plugin_json_raw = std::fs::read_to_string(&plugin_json_path).map_err(io_err)?;
        let plugin_json: serde_json::Value = serde_json::from_str(&plugin_json_raw)
            .map_err(|e| {
                AisyncError::Config(ConfigError::ReadFile(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid plugin.json: {e}"),
                )))
            })?;

        let name = plugin_json
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                source_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
            })
            .to_string();
        let version = plugin_json
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let description = plugin_json
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // 2. Copy commands/*.md
        let commands_src = source_path.join("commands");
        if commands_src.is_dir() {
            let commands_dst = source_path.join("commands");
            // Commands are already in place for Claude Code plugins -- just mark as imported
            if let Ok(entries) = std::fs::read_dir(&commands_src) {
                let has_any = entries
                    .filter_map(|e| e.ok())
                    .any(|e| {
                        e.path()
                            .extension()
                            .map_or(false, |ext| ext == "md")
                    });
                if has_any {
                    // If source != output (which it is here), we'd copy.
                    // Since source IS the output dir, commands are already in place.
                    components_imported.push(ComponentKind::Commands);
                    components.has_commands = true;
                }
            }
            let _ = commands_dst; // suppress unused warning
        }

        // 3. Copy skills/*/SKILL.md
        let skills_src = source_path.join("skills");
        if skills_src.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&skills_src) {
                let has_any = entries
                    .filter_map(|e| e.ok())
                    .any(|e| e.path().is_dir() && e.path().join("SKILL.md").exists());
                if has_any {
                    components_imported.push(ComponentKind::Skills);
                    components.has_skills = true;
                }
            }
        }

        // 4. Copy agents/*.md
        let agents_src = source_path.join("agents");
        if agents_src.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&agents_src) {
                let has_any = entries
                    .filter_map(|e| e.ok())
                    .any(|e| {
                        e.path()
                            .extension()
                            .map_or(false, |ext| ext == "md")
                    });
                if has_any {
                    components_imported.push(ComponentKind::Agents);
                    components.has_agents = true;
                }
            }
        }

        // 5. Convert hooks/hooks.json -> hooks.toml
        let hooks_json_path = source_path.join("hooks/hooks.json");
        if hooks_json_path.exists() {
            let hooks_raw = std::fs::read_to_string(&hooks_json_path).map_err(io_err)?;
            let hooks_json: serde_json::Value =
                serde_json::from_str(&hooks_raw).map_err(|e| {
                    AisyncError::Config(ConfigError::ReadFile(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("invalid hooks.json: {e}"),
                    )))
                })?;

            let hooks_config = Self::parse_claude_hooks_json(&hooks_json)?;
            if !hooks_config.events.is_empty() {
                let toml_str = HookEngine::serialize(&hooks_config)?;
                std::fs::write(source_path.join("hooks.toml"), toml_str).map_err(io_err)?;
                components_imported.push(ComponentKind::Hooks);
                components.has_hooks = true;
            }
        }

        // 6. Convert .mcp.json -> mcp.toml
        let mcp_json_path = source_path.join(".mcp.json");
        if mcp_json_path.exists() {
            let mcp_config = McpEngine::parse_mcp_json(&mcp_json_path)?;
            if !mcp_config.servers.is_empty() {
                let toml_str = toml::to_string_pretty(&mcp_config).map_err(|e| {
                    AisyncError::Config(ConfigError::Parse(toml::de::Error::custom(e.to_string())))
                })?;
                std::fs::write(source_path.join("mcp.toml"), toml_str).map_err(io_err)?;
                components_imported.push(ComponentKind::Mcp);
                components.has_mcp = true;
            }
        }

        // 7-8. Write plugin.toml manifest
        let manifest = CanonicalPluginManifest {
            metadata: PluginMetadata {
                name: name.clone(),
                version,
                description,
                source_tool: Some("claude-code".to_string()),
            },
            components,
        };
        Self::save_manifest(source_path, &manifest)?;

        Ok(ImportReport {
            name,
            source_tool: ToolKind::ClaudeCode,
            components_imported,
            components_skipped,
        })
    }

    /// Parse Claude Code plugin hooks.json into canonical HooksConfig.
    ///
    /// Format: `{"description": "...", "hooks": {"EventName": [...]}}`
    /// Each event value is an array of hook group objects with optional `matcher`
    /// and a `command` + optional `timeout`.
    fn parse_claude_hooks_json(json: &serde_json::Value) -> Result<HooksConfig, AisyncError> {
        let mut events = BTreeMap::new();

        let hooks_obj = json
            .get("hooks")
            .and_then(|v| v.as_object())
            .unwrap_or(&serde_json::Map::new())
            .clone();

        for (event_name, groups_val) in &hooks_obj {
            let groups_arr = match groups_val.as_array() {
                Some(arr) => arr,
                None => continue,
            };

            let mut hook_groups = Vec::new();
            for group_val in groups_arr {
                let matcher = group_val
                    .get("matcher")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // Each group can have a "hooks" array or be a single hook
                let handlers = if let Some(hooks_arr) = group_val.get("hooks").and_then(|v| v.as_array()) {
                    hooks_arr
                        .iter()
                        .filter_map(|h| {
                            let command = h.get("command").and_then(|v| v.as_str())?;
                            let timeout = h.get("timeout").and_then(|v| v.as_u64());
                            let hook_type = h
                                .get("type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("command")
                                .to_string();
                            Some(HookHandler {
                                hook_type,
                                command: command.to_string(),
                                timeout,
                            })
                        })
                        .collect()
                } else if let Some(command) = group_val.get("command").and_then(|v| v.as_str()) {
                    let timeout = group_val.get("timeout").and_then(|v| v.as_u64());
                    vec![HookHandler {
                        hook_type: "command".to_string(),
                        command: command.to_string(),
                        timeout,
                    }]
                } else {
                    continue;
                };

                if !handlers.is_empty() {
                    hook_groups.push(HookGroup {
                        matcher,
                        hooks: handlers,
                    });
                }
            }

            if !hook_groups.is_empty() {
                events.insert(event_name.clone(), hook_groups);
            }
        }

        Ok(HooksConfig { events })
    }

    // ---------------------------------------------------------------
    // Cursor import
    // ---------------------------------------------------------------

    fn import_cursor(source_path: &Path) -> Result<ImportReport, AisyncError> {
        let io_err = |e| AisyncError::Config(ConfigError::ReadFile(e));
        let mut components_imported = Vec::new();
        let mut components_skipped = Vec::new();
        let mut components = PluginComponents::default();

        let name = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // 1. Convert .cursor/rules/*.mdc -> rules/*.md
        let rules_src = source_path.join(".cursor/rules");
        if rules_src.is_dir() {
            let rules_dst = source_path.join("rules");
            std::fs::create_dir_all(&rules_dst).map_err(io_err)?;

            let mut found_rules = false;
            if let Ok(entries) = std::fs::read_dir(&rules_src) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.extension().map_or(false, |ext| ext == "mdc") {
                        let content = std::fs::read_to_string(&path).map_err(io_err)?;
                        let (frontmatter, body) = Self::parse_mdc_frontmatter(&content);
                        let canonical_content = Self::build_canonical_rule_content(&frontmatter, &body);

                        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                        let out_path = rules_dst.join(format!("{stem}.md"));
                        std::fs::write(&out_path, canonical_content).map_err(io_err)?;
                        found_rules = true;
                    }
                }
            }
            if found_rules {
                components_imported.push(ComponentKind::Rules);
                components.has_rules = true;
            }
        }

        // 2. Convert .cursor/hooks.json -> hooks.toml
        let hooks_json_path = source_path.join(".cursor/hooks.json");
        if hooks_json_path.exists() {
            let hooks_raw = std::fs::read_to_string(&hooks_json_path).map_err(io_err)?;
            let hooks_json: serde_json::Value =
                serde_json::from_str(&hooks_raw).map_err(|e| {
                    AisyncError::Config(ConfigError::ReadFile(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("invalid hooks.json: {e}"),
                    )))
                })?;

            let hooks_config = Self::parse_cursor_hooks_json(&hooks_json)?;
            if !hooks_config.events.is_empty() {
                let toml_str = HookEngine::serialize(&hooks_config)?;
                std::fs::write(source_path.join("hooks.toml"), toml_str).map_err(io_err)?;
                components_imported.push(ComponentKind::Hooks);
                components.has_hooks = true;
            }
        }

        // 3. Convert .cursor/mcp.json -> mcp.toml
        let mcp_json_path = source_path.join(".cursor/mcp.json");
        if mcp_json_path.exists() {
            let mut mcp_config = McpEngine::parse_mcp_json(&mcp_json_path)?;
            // Translate Cursor env var format to canonical
            for server in mcp_config.servers.values_mut() {
                let keys: Vec<String> = server.env.keys().cloned().collect();
                for key in keys {
                    if let Some(val) = server.env.get(&key).cloned() {
                        server
                            .env
                            .insert(key, McpEngine::env_from_cursor(&val));
                    }
                }
            }
            if !mcp_config.servers.is_empty() {
                let toml_str = toml::to_string_pretty(&mcp_config).map_err(|e| {
                    AisyncError::Config(ConfigError::Parse(toml::de::Error::custom(e.to_string())))
                })?;
                std::fs::write(source_path.join("mcp.toml"), toml_str).map_err(io_err)?;
                components_imported.push(ComponentKind::Mcp);
                components.has_mcp = true;
            }
        }

        // 4. Components with no Cursor equivalent
        components_skipped.push((ComponentKind::Commands, "no cursor equivalent".to_string()));
        components_skipped.push((ComponentKind::Skills, "no cursor equivalent".to_string()));
        components_skipped.push((ComponentKind::Agents, "no cursor equivalent".to_string()));

        // Write plugin.toml
        let manifest = CanonicalPluginManifest {
            metadata: PluginMetadata {
                name: name.clone(),
                version: None,
                description: None,
                source_tool: Some("cursor".to_string()),
            },
            components,
        };
        Self::save_manifest(source_path, &manifest)?;

        Ok(ImportReport {
            name,
            source_tool: ToolKind::Cursor,
            components_imported,
            components_skipped,
        })
    }

    /// Parse MDC YAML frontmatter from a Cursor rule file.
    ///
    /// Returns (frontmatter fields as map, body markdown).
    fn parse_mdc_frontmatter(content: &str) -> (BTreeMap<String, String>, String) {
        let mut fields = BTreeMap::new();
        let trimmed = content.trim_start();

        if !trimmed.starts_with("---") {
            return (fields, content.to_string());
        }

        // Find the closing ---
        let after_first = &trimmed[3..];
        if let Some(end_idx) = after_first.find("\n---") {
            let frontmatter_block = &after_first[..end_idx];
            let body_start = end_idx + 4; // skip "\n---"
            let body = after_first[body_start..].trim_start_matches('\n').to_string();

            // Parse simple YAML key: value lines
            for line in frontmatter_block.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Some((key, value)) = line.split_once(':') {
                    let key = key.trim().to_string();
                    let value = value.trim().trim_matches('"').to_string();
                    fields.insert(key, value);
                }
            }

            (fields, body)
        } else {
            (fields, content.to_string())
        }
    }

    /// Build canonical rule content with YAML frontmatter from MDC fields.
    fn build_canonical_rule_content(fields: &BTreeMap<String, String>, body: &str) -> String {
        let mut out = String::from("---\n");
        if let Some(desc) = fields.get("description") {
            out.push_str(&format!("description: {desc}\n"));
        }
        if let Some(globs) = fields.get("globs") {
            out.push_str(&format!("globs: \"{globs}\"\n"));
        }
        if let Some(always) = fields.get("alwaysApply") {
            out.push_str(&format!("always_apply: {always}\n"));
        }
        out.push_str("---\n\n");
        out.push_str(body);
        out
    }

    /// Parse Cursor hooks.json into canonical HooksConfig.
    ///
    /// Cursor format: `{"hooks": {"eventName": [{"command": "...", "matcher": "...", ...}]}}`
    /// or the simpler: `{"eventName": [...]}`
    fn parse_cursor_hooks_json(json: &serde_json::Value) -> Result<HooksConfig, AisyncError> {
        let mut events = BTreeMap::new();

        // Try wrapped format first, then flat
        let hooks_obj = json
            .get("hooks")
            .and_then(|v| v.as_object())
            .or_else(|| json.as_object())
            .unwrap_or(&serde_json::Map::new())
            .clone();

        for (cursor_event, groups_val) in &hooks_obj {
            // Skip non-event keys like "description"
            if cursor_event == "description" || cursor_event == "version" {
                continue;
            }

            let canonical_event = event_name_from_cursor(cursor_event);

            let groups_arr = match groups_val.as_array() {
                Some(arr) => arr,
                None => continue,
            };

            let mut hook_groups = Vec::new();
            for group_val in groups_arr {
                let matcher = group_val
                    .get("matcher")
                    .and_then(|v| v.as_str())
                    .map(|s| translate_matcher_from_cursor(s));

                let handlers = if let Some(hooks_arr) =
                    group_val.get("hooks").and_then(|v| v.as_array())
                {
                    hooks_arr
                        .iter()
                        .filter_map(|h| {
                            let command = h.get("command").and_then(|v| v.as_str())?;
                            let timeout = h.get("timeout").and_then(|v| v.as_u64());
                            Some(HookHandler {
                                hook_type: "command".to_string(),
                                command: command.to_string(),
                                timeout,
                            })
                        })
                        .collect()
                } else if let Some(command) = group_val.get("command").and_then(|v| v.as_str()) {
                    let timeout = group_val.get("timeout").and_then(|v| v.as_u64());
                    vec![HookHandler {
                        hook_type: "command".to_string(),
                        command: command.to_string(),
                        timeout,
                    }]
                } else {
                    continue;
                };

                if !handlers.is_empty() {
                    hook_groups.push(HookGroup {
                        matcher,
                        hooks: handlers,
                    });
                }
            }

            if !hook_groups.is_empty() {
                events.insert(canonical_event, hook_groups);
            }
        }

        Ok(HooksConfig { events })
    }

    // ---------------------------------------------------------------
    // OpenCode import
    // ---------------------------------------------------------------

    fn import_opencode(source_path: &Path) -> Result<ImportReport, AisyncError> {
        let io_err = |e| AisyncError::Config(ConfigError::ReadFile(e));
        let mut components_imported = Vec::new();
        let mut components_skipped = Vec::new();
        let mut components = PluginComponents::default();

        let name = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // 1. Copy AGENTS.md -> instructions.md
        let agents_md = source_path.join("AGENTS.md");
        if agents_md.exists() {
            let content = std::fs::read_to_string(&agents_md).map_err(io_err)?;
            std::fs::write(source_path.join("instructions.md"), content).map_err(io_err)?;
            components_imported.push(ComponentKind::Instructions);
            components.has_instructions = true;
        }

        // 2-3. Components not round-trippable or not applicable
        components_skipped.push((
            ComponentKind::Hooks,
            "OpenCode JS stubs are not round-trippable".to_string(),
        ));
        components_skipped.push((ComponentKind::Commands, "no opencode equivalent".to_string()));
        components_skipped.push((ComponentKind::Skills, "no opencode equivalent".to_string()));
        components_skipped.push((ComponentKind::Agents, "no opencode equivalent".to_string()));
        components_skipped.push((ComponentKind::Mcp, "no opencode equivalent".to_string()));
        components_skipped.push((ComponentKind::Rules, "no opencode equivalent".to_string()));

        // Write plugin.toml
        let manifest = CanonicalPluginManifest {
            metadata: PluginMetadata {
                name: name.clone(),
                version: None,
                description: None,
                source_tool: Some("opencode".to_string()),
            },
            components,
        };
        Self::save_manifest(source_path, &manifest)?;

        Ok(ImportReport {
            name,
            source_tool: ToolKind::OpenCode,
            components_imported,
            components_skipped,
        })
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

    // --- Claude Code import tests ---

    /// Helper: create a minimal Claude Code plugin fixture.
    fn create_claude_code_fixture(dir: &Path) {
        // plugin.json
        std::fs::create_dir_all(dir.join(".claude-plugin")).unwrap();
        std::fs::write(
            dir.join(".claude-plugin/plugin.json"),
            r#"{"name": "my-cc-plugin", "version": "1.0.0", "description": "A test plugin"}"#,
        )
        .unwrap();

        // commands
        std::fs::create_dir_all(dir.join("commands")).unwrap();
        std::fs::write(dir.join("commands/build.md"), "# Build\nRun the build.").unwrap();

        // skills
        std::fs::create_dir_all(dir.join("skills/deploy")).unwrap();
        std::fs::write(dir.join("skills/deploy/SKILL.md"), "# Deploy Skill").unwrap();

        // agents
        std::fs::create_dir_all(dir.join("agents")).unwrap();
        std::fs::write(dir.join("agents/reviewer.md"), "# Reviewer Agent").unwrap();

        // hooks
        std::fs::create_dir_all(dir.join("hooks")).unwrap();
        std::fs::write(
            dir.join("hooks/hooks.json"),
            r#"{
                "description": "Plugin hooks",
                "hooks": {
                    "PreToolUse": [
                        {
                            "matcher": "Edit",
                            "hooks": [
                                {"type": "command", "command": "npm run lint", "timeout": 10000}
                            ]
                        }
                    ],
                    "PostToolUse": [
                        {
                            "command": "cargo fmt"
                        }
                    ]
                }
            }"#,
        )
        .unwrap();

        // MCP
        std::fs::write(
            dir.join(".mcp.json"),
            r#"{
                "mcpServers": {
                    "filesystem": {
                        "command": "npx",
                        "args": ["-y", "@mcp/server-fs"],
                        "env": {"HOME": "/home/user"}
                    }
                }
            }"#,
        )
        .unwrap();
    }

    #[test]
    fn test_import_claude_code_full() {
        let dir = TempDir::new().unwrap();
        create_claude_code_fixture(dir.path());

        let report = PluginTranslator::import(dir.path(), Some(ToolKind::ClaudeCode)).unwrap();

        assert_eq!(report.name, "my-cc-plugin");
        assert_eq!(report.source_tool, ToolKind::ClaudeCode);

        // All components should be imported
        assert!(
            report.components_imported.contains(&ComponentKind::Commands),
            "expected Commands in imported: {:?}",
            report.components_imported
        );
        assert!(
            report.components_imported.contains(&ComponentKind::Skills),
            "expected Skills in imported: {:?}",
            report.components_imported
        );
        assert!(
            report.components_imported.contains(&ComponentKind::Agents),
            "expected Agents in imported: {:?}",
            report.components_imported
        );
        assert!(
            report.components_imported.contains(&ComponentKind::Hooks),
            "expected Hooks in imported: {:?}",
            report.components_imported
        );
        assert!(
            report.components_imported.contains(&ComponentKind::Mcp),
            "expected Mcp in imported: {:?}",
            report.components_imported
        );

        // Verify hooks.toml was written
        let hooks_toml = dir.path().join("hooks.toml");
        assert!(hooks_toml.exists(), "hooks.toml should exist");
        let hooks_content = std::fs::read_to_string(&hooks_toml).unwrap();
        assert!(hooks_content.contains("PreToolUse"), "hooks.toml should contain PreToolUse");
        assert!(hooks_content.contains("npm run lint"), "hooks.toml should contain the lint command");

        // Verify mcp.toml was written
        let mcp_toml = dir.path().join("mcp.toml");
        assert!(mcp_toml.exists(), "mcp.toml should exist");
        let mcp_content = std::fs::read_to_string(&mcp_toml).unwrap();
        assert!(mcp_content.contains("filesystem"), "mcp.toml should contain filesystem server");

        // Verify plugin.toml was written
        let manifest = PluginTranslator::load_manifest(dir.path()).unwrap();
        assert_eq!(manifest.metadata.name, "my-cc-plugin");
        assert_eq!(manifest.metadata.version, Some("1.0.0".to_string()));
        assert_eq!(manifest.metadata.source_tool, Some("claude-code".to_string()));
        assert!(manifest.components.has_hooks);
        assert!(manifest.components.has_mcp);
        assert!(manifest.components.has_commands);
        assert!(manifest.components.has_skills);
        assert!(manifest.components.has_agents);
    }

    #[test]
    fn test_import_claude_code_hooks_json_parsing() {
        let json: serde_json::Value = serde_json::from_str(r#"{
            "description": "test hooks",
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Edit",
                        "hooks": [
                            {"type": "command", "command": "lint", "timeout": 5000}
                        ]
                    }
                ],
                "Stop": [
                    {"command": "cleanup"}
                ]
            }
        }"#).unwrap();

        let config = PluginTranslator::parse_claude_hooks_json(&json).unwrap();
        assert!(config.events.contains_key("PreToolUse"));
        assert!(config.events.contains_key("Stop"));
        assert_eq!(config.events["PreToolUse"][0].matcher, Some("Edit".to_string()));
        assert_eq!(config.events["PreToolUse"][0].hooks[0].command, "lint");
        assert_eq!(config.events["PreToolUse"][0].hooks[0].timeout, Some(5000));
        assert_eq!(config.events["Stop"][0].hooks[0].command, "cleanup");
    }

    // --- Cursor import tests ---

    fn create_cursor_fixture(dir: &Path) {
        std::fs::create_dir_all(dir.join(".cursor/rules")).unwrap();

        // Rule file with MDC frontmatter
        std::fs::write(
            dir.join(".cursor/rules/coding-style.mdc"),
            "---\ndescription: Coding style rules\nglobs: \"**/*.ts\"\nalwaysApply: true\n---\n\n# Coding Style\nUse 2-space indentation.",
        )
        .unwrap();

        // Hooks
        std::fs::write(
            dir.join(".cursor/hooks.json"),
            r#"{
                "hooks": {
                    "preToolUse": [
                        {
                            "matcher": "Write",
                            "hooks": [
                                {"command": "eslint --fix", "timeout": 8000}
                            ]
                        }
                    ]
                }
            }"#,
        )
        .unwrap();

        // MCP
        std::fs::write(
            dir.join(".cursor/mcp.json"),
            r#"{
                "mcpServers": {
                    "github": {
                        "command": "npx",
                        "args": ["-y", "@mcp/server-github"],
                        "env": {"TOKEN": "${env:GITHUB_TOKEN}"}
                    }
                }
            }"#,
        )
        .unwrap();
    }

    #[test]
    fn test_import_cursor_full() {
        let dir = TempDir::new().unwrap();
        create_cursor_fixture(dir.path());

        let report = PluginTranslator::import(dir.path(), Some(ToolKind::Cursor)).unwrap();

        assert_eq!(report.source_tool, ToolKind::Cursor);

        // Rules, hooks, and MCP should be imported
        assert!(
            report.components_imported.contains(&ComponentKind::Rules),
            "expected Rules imported: {:?}",
            report.components_imported
        );
        assert!(
            report.components_imported.contains(&ComponentKind::Hooks),
            "expected Hooks imported: {:?}",
            report.components_imported
        );
        assert!(
            report.components_imported.contains(&ComponentKind::Mcp),
            "expected Mcp imported: {:?}",
            report.components_imported
        );

        // Commands, skills, agents should be skipped
        let skipped_kinds: Vec<_> = report.components_skipped.iter().map(|(k, _)| k.clone()).collect();
        assert!(skipped_kinds.contains(&ComponentKind::Commands));
        assert!(skipped_kinds.contains(&ComponentKind::Skills));
        assert!(skipped_kinds.contains(&ComponentKind::Agents));

        // Verify rules output
        let rule_file = dir.path().join("rules/coding-style.md");
        assert!(rule_file.exists(), "rule file should exist");
        let rule_content = std::fs::read_to_string(&rule_file).unwrap();
        assert!(rule_content.contains("description: Coding style rules"));
        assert!(rule_content.contains("# Coding Style"));

        // Verify hooks.toml with canonical event names and matcher translation
        let hooks_content = std::fs::read_to_string(dir.path().join("hooks.toml")).unwrap();
        assert!(
            hooks_content.contains("PreToolUse"),
            "should translate preToolUse to PreToolUse: {hooks_content}"
        );
        assert!(
            hooks_content.contains("Edit"),
            "should translate Write matcher to Edit: {hooks_content}"
        );

        // Verify mcp.toml with env var translation
        let mcp_content = std::fs::read_to_string(dir.path().join("mcp.toml")).unwrap();
        assert!(mcp_content.contains("github"));
        assert!(
            mcp_content.contains("${GITHUB_TOKEN}"),
            "should translate ${{env:GITHUB_TOKEN}} to ${{GITHUB_TOKEN}}: {mcp_content}"
        );
    }

    #[test]
    fn test_import_cursor_mdc_frontmatter_parsing() {
        let content = "---\ndescription: Test rule\nglobs: \"*.rs\"\nalwaysApply: false\n---\n\n# Rule Body\nSome content.";
        let (fields, body) = PluginTranslator::parse_mdc_frontmatter(content);

        assert_eq!(fields.get("description").unwrap(), "Test rule");
        assert_eq!(fields.get("globs").unwrap(), "*.rs");
        assert_eq!(fields.get("alwaysApply").unwrap(), "false");
        assert!(body.starts_with("# Rule Body"));
    }

    #[test]
    fn test_import_cursor_mdc_no_frontmatter() {
        let content = "# Just Markdown\nNo frontmatter here.";
        let (fields, body) = PluginTranslator::parse_mdc_frontmatter(content);

        assert!(fields.is_empty());
        assert_eq!(body, content);
    }

    // --- OpenCode import tests ---

    #[test]
    fn test_import_opencode_with_agents_md() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("opencode.json"), "{}").unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "# Agent Instructions\nDo the thing.").unwrap();

        let report = PluginTranslator::import(dir.path(), Some(ToolKind::OpenCode)).unwrap();

        assert_eq!(report.source_tool, ToolKind::OpenCode);
        assert!(report.components_imported.contains(&ComponentKind::Instructions));

        let instructions = std::fs::read_to_string(dir.path().join("instructions.md")).unwrap();
        assert_eq!(instructions, "# Agent Instructions\nDo the thing.");

        // Should have many skipped components
        assert!(
            report.components_skipped.len() >= 5,
            "expected at least 5 skipped components: {:?}",
            report.components_skipped
        );
    }

    #[test]
    fn test_import_opencode_skip_reporting() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("opencode.json"), "{}").unwrap();

        let report = PluginTranslator::import(dir.path(), Some(ToolKind::OpenCode)).unwrap();

        // Hooks should be skipped with specific reason
        let hooks_skip = report
            .components_skipped
            .iter()
            .find(|(k, _)| *k == ComponentKind::Hooks);
        assert!(hooks_skip.is_some(), "hooks should be in skipped list");
        assert!(
            hooks_skip.unwrap().1.contains("not round-trippable"),
            "hooks skip reason should mention not round-trippable: {}",
            hooks_skip.unwrap().1
        );
    }

    // --- Auto-detection integration tests ---

    #[test]
    fn test_import_auto_detects_claude_code() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude-plugin")).unwrap();
        std::fs::write(
            dir.path().join(".claude-plugin/plugin.json"),
            r#"{"name": "auto-detect-test"}"#,
        )
        .unwrap();

        let report = PluginTranslator::import(dir.path(), None).unwrap();
        assert_eq!(report.source_tool, ToolKind::ClaudeCode);
        assert_eq!(report.name, "auto-detect-test");
    }

    #[test]
    fn test_import_auto_detects_cursor() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".cursor")).unwrap();

        let report = PluginTranslator::import(dir.path(), None).unwrap();
        assert_eq!(report.source_tool, ToolKind::Cursor);
    }

    #[test]
    fn test_import_auto_detects_opencode() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("opencode.json"), "{}").unwrap();

        let report = PluginTranslator::import(dir.path(), None).unwrap();
        assert_eq!(report.source_tool, ToolKind::OpenCode);
    }

    #[test]
    fn test_import_auto_detect_fails_for_unknown() {
        let dir = TempDir::new().unwrap();
        let result = PluginTranslator::import(dir.path(), None);
        assert!(result.is_err());
    }
}
