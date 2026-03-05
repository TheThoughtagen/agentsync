use std::path::Path;

use crate::adapter::{AnyAdapter, ClaudeCodeAdapter, CursorAdapter, OpenCodeAdapter, ToolAdapter};
use crate::config::{AisyncConfig, SyncStrategy};
use crate::error::{AisyncError, SyncError};
use crate::types::{
    content_hash, DriftState, StatusReport, SyncAction, SyncReport, ToolKind, ToolSyncResult,
    ToolSyncStatus,
};

/// The sync engine orchestrates planning, executing, and checking status
/// of sync operations across all enabled tool adapters.
pub struct SyncEngine;

impl SyncEngine {
    /// Plan sync actions for all enabled tools. Does not modify the filesystem.
    /// Used by both `aisync sync` and `aisync sync --dry-run`.
    pub fn plan(config: &AisyncConfig, project_root: &Path) -> Result<SyncReport, AisyncError> {
        let canonical_path = project_root.join(".ai/instructions.md");
        let canonical_content =
            std::fs::read_to_string(&canonical_path).map_err(|_| {
                AisyncError::Sync(SyncError::CanonicalMissing {
                    path: canonical_path.display().to_string(),
                })
            })?;

        let mut results = Vec::new();

        for (tool_kind, adapter, tool_config_opt) in Self::enabled_tools(config) {
            let strategy = tool_config_opt
                .map(|tc| tc.effective_sync_strategy(&config.defaults))
                .unwrap_or(config.defaults.sync_strategy);

            match adapter.plan_sync(project_root, &canonical_content, strategy) {
                Ok(actions) => {
                    results.push(ToolSyncResult {
                        tool: tool_kind,
                        actions,
                        error: None,
                    });
                }
                Err(e) => {
                    results.push(ToolSyncResult {
                        tool: tool_kind,
                        actions: vec![],
                        error: Some(format!("{e}")),
                    });
                }
            }
        }

        Ok(SyncReport { results })
    }

    /// Execute a planned sync. Mutates the filesystem.
    /// Non-interactive mode: SkipExistingFile actions are recorded but not executed.
    /// After executing tool syncs, updates .gitignore with aisync-managed section.
    pub fn execute(
        report: &SyncReport,
        project_root: &Path,
    ) -> Result<SyncReport, AisyncError> {
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
                                    gitignore_entries
                                        .push(name.to_string_lossy().to_string());
                                }
                            }
                            SyncAction::GenerateMdc { output, .. } => {
                                // Use relative path from project root
                                if let Ok(rel) = output.strip_prefix(project_root) {
                                    gitignore_entries
                                        .push(rel.display().to_string());
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
    pub fn status(
        config: &AisyncConfig,
        project_root: &Path,
    ) -> Result<StatusReport, AisyncError> {
        let canonical_path = project_root.join(".ai/instructions.md");
        let canonical_content =
            std::fs::read_to_string(&canonical_path).map_err(|_| {
                AisyncError::Sync(SyncError::CanonicalMissing {
                    path: canonical_path.display().to_string(),
                })
            })?;
        let hash = content_hash(canonical_content.as_bytes());

        let mut tools = Vec::new();

        for (tool_kind, adapter, _) in Self::enabled_tools(config) {
            match adapter.sync_status(project_root, &hash) {
                Ok(status) => tools.push(status),
                Err(_) => {
                    tools.push(ToolSyncStatus {
                        tool: tool_kind,
                        strategy: SyncStrategy::Symlink,
                        drift: DriftState::NotConfigured,
                        details: Some("failed to check status".to_string()),
                    });
                }
            }
        }

        Ok(StatusReport { tools })
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
                std::fs::write(path, content)
                    .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
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
        }
    }

    /// Returns an iterator of (ToolKind, AnyAdapter, Option<&ToolConfig>) for all enabled tools.
    fn enabled_tools(
        config: &AisyncConfig,
    ) -> Vec<(ToolKind, AnyAdapter, Option<&crate::config::ToolConfig>)> {
        let mut tools = Vec::new();

        // Claude Code
        let claude_enabled = config
            .tools
            .claude_code
            .as_ref()
            .map_or(true, |tc| tc.enabled);
        if claude_enabled {
            tools.push((
                ToolKind::ClaudeCode,
                AnyAdapter::ClaudeCode(ClaudeCodeAdapter),
                config.tools.claude_code.as_ref(),
            ));
        }

        // Cursor
        let cursor_enabled = config.tools.cursor.as_ref().map_or(true, |tc| tc.enabled);
        if cursor_enabled {
            tools.push((
                ToolKind::Cursor,
                AnyAdapter::Cursor(CursorAdapter),
                config.tools.cursor.as_ref(),
            ));
        }

        // OpenCode
        let opencode_enabled = config
            .tools
            .opencode
            .as_ref()
            .map_or(true, |tc| tc.enabled);
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
    use crate::config::{AisyncConfig, DefaultsConfig, ToolConfig, ToolsConfig};
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
        assert!(claude_md.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&claude_md).unwrap(), canonical);

        // Check AGENTS.md symlink exists
        let agents_md = dir.path().join("AGENTS.md");
        assert!(agents_md.symlink_metadata().unwrap().file_type().is_symlink());
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
        let claude_result = result.results.iter().find(|r| r.tool == ToolKind::ClaudeCode).unwrap();
        assert!(claude_result.error.is_none());

        // Cursor should have succeeded
        let cursor_result = result.results.iter().find(|r| r.tool == ToolKind::Cursor).unwrap();
        assert!(cursor_result.error.is_none());
        assert!(dir.path().join(".cursor/rules/project.mdc").exists());

        // OpenCode should have succeeded
        let opencode_result = result.results.iter().find(|r| r.tool == ToolKind::OpenCode).unwrap();
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
        let gitignore1 = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();

        // Second sync -- should produce no changes
        let plan2 = SyncEngine::plan(&config, dir.path()).unwrap();

        // ClaudeCode and OpenCode should have empty actions (symlinks already correct)
        let claude_plan = plan2.results.iter().find(|r| r.tool == ToolKind::ClaudeCode).unwrap();
        assert!(
            claude_plan.actions.is_empty(),
            "expected no actions for ClaudeCode on second plan, got {:?}",
            claude_plan.actions
        );

        let opencode_plan = plan2.results.iter().find(|r| r.tool == ToolKind::OpenCode).unwrap();
        assert!(
            opencode_plan.actions.is_empty(),
            "expected no actions for OpenCode on second plan"
        );

        let cursor_plan = plan2.results.iter().find(|r| r.tool == ToolKind::Cursor).unwrap();
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
    fn test_plan_errors_when_canonical_missing() {
        let dir = TempDir::new().unwrap();
        // No .ai/instructions.md created

        let config = all_enabled_config();
        let result = SyncEngine::plan(&config, dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{err}").contains("canonical"));
    }
}
