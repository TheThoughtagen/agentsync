use std::path::Path;

use crate::adapter::{AnyAdapter, ClaudeCodeAdapter, CursorAdapter, OpenCodeAdapter, ToolAdapter};
use crate::conditional::ConditionalProcessor;
use crate::config::AisyncConfig;
use crate::error::{AisyncError, SyncError};
use crate::hooks::HookEngine;
use crate::types::{
    DriftState, HookTranslation, StatusReport, SyncAction, SyncReport, ToolKind, ToolSyncResult,
    ToolSyncStatus, content_hash,
};

/// The sync engine orchestrates planning, executing, and checking status
/// of sync operations across all enabled tool adapters.
pub struct SyncEngine;

impl SyncEngine {
    /// Plan sync actions for all enabled tools. Does not modify the filesystem.
    /// Used by both `aisync sync` and `aisync sync --dry-run`.
    pub fn plan(config: &AisyncConfig, project_root: &Path) -> Result<SyncReport, AisyncError> {
        let canonical_path = project_root.join(".ai/instructions.md");
        let canonical_content = std::fs::read_to_string(&canonical_path).map_err(|_| {
            AisyncError::Sync(SyncError::CanonicalMissing {
                path: canonical_path.display().to_string(),
            })
        })?;

        // Scan for memory files
        let memory_files = crate::memory::MemoryEngine::list(project_root)?;

        let mut results = Vec::new();

        for (tool_kind, adapter, tool_config_opt) in Self::enabled_tools(config) {
            let strategy = tool_config_opt
                .map(|tc| tc.effective_sync_strategy(&config.defaults))
                .unwrap_or(config.defaults.sync_strategy);

            let mut actions = Vec::new();

            // Apply conditional processing for this tool
            let tool_content = ConditionalProcessor::process(&canonical_content, tool_kind);

            // Plan instruction sync
            match adapter.plan_sync(project_root, &tool_content, strategy) {
                Ok(instruction_actions) => {
                    actions.extend(instruction_actions);
                }
                Err(e) => {
                    results.push(ToolSyncResult {
                        tool: tool_kind,
                        actions: vec![],
                        error: Some(format!("{e}")),
                    });
                    continue;
                }
            }

            // Plan memory sync (if memory files exist)
            if !memory_files.is_empty() {
                match adapter.plan_memory_sync(project_root, &memory_files) {
                    Ok(memory_actions) => {
                        actions.extend(memory_actions);
                    }
                    Err(e) => {
                        // Memory sync errors are non-fatal; log but continue
                        actions.push(SyncAction::WarnUnsupportedHooks {
                            tool: tool_kind,
                            reason: format!("memory sync failed: {e}"),
                        });
                    }
                }
            }

            // Plan hook translation (if hooks.toml exists)
            if let Ok(hooks_config) = HookEngine::parse(project_root) {
                match adapter.translate_hooks(&hooks_config) {
                    Ok(HookTranslation::Supported { tool, content, .. }) => {
                        let path = match tool {
                            ToolKind::ClaudeCode => project_root.join(".claude/settings.json"),
                            ToolKind::OpenCode => {
                                project_root.join(".opencode/plugins/aisync-hooks.js")
                            }
                            _ => continue, // Should not happen for supported
                        };
                        actions.push(SyncAction::WriteHookTranslation {
                            path,
                            content,
                            tool,
                        });
                    }
                    Ok(HookTranslation::Unsupported { tool, reason }) => {
                        actions.push(SyncAction::WarnUnsupportedHooks { tool, reason });
                    }
                    Err(_) => {
                        // Hook translation error is non-fatal
                    }
                }
            }

            results.push(ToolSyncResult {
                tool: tool_kind,
                actions,
                error: None,
            });
        }

        Ok(SyncReport { results })
    }

    /// Execute a planned sync. Mutates the filesystem.
    /// Non-interactive mode: SkipExistingFile actions are recorded but not executed.
    /// After executing tool syncs, updates .gitignore with aisync-managed section.
    pub fn execute(report: &SyncReport, project_root: &Path) -> Result<SyncReport, AisyncError> {
        let mut executed_results = Vec::new();
        let mut gitignore_entries = Vec::new();

        for tool_result in &report.results {
            if tool_result.error.is_some() {
                // Preserve error from planning phase
                executed_results.push(tool_result.clone());
                continue;
            }

            let mut executed_actions = Vec::new();
            let mut tool_error = None;

            for action in &tool_result.actions {
                match Self::execute_action(action) {
                    Ok(()) => {
                        // Track gitignore entries based on what was synced
                        match action {
                            SyncAction::CreateSymlink { link, .. }
                            | SyncAction::RemoveAndRelink { link, .. } => {
                                if let Some(name) = link.file_name() {
                                    gitignore_entries.push(name.to_string_lossy().to_string());
                                }
                            }
                            SyncAction::CreateFile { path, .. } => {
                                // Track aisync-managed files (e.g., CLAUDE.md with conditionals)
                                if let Some(name) = path.file_name() {
                                    gitignore_entries.push(name.to_string_lossy().to_string());
                                }
                            }
                            SyncAction::GenerateMdc { output, .. } => {
                                // Use relative path from project root
                                if let Ok(rel) = output.strip_prefix(project_root) {
                                    gitignore_entries.push(rel.display().to_string());
                                }
                            }
                            _ => {}
                        }
                        executed_actions.push(action.clone());
                    }
                    Err(e) => {
                        tool_error = Some(format!("{e}"));
                        break;
                    }
                }
            }

            executed_results.push(ToolSyncResult {
                tool: tool_result.tool,
                actions: executed_actions,
                error: tool_error,
            });
        }

        // Update .gitignore with managed section (INST-07)
        if !gitignore_entries.is_empty() {
            let gitignore_path = project_root.join(".gitignore");
            let entry_refs: Vec<&str> = gitignore_entries.iter().map(|s| s.as_str()).collect();
            crate::gitignore::update_managed_section(&gitignore_path, &entry_refs)
                .map_err(|e| AisyncError::Sync(SyncError::GitignoreFailed(e)))?;
        }

        Ok(SyncReport {
            results: executed_results,
        })
    }

    /// Get sync status for all enabled tools.
    pub fn status(config: &AisyncConfig, project_root: &Path) -> Result<StatusReport, AisyncError> {
        let canonical_path = project_root.join(".ai/instructions.md");
        let canonical_content = std::fs::read_to_string(&canonical_path).map_err(|_| {
            AisyncError::Sync(SyncError::CanonicalMissing {
                path: canonical_path.display().to_string(),
            })
        })?;
        let hash = content_hash(canonical_content.as_bytes());

        let mut tools = Vec::new();

        for (tool_kind, adapter, tool_config_opt) in Self::enabled_tools(config) {
            let strategy = tool_config_opt
                .map(|tc| tc.effective_sync_strategy(&config.defaults))
                .unwrap_or(config.defaults.sync_strategy);
            match adapter.sync_status(project_root, &hash, strategy) {
                Ok(status) => tools.push(status),
                Err(_) => {
                    tools.push(ToolSyncStatus {
                        tool: tool_kind,
                        strategy,
                        drift: DriftState::NotConfigured,
                        details: Some("failed to check status".to_string()),
                    });
                }
            }
        }

        // Check memory status
        let memory = Self::check_memory_status(config, project_root);

        // Check hook status
        let hooks = Self::check_hook_status(config, project_root);

        Ok(StatusReport {
            tools,
            memory,
            hooks,
        })
    }

    /// Check memory sync status for all enabled tools.
    fn check_memory_status(
        config: &AisyncConfig,
        project_root: &Path,
    ) -> Option<crate::types::MemoryStatusReport> {
        let memory_files = crate::memory::MemoryEngine::list(project_root).ok()?;
        if memory_files.is_empty() {
            return None;
        }

        let files: Vec<String> = memory_files
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .collect();

        let mut per_tool = Vec::new();

        for (tool_kind, _adapter, _) in Self::enabled_tools(config) {
            let (synced, details) = match tool_kind {
                ToolKind::ClaudeCode => {
                    match crate::memory::MemoryEngine::claude_memory_path(project_root) {
                        Ok(claude_memory) => {
                            if claude_memory.symlink_metadata().is_ok() {
                                let meta = claude_memory.symlink_metadata().unwrap();
                                if meta.file_type().is_symlink() {
                                    (true, Some("symlinked".to_string()))
                                } else if meta.is_dir() {
                                    (false, Some("native directory (not symlinked)".to_string()))
                                } else {
                                    (false, Some("unexpected file type".to_string()))
                                }
                            } else {
                                (false, Some("not synced".to_string()))
                            }
                        }
                        Err(_) => (false, Some("could not resolve path".to_string())),
                    }
                }
                ToolKind::OpenCode => {
                    let agents_md = project_root.join("AGENTS.md");
                    if agents_md.exists() {
                        let content = std::fs::read_to_string(&agents_md).unwrap_or_default();
                        if content.contains("<!-- aisync:memory -->") {
                            (true, Some("references in AGENTS.md".to_string()))
                        } else {
                            (false, Some("no memory references in AGENTS.md".to_string()))
                        }
                    } else {
                        (false, Some("AGENTS.md not found".to_string()))
                    }
                }
                ToolKind::Cursor => {
                    let mdc = project_root.join(".cursor/rules/project.mdc");
                    if mdc.exists() {
                        let content = std::fs::read_to_string(&mdc).unwrap_or_default();
                        if content.contains("<!-- aisync:memory -->") {
                            (true, Some("references in project.mdc".to_string()))
                        } else {
                            (
                                false,
                                Some("no memory references in project.mdc".to_string()),
                            )
                        }
                    } else {
                        (false, Some("project.mdc not found".to_string()))
                    }
                }
            };

            per_tool.push(crate::types::ToolMemoryStatus {
                tool: tool_kind,
                synced,
                details,
            });
        }

        Some(crate::types::MemoryStatusReport {
            file_count: memory_files.len(),
            files,
            per_tool,
        })
    }

    /// Check hook translation status for all enabled tools.
    fn check_hook_status(
        config: &AisyncConfig,
        project_root: &Path,
    ) -> Option<crate::types::HookStatusReport> {
        let hooks_config = HookEngine::parse(project_root).ok()?;
        let summaries = HookEngine::list_hooks(&hooks_config);
        if summaries.is_empty() {
            return None;
        }

        let mut per_tool = Vec::new();

        for (tool_kind, adapter, _) in Self::enabled_tools(config) {
            let translation = adapter.translate_hooks(&hooks_config);
            let (supported, translated, details) = match translation {
                Ok(HookTranslation::Supported { .. }) => {
                    let is_translated = match tool_kind {
                        ToolKind::ClaudeCode => {
                            let settings = project_root.join(".claude/settings.json");
                            if settings.exists() {
                                let content =
                                    std::fs::read_to_string(&settings).unwrap_or_default();
                                content.contains("\"hooks\"")
                            } else {
                                false
                            }
                        }
                        ToolKind::OpenCode => project_root
                            .join(".opencode/plugins/aisync-hooks.js")
                            .exists(),
                        _ => false,
                    };
                    let detail = if is_translated {
                        match tool_kind {
                            ToolKind::ClaudeCode => Some("settings.json".to_string()),
                            ToolKind::OpenCode => Some("aisync-hooks.js".to_string()),
                            _ => None,
                        }
                    } else {
                        Some("not translated yet".to_string())
                    };
                    (true, is_translated, detail)
                }
                Ok(HookTranslation::Unsupported { reason, .. }) => (false, false, Some(reason)),
                Err(e) => (false, false, Some(format!("error: {e}"))),
            };

            per_tool.push(crate::types::ToolHookStatus {
                tool: tool_kind,
                supported,
                translated,
                details,
            });
        }

        Some(crate::types::HookStatusReport {
            hook_count: summaries.len(),
            per_tool,
        })
    }

    /// Execute a single sync action on the filesystem.
    fn execute_action(action: &SyncAction) -> Result<(), AisyncError> {
        match action {
            SyncAction::CreateSymlink { link, target } => {
                #[cfg(unix)]
                {
                    std::os::unix::fs::symlink(target, link)
                        .map_err(|e| AisyncError::Sync(SyncError::SymlinkFailed(e)))?;
                }
                #[cfg(not(unix))]
                {
                    // On non-Unix, fall back to copy
                    let canonical_path = link.parent().unwrap_or(Path::new(".")).join(target);
                    std::fs::copy(&canonical_path, link)
                        .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                }
                Ok(())
            }
            SyncAction::RemoveAndRelink { link, target } => {
                std::fs::remove_file(link)
                    .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                #[cfg(unix)]
                {
                    std::os::unix::fs::symlink(target, link)
                        .map_err(|e| AisyncError::Sync(SyncError::SymlinkFailed(e)))?;
                }
                #[cfg(not(unix))]
                {
                    let canonical_path = link.parent().unwrap_or(Path::new(".")).join(target);
                    std::fs::copy(&canonical_path, link)
                        .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                }
                Ok(())
            }
            SyncAction::GenerateMdc { output, content } => {
                if let Some(parent) = output.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                }
                std::fs::write(output, content)
                    .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                Ok(())
            }
            SyncAction::CreateDirectory { path } => {
                std::fs::create_dir_all(path)
                    .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                Ok(())
            }
            SyncAction::CreateFile { path, content } => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                }
                // If path is a symlink, remove it first to avoid writing through
                // the symlink to the target file (which would corrupt canonical).
                if let Ok(meta) = path.symlink_metadata() {
                    if meta.file_type().is_symlink() {
                        std::fs::remove_file(path)
                            .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                    }
                }
                std::fs::write(path, content)
                    .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                Ok(())
            }
            SyncAction::RemoveFile { path } => {
                if path.symlink_metadata().is_ok() {
                    std::fs::remove_file(path)
                        .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                }
                Ok(())
            }
            SyncAction::UpdateGitignore { path, entries } => {
                let entry_refs: Vec<&str> = entries.iter().map(|s| s.as_str()).collect();
                crate::gitignore::update_managed_section(path, &entry_refs)
                    .map_err(|e| AisyncError::Sync(SyncError::GitignoreFailed(e)))?;
                Ok(())
            }
            SyncAction::SkipExistingFile { .. } => {
                // No filesystem change -- just recorded
                Ok(())
            }
            SyncAction::CreateMemorySymlink { link, target } => {
                // Create parent directories for the symlink
                if let Some(parent) = link.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                }
                #[cfg(unix)]
                {
                    std::os::unix::fs::symlink(target, link)
                        .map_err(|e| AisyncError::Sync(SyncError::SymlinkFailed(e)))?;
                }
                #[cfg(not(unix))]
                {
                    let _ = (link, target); // Suppress unused warnings
                    eprintln!("Warning: memory directory symlinks not supported on this platform");
                }
                Ok(())
            }
            SyncAction::UpdateMemoryReferences {
                path,
                references,
                marker_start,
                marker_end,
            } => {
                let entry_refs: Vec<&str> = references.iter().map(|s| s.as_str()).collect();
                crate::managed_section::update_managed_section(
                    path,
                    &entry_refs,
                    marker_start,
                    marker_end,
                )
                .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                Ok(())
            }
            SyncAction::WriteHookTranslation {
                path,
                content,
                tool,
            } => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                }
                match tool {
                    ToolKind::ClaudeCode => {
                        // Merge hooks into existing settings.json if it exists
                        let hooks_value: serde_json::Value = serde_json::from_str(content)
                            .map_err(|e| {
                                AisyncError::Sync(SyncError::WriteFailed(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    format!("failed to parse hook JSON: {e}"),
                                )))
                            })?;
                        let mut settings = if path.exists() {
                            let existing = std::fs::read_to_string(path)
                                .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                            serde_json::from_str::<serde_json::Value>(&existing).unwrap_or_else(
                                |_| serde_json::Value::Object(serde_json::Map::new()),
                            )
                        } else {
                            serde_json::Value::Object(serde_json::Map::new())
                        };
                        if let (Some(settings_map), Some(hooks_obj)) =
                            (settings.as_object_mut(), hooks_value.get("hooks"))
                        {
                            settings_map.insert("hooks".to_string(), hooks_obj.clone());
                        }
                        let output = serde_json::to_string_pretty(&settings).map_err(|e| {
                            AisyncError::Sync(SyncError::WriteFailed(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!("failed to serialize settings.json: {e}"),
                            )))
                        })?;
                        std::fs::write(path, output)
                            .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                    }
                    _ => {
                        // OpenCode and others: write directly
                        std::fs::write(path, content)
                            .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                    }
                }
                Ok(())
            }
            SyncAction::WarnUnsupportedHooks { .. } => {
                // No filesystem change -- advisory only
                Ok(())
            }
        }
    }

    /// Returns an iterator of (ToolKind, AnyAdapter, Option<&ToolConfig>) for all enabled tools.
    pub(crate) fn enabled_tools(
        config: &AisyncConfig,
    ) -> Vec<(ToolKind, AnyAdapter, Option<&crate::config::ToolConfig>)> {
        let mut tools = Vec::new();

        // Claude Code
        let claude_enabled = config
            .tools
            .claude_code
            .as_ref()
            .is_none_or(|tc| tc.enabled);
        if claude_enabled {
            tools.push((
                ToolKind::ClaudeCode,
                AnyAdapter::ClaudeCode(ClaudeCodeAdapter),
                config.tools.claude_code.as_ref(),
            ));
        }

        // Cursor
        let cursor_enabled = config.tools.cursor.as_ref().is_none_or(|tc| tc.enabled);
        if cursor_enabled {
            tools.push((
                ToolKind::Cursor,
                AnyAdapter::Cursor(CursorAdapter),
                config.tools.cursor.as_ref(),
            ));
        }

        // OpenCode
        let opencode_enabled = config.tools.opencode.as_ref().is_none_or(|tc| tc.enabled);
        if opencode_enabled {
            tools.push((
                ToolKind::OpenCode,
                AnyAdapter::OpenCode(OpenCodeAdapter),
                config.tools.opencode.as_ref(),
            ));
        }

        tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AisyncConfig, DefaultsConfig, SyncStrategy, ToolConfig, ToolsConfig};
    use tempfile::TempDir;

    fn setup_canonical(dir: &Path, content: &str) {
        let ai_dir = dir.join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("instructions.md"), content).unwrap();
    }

    fn all_enabled_config() -> AisyncConfig {
        AisyncConfig {
            schema_version: 1,
            defaults: DefaultsConfig {
                sync_strategy: SyncStrategy::Symlink,
            },
            tools: ToolsConfig {
                claude_code: Some(ToolConfig {
                    enabled: true,
                    sync_strategy: Some(SyncStrategy::Symlink),
                }),
                cursor: Some(ToolConfig {
                    enabled: true,
                    sync_strategy: Some(SyncStrategy::Generate),
                }),
                opencode: Some(ToolConfig {
                    enabled: true,
                    sync_strategy: Some(SyncStrategy::Symlink),
                }),
            },
        }
    }

    #[test]
    fn test_plan_returns_actions_for_all_enabled_tools() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        let config = all_enabled_config();
        let report = SyncEngine::plan(&config, dir.path()).unwrap();

        assert_eq!(report.results.len(), 3);
        let tools: Vec<ToolKind> = report.results.iter().map(|r| r.tool).collect();
        assert!(tools.contains(&ToolKind::ClaudeCode));
        assert!(tools.contains(&ToolKind::Cursor));
        assert!(tools.contains(&ToolKind::OpenCode));
    }

    #[test]
    fn test_plan_skips_disabled_tools() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        let config = AisyncConfig {
            schema_version: 1,
            defaults: DefaultsConfig {
                sync_strategy: SyncStrategy::Symlink,
            },
            tools: ToolsConfig {
                claude_code: Some(ToolConfig {
                    enabled: true,
                    sync_strategy: None,
                }),
                cursor: Some(ToolConfig {
                    enabled: false,
                    sync_strategy: None,
                }),
                opencode: Some(ToolConfig {
                    enabled: false,
                    sync_strategy: None,
                }),
            },
        };

        let report = SyncEngine::plan(&config, dir.path()).unwrap();
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].tool, ToolKind::ClaudeCode);
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_creates_symlinks_and_mdc() {
        let dir = TempDir::new().unwrap();
        let canonical = "# My Instructions";
        setup_canonical(dir.path(), canonical);

        let config = all_enabled_config();
        let plan = SyncEngine::plan(&config, dir.path()).unwrap();
        let result = SyncEngine::execute(&plan, dir.path()).unwrap();

        // Check CLAUDE.md symlink exists
        let claude_md = dir.path().join("CLAUDE.md");
        assert!(
            claude_md
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read_to_string(&claude_md).unwrap(), canonical);

        // Check AGENTS.md symlink exists
        let agents_md = dir.path().join("AGENTS.md");
        assert!(
            agents_md
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read_to_string(&agents_md).unwrap(), canonical);

        // Check .cursor/rules/project.mdc exists with frontmatter
        let mdc = dir.path().join(".cursor/rules/project.mdc");
        assert!(mdc.exists());
        let mdc_content = std::fs::read_to_string(&mdc).unwrap();
        assert!(mdc_content.contains("description: Project instructions synced by aisync"));
        assert!(mdc_content.contains("globs: \"**\""));
        assert!(mdc_content.contains("alwaysApply: true"));
        assert!(mdc_content.contains(canonical));

        // No errors
        assert!(!result.has_errors());
    }

    #[test]
    fn test_execute_continues_after_one_tool_fails() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        // Create a regular CLAUDE.md so ClaudeCode will SkipExistingFile,
        // but Cursor and OpenCode should still work
        std::fs::write(dir.path().join("CLAUDE.md"), "existing content").unwrap();

        let config = all_enabled_config();
        let plan = SyncEngine::plan(&config, dir.path()).unwrap();
        let result = SyncEngine::execute(&plan, dir.path()).unwrap();

        // ClaudeCode should have a SkipExistingFile (not an error)
        let claude_result = result
            .results
            .iter()
            .find(|r| r.tool == ToolKind::ClaudeCode)
            .unwrap();
        assert!(claude_result.error.is_none());

        // Cursor should have succeeded
        let cursor_result = result
            .results
            .iter()
            .find(|r| r.tool == ToolKind::Cursor)
            .unwrap();
        assert!(cursor_result.error.is_none());
        assert!(dir.path().join(".cursor/rules/project.mdc").exists());

        // OpenCode should have succeeded
        let opencode_result = result
            .results
            .iter()
            .find(|r| r.tool == ToolKind::OpenCode)
            .unwrap();
        assert!(opencode_result.error.is_none());
    }

    #[test]
    fn test_execute_updates_gitignore() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        let config = all_enabled_config();
        let plan = SyncEngine::plan(&config, dir.path()).unwrap();
        SyncEngine::execute(&plan, dir.path()).unwrap();

        let gitignore = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(gitignore.contains(crate::gitignore::MARKER_START));
        assert!(gitignore.contains(crate::gitignore::MARKER_END));
        assert!(gitignore.contains("CLAUDE.md"));
        assert!(gitignore.contains("AGENTS.md"));
        assert!(gitignore.contains(".cursor/rules/project.mdc"));
    }

    #[test]
    fn test_status_returns_per_tool_drift_states() {
        let dir = TempDir::new().unwrap();
        let canonical = "# Instructions";
        setup_canonical(dir.path(), canonical);

        let config = all_enabled_config();
        let status = SyncEngine::status(&config, dir.path()).unwrap();

        assert_eq!(status.tools.len(), 3);
        // All should be Missing since nothing is synced yet
        for tool_status in &status.tools {
            assert_eq!(tool_status.drift, DriftState::Missing);
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_status_in_sync_after_execute() {
        let dir = TempDir::new().unwrap();
        let canonical = "# Instructions";
        setup_canonical(dir.path(), canonical);

        let config = all_enabled_config();
        let plan = SyncEngine::plan(&config, dir.path()).unwrap();
        SyncEngine::execute(&plan, dir.path()).unwrap();

        let status = SyncEngine::status(&config, dir.path()).unwrap();
        assert!(status.all_in_sync());
    }

    #[cfg(unix)]
    #[test]
    fn test_idempotent_double_execute() {
        let dir = TempDir::new().unwrap();
        let canonical = "# Instructions";
        setup_canonical(dir.path(), canonical);

        let config = all_enabled_config();

        // First sync
        let plan1 = SyncEngine::plan(&config, dir.path()).unwrap();
        SyncEngine::execute(&plan1, dir.path()).unwrap();

        // Capture state after first sync
        let claude_content1 = std::fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        let mdc_content1 =
            std::fs::read_to_string(dir.path().join(".cursor/rules/project.mdc")).unwrap();
        let _gitignore1 = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();

        // Second sync -- should produce no changes
        let plan2 = SyncEngine::plan(&config, dir.path()).unwrap();

        // ClaudeCode and OpenCode should have empty actions (symlinks already correct)
        let claude_plan = plan2
            .results
            .iter()
            .find(|r| r.tool == ToolKind::ClaudeCode)
            .unwrap();
        assert!(
            claude_plan.actions.is_empty(),
            "expected no actions for ClaudeCode on second plan, got {:?}",
            claude_plan.actions
        );

        let opencode_plan = plan2
            .results
            .iter()
            .find(|r| r.tool == ToolKind::OpenCode)
            .unwrap();
        assert!(
            opencode_plan.actions.is_empty(),
            "expected no actions for OpenCode on second plan"
        );

        let cursor_plan = plan2
            .results
            .iter()
            .find(|r| r.tool == ToolKind::Cursor)
            .unwrap();
        assert!(
            cursor_plan.actions.is_empty(),
            "expected no actions for Cursor on second plan"
        );

        // Execute second sync anyway
        SyncEngine::execute(&plan2, dir.path()).unwrap();

        // Content should be identical
        let claude_content2 = std::fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        let mdc_content2 =
            std::fs::read_to_string(dir.path().join(".cursor/rules/project.mdc")).unwrap();

        assert_eq!(claude_content1, claude_content2);
        assert_eq!(mdc_content1, mdc_content2);
    }

    #[test]
    fn test_plan_includes_memory_sync_actions() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");
        // Create memory files
        let memory_dir = dir.path().join(".ai/memory");
        std::fs::create_dir_all(&memory_dir).unwrap();
        std::fs::write(memory_dir.join("debugging.md"), "# Debugging").unwrap();

        let config = all_enabled_config();
        let report = SyncEngine::plan(&config, dir.path()).unwrap();

        // Each tool should have memory sync actions in addition to instruction sync actions
        for tool_result in &report.results {
            let has_memory_action = tool_result.actions.iter().any(|a| {
                matches!(
                    a,
                    SyncAction::CreateMemorySymlink { .. }
                        | SyncAction::UpdateMemoryReferences { .. }
                )
            });
            assert!(
                has_memory_action,
                "expected memory sync action for {:?}, got: {:?}",
                tool_result.tool, tool_result.actions
            );
        }
    }

    #[test]
    fn test_plan_no_memory_actions_when_no_memory_files() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");
        // No .ai/memory/ directory

        let config = all_enabled_config();
        let report = SyncEngine::plan(&config, dir.path()).unwrap();

        // No memory actions should be present
        for tool_result in &report.results {
            let has_memory_action = tool_result.actions.iter().any(|a| {
                matches!(
                    a,
                    SyncAction::CreateMemorySymlink { .. }
                        | SyncAction::UpdateMemoryReferences { .. }
                )
            });
            assert!(
                !has_memory_action,
                "expected no memory sync actions for {:?} when no memory files",
                tool_result.tool,
            );
        }
    }

    #[test]
    fn test_execute_handles_memory_references() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");
        let memory_dir = dir.path().join(".ai/memory");
        std::fs::create_dir_all(&memory_dir).unwrap();
        std::fs::write(memory_dir.join("debugging.md"), "# Debugging").unwrap();

        let config = all_enabled_config();
        let plan = SyncEngine::plan(&config, dir.path()).unwrap();
        let result = SyncEngine::execute(&plan, dir.path()).unwrap();
        assert!(
            !result.has_errors(),
            "sync should not have errors: {:?}",
            result
        );

        // Check AGENTS.md has memory references
        let agents_content = std::fs::read_to_string(dir.path().join("AGENTS.md"));
        // AGENTS.md is a symlink, but managed section updates write to the resolved path
        // For OpenCode memory, UpdateMemoryReferences targets AGENTS.md
    }

    #[test]
    fn test_plan_applies_conditional_processing_per_tool() {
        let dir = TempDir::new().unwrap();
        let content = "# Common\n\n<!-- aisync:claude-only -->\nClaude-specific info\n<!-- /aisync:claude-only -->\n\n<!-- aisync:cursor-only -->\nCursor-specific info\n<!-- /aisync:cursor-only -->\n\nShared footer\n";
        setup_canonical(dir.path(), content);

        let config = all_enabled_config();
        let report = SyncEngine::plan(&config, dir.path()).unwrap();

        // The Cursor adapter generates MDC content; check that the claude-only section is stripped
        let cursor_result = report
            .results
            .iter()
            .find(|r| r.tool == ToolKind::Cursor)
            .unwrap();
        for action in &cursor_result.actions {
            if let SyncAction::GenerateMdc { content, .. } = action {
                assert!(
                    !content.contains("Claude-specific info"),
                    "Cursor MDC should NOT contain claude-only content"
                );
                assert!(
                    content.contains("Cursor-specific info"),
                    "Cursor MDC should contain cursor-only content"
                );
                assert!(
                    content.contains("Shared footer"),
                    "Cursor MDC should contain common content"
                );
            }
        }
    }

    #[test]
    fn test_plan_errors_when_canonical_missing() {
        let dir = TempDir::new().unwrap();
        // No .ai/instructions.md created

        let config = all_enabled_config();
        let result = SyncEngine::plan(&config, dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{err}").contains("canonical"));
    }

    #[cfg(unix)]
    #[test]
    fn test_conditional_sync_end_to_end_cursor_only_stripped_from_claude() {
        let dir = TempDir::new().unwrap();
        let content = "# Instructions\n\n<!-- aisync:cursor-only -->\nCursor-specific content\n<!-- /aisync:cursor-only -->\n\nShared content\n";
        setup_canonical(dir.path(), content);

        let config = all_enabled_config();
        let plan = SyncEngine::plan(&config, dir.path()).unwrap();
        let result = SyncEngine::execute(&plan, dir.path()).unwrap();
        assert!(!result.has_errors(), "sync errors: {:?}", result);

        // CLAUDE.md should NOT contain cursor-only content (regular file, not symlink)
        let claude_md = dir.path().join("CLAUDE.md");
        let claude_content = std::fs::read_to_string(&claude_md).unwrap();
        assert!(
            !claude_content.contains("Cursor-specific content"),
            "CLAUDE.md should NOT contain cursor-only content"
        );
        assert!(
            claude_content.contains("Shared content"),
            "CLAUDE.md should contain shared content"
        );
        let meta = claude_md.symlink_metadata().unwrap();
        assert!(
            !meta.file_type().is_symlink(),
            "CLAUDE.md should be a regular file when conditionals are active"
        );

        // Cursor MDC should contain cursor-only content
        let mdc_content =
            std::fs::read_to_string(dir.path().join(".cursor/rules/project.mdc")).unwrap();
        assert!(
            mdc_content.contains("Cursor-specific content"),
            "Cursor MDC should contain cursor-only content"
        );

        // AGENTS.md (OpenCode) should NOT contain cursor-only content
        let agents_content = std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
        assert!(
            !agents_content.contains("Cursor-specific content"),
            "AGENTS.md should NOT contain cursor-only content"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_conditional_sync_end_to_end_claude_only_kept_in_claude() {
        let dir = TempDir::new().unwrap();
        let content = "# Instructions\n\n<!-- aisync:claude-only -->\nClaude-only content\n<!-- /aisync:claude-only -->\n\nShared content\n";
        setup_canonical(dir.path(), content);

        let config = all_enabled_config();
        let plan = SyncEngine::plan(&config, dir.path()).unwrap();
        let result = SyncEngine::execute(&plan, dir.path()).unwrap();
        assert!(!result.has_errors(), "sync errors: {:?}", result);

        // CLAUDE.md should contain claude-only content
        let claude_content = std::fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        assert!(
            claude_content.contains("Claude-only content"),
            "CLAUDE.md should contain claude-only content"
        );

        // Cursor MDC should NOT contain claude-only content
        let mdc_content =
            std::fs::read_to_string(dir.path().join(".cursor/rules/project.mdc")).unwrap();
        assert!(
            !mdc_content.contains("Claude-only content"),
            "Cursor MDC should NOT contain claude-only content"
        );

        // AGENTS.md should NOT contain claude-only content
        let agents_content = std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
        assert!(
            !agents_content.contains("Claude-only content"),
            "AGENTS.md should NOT contain claude-only content"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_no_conditionals_claude_md_remains_symlink() {
        let dir = TempDir::new().unwrap();
        let content = "# Instructions\n\nNo conditional sections here.\n";
        setup_canonical(dir.path(), content);

        let config = all_enabled_config();
        let plan = SyncEngine::plan(&config, dir.path()).unwrap();
        let result = SyncEngine::execute(&plan, dir.path()).unwrap();
        assert!(!result.has_errors(), "sync errors: {:?}", result);

        let claude_md = dir.path().join("CLAUDE.md");
        let meta = claude_md.symlink_metadata().unwrap();
        assert!(
            meta.file_type().is_symlink(),
            "CLAUDE.md should be a symlink when no conditionals are used"
        );
        let claude_content = std::fs::read_to_string(&claude_md).unwrap();
        assert_eq!(claude_content, content);
    }
}
