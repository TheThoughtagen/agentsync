use std::path::{Path, PathBuf};

use crate::adapter::{AnyAdapter, ToolAdapter};
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
        Self::plan_all_internal(config, project_root)
    }

    /// Plan sync actions for only the specified tools, while still running full
    /// deduplication across ALL enabled tools to prevent path conflicts.
    ///
    /// This is used by `aisync add-tool` to sync only newly added tools without
    /// re-syncing everything.
    pub fn plan_for_tools(
        config: &AisyncConfig,
        project_root: &Path,
        only_tools: &[ToolKind],
    ) -> Result<SyncReport, AisyncError> {
        if only_tools.is_empty() {
            return Ok(SyncReport { results: vec![] });
        }

        let mut report = Self::plan_all_internal(config, project_root)?;
        report
            .results
            .retain(|r| only_tools.contains(&r.tool));
        Ok(report)
    }

    /// Internal helper that plans sync for all enabled tools with deduplication.
    /// Both `plan()` and `plan_for_tools()` delegate to this.
    fn plan_all_internal(
        config: &AisyncConfig,
        project_root: &Path,
    ) -> Result<SyncReport, AisyncError> {
        let canonical_path = project_root.join(".ai/instructions.md");
        let canonical_content = std::fs::read_to_string(&canonical_path).map_err(|_| {
            AisyncError::Sync(SyncError::CanonicalMissing {
                path: canonical_path.display().to_string(),
            })
        })?;

        // Scan for memory files
        let memory_files = crate::memory::MemoryEngine::list(project_root)?;

        // Load canonical rule files
        let rules = crate::rules::RuleEngine::load(project_root)?;

        // Load MCP config
        let mcp_config = crate::mcp::McpEngine::load(project_root)?;

        // Run security scan (warnings are advisory, don't block)
        let security_warnings =
            crate::security::SecurityScanner::scan_mcp_config(&mcp_config);

        // Sanitize env values before passing to adapters
        let mut mcp_config = mcp_config;
        crate::mcp::McpEngine::sanitize_env(&mut mcp_config);

        // Load canonical command files
        let commands = crate::commands::CommandEngine::load(project_root)?;

        // Load canonical skill and agent files
        let skills = crate::skills::SkillEngine::load(project_root)?;
        let agents = crate::agents::AgentEngine::load(project_root)?;

        let mut results = Vec::new();

        for (tool_kind, adapter, tool_config_opt) in Self::enabled_tools(config, project_root) {
            let strategy = tool_config_opt
                .map(|tc| tc.effective_sync_strategy(&config.defaults))
                .unwrap_or_else(|| adapter.default_sync_strategy());

            let mut actions = Vec::new();

            // Apply conditional processing for this tool
            let tool_content =
                ConditionalProcessor::process(&canonical_content, tool_kind.clone());

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
                            tool: tool_kind.clone(),
                            reason: format!("memory sync failed: {e}"),
                        });
                    }
                }
            }

            // Plan hook translation (if hooks.toml exists)
            if let Ok(hooks_config) = HookEngine::parse(project_root) {
                match adapter.translate_hooks(&hooks_config) {
                    Ok(HookTranslation::Supported { tool, content, .. }) => {
                        let path = match &tool {
                            ToolKind::ClaudeCode => project_root.join(".claude/settings.json"),
                            ToolKind::OpenCode => {
                                project_root.join(".opencode/plugins/aisync-hooks.js")
                            }
                            ToolKind::Cursor => project_root.join(".cursor/hooks.json"),
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

            // Plan rule sync (if canonical rules exist)
            if !rules.is_empty() {
                match adapter.plan_rules_sync(project_root, &rules) {
                    Ok(rule_actions) => actions.extend(rule_actions),
                    Err(e) => {
                        actions.push(SyncAction::WarnUnsupportedDimension {
                            tool: tool_kind.clone(),
                            dimension: "rules".into(),
                            reason: format!("rule sync failed: {e}"),
                        });
                    }
                }
            }

            // Plan MCP sync (if MCP servers exist)
            if !mcp_config.servers.is_empty() {
                // Emit security warnings as WarnUnsupportedDimension actions
                // so they flow through the existing action display pipeline
                for warning in &security_warnings {
                    actions.push(SyncAction::WarnUnsupportedDimension {
                        tool: tool_kind.clone(),
                        dimension: "security".into(),
                        reason: format!(
                            "Potential secret in server '{}' env var '{}': matches {} pattern",
                            warning.server_name, warning.env_key, warning.pattern_name
                        ),
                    });
                }

                match adapter.plan_mcp_sync(project_root, &mcp_config) {
                    Ok(mcp_actions) => actions.extend(mcp_actions),
                    Err(e) => {
                        actions.push(SyncAction::WarnUnsupportedDimension {
                            tool: tool_kind.clone(),
                            dimension: "mcp".into(),
                            reason: format!("MCP sync failed: {e}"),
                        });
                    }
                }
            }

            // Plan command sync (if canonical commands exist)
            if !commands.is_empty() {
                match adapter.plan_commands_sync(project_root, &commands) {
                    Ok(cmd_actions) => actions.extend(cmd_actions),
                    Err(e) => {
                        actions.push(SyncAction::WarnUnsupportedDimension {
                            tool: tool_kind.clone(),
                            dimension: "commands".into(),
                            reason: format!("command sync failed: {e}"),
                        });
                    }
                }
            }

            // Plan skills sync (if canonical skills exist)
            if !skills.is_empty() {
                match adapter.plan_skills_sync(project_root, &skills) {
                    Ok(skill_actions) => actions.extend(skill_actions),
                    Err(e) => {
                        actions.push(SyncAction::WarnUnsupportedDimension {
                            tool: tool_kind.clone(),
                            dimension: "skills".into(),
                            reason: format!("skill sync failed: {e}"),
                        });
                    }
                }
            }

            // Plan agents sync (if canonical agents exist)
            if !agents.is_empty() {
                match adapter.plan_agents_sync(project_root, &agents) {
                    Ok(agent_actions) => actions.extend(agent_actions),
                    Err(e) => {
                        actions.push(SyncAction::WarnUnsupportedDimension {
                            tool: tool_kind.clone(),
                            dimension: "agents".into(),
                            reason: format!("agent sync failed: {e}"),
                        });
                    }
                }
            }

            results.push(ToolSyncResult {
                tool: tool_kind,
                actions,
                error: None,
            });
        }

        Self::deduplicate_actions(&mut results);

        Ok(SyncReport { results })
    }

    /// Deduplicate sync actions that target the same output path.
    /// When multiple tools produce CreateSymlink or CreateFile for the same path,
    /// only the first tool's action is kept. This handles the Codex+OpenCode
    /// AGENTS.md case where both tools legitimately target the same file.
    fn deduplicate_actions(results: &mut [ToolSyncResult]) {
        use std::collections::HashSet;
        let mut claimed_paths: HashSet<PathBuf> = HashSet::new();
        for result in results.iter_mut() {
            result.actions.retain(|action| {
                let path = match action {
                    SyncAction::CreateSymlink { link, .. } => Some(link.clone()),
                    SyncAction::CreateFile { path, .. } => Some(path.clone()),
                    SyncAction::RemoveAndRelink { link, .. } => Some(link.clone()),
                    _ => None,
                };
                if let Some(p) = path {
                    claimed_paths.insert(p) // returns true if newly inserted (keep), false if duplicate (remove)
                } else {
                    true // keep non-path actions (warnings, directory creation, etc.)
                }
            });
        }
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
                tool: tool_result.tool.clone(),
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

        for (tool_kind, adapter, tool_config_opt) in Self::enabled_tools(config, project_root) {
            let strategy = tool_config_opt
                .map(|tc| tc.effective_sync_strategy(&config.defaults))
                .unwrap_or_else(|| adapter.default_sync_strategy());
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

        for (tool_kind, _adapter, _) in Self::enabled_tools(config, project_root) {
            let (synced, details) = match &tool_kind {
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
                ToolKind::Windsurf => {
                    let md = project_root.join(".windsurf/rules/project.md");
                    if md.exists() {
                        let content = std::fs::read_to_string(&md).unwrap_or_default();
                        if content.contains("<!-- aisync:memory -->") {
                            (true, Some("references in project.md".to_string()))
                        } else {
                            (
                                false,
                                Some("no memory references in project.md".to_string()),
                            )
                        }
                    } else {
                        (false, Some("project.md not found".to_string()))
                    }
                }
                ToolKind::Codex => {
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
                ToolKind::Custom(_) => {
                    (false, Some("memory sync not supported for custom tools".to_string()))
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

        for (tool_kind, adapter, _) in Self::enabled_tools(config, project_root) {
            let translation = adapter.translate_hooks(&hooks_config);
            let (supported, translated, details) = match translation {
                Ok(HookTranslation::Supported { .. }) => {
                    let is_translated = match &tool_kind {
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
                        match &tool_kind {
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
                // If symlink already exists and points to the correct target, skip
                if let Ok(existing_target) = std::fs::read_link(link) {
                    if existing_target == *target {
                        return Ok(());
                    }
                }
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
            SyncAction::WarnContentSize { .. } => {
                // No filesystem change -- advisory only
                Ok(())
            }
            SyncAction::CreateRuleFile { output, content, .. } => {
                if let Some(parent) = output.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                }
                std::fs::write(output, content)
                    .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                Ok(())
            }
            SyncAction::WriteMcpConfig { output, content } => {
                if let Some(parent) = output.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                }
                std::fs::write(output, content)
                    .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                Ok(())
            }
            SyncAction::CopyCommandFile { output, source, .. } => {
                if let Some(parent) = output.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                }
                std::fs::copy(source, output)
                    .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                Ok(())
            }
            SyncAction::WriteSkillFile { output, content, .. } => {
                if let Some(parent) = output.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                }
                std::fs::write(output, content)
                    .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                Ok(())
            }
            SyncAction::WriteAgentFile { output, content, .. } => {
                if let Some(parent) = output.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                }
                std::fs::write(output, content)
                    .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                Ok(())
            }
            SyncAction::RemoveSkillDir { path } => {
                if path.is_dir() {
                    std::fs::remove_dir_all(path)
                        .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                }
                Ok(())
            }
            SyncAction::WarnUnsupportedDimension { .. } => {
                // No filesystem change -- advisory only
                Ok(())
            }
        }
    }

    /// Returns an iterator of (ToolKind, AnyAdapter, Option<&ToolConfig>) for all enabled tools.
    pub(crate) fn enabled_tools<'a>(
        config: &'a AisyncConfig,
        project_root: &Path,
    ) -> Vec<(ToolKind, AnyAdapter, Option<&'a crate::config::ToolConfig>)> {
        let mut tools = Vec::new();
        for adapter in AnyAdapter::all_builtin() {
            let key = adapter.name().as_str().to_string();
            if config.tools.is_enabled(&key) {
                tools.push((
                    adapter.name(),
                    adapter,
                    config.tools.get_tool(&key),
                ));
            }
        }

        // Include TOML-defined adapters from .ai/adapters/*.toml
        for adapter in crate::declarative::discover_toml_adapters(project_root) {
            let key = adapter.name().as_str().to_string();
            if config.tools.is_enabled(&key) {
                let tool_config = config.tools.get_tool(&key);
                tools.push((
                    adapter.name(),
                    AnyAdapter::Plugin(std::sync::Arc::new(adapter)),
                    tool_config,
                ));
            }
        }

        // Collect seen names for deduplication against inventory adapters
        let mut seen_names: std::collections::HashSet<String> =
            tools.iter().map(|(tk, _, _)| tk.as_str().to_string()).collect();

        // Include compile-time registered adapters from inventory
        for factory in inventory::iter::<aisync_adapter::AdapterFactory> {
            let adapter = (factory.create)();
            let name_str = adapter.name().as_str().to_string();

            if seen_names.contains(&name_str) {
                eprintln!(
                    "Warning: inventory adapter '{}' skipped (name collision with builtin or TOML adapter)",
                    name_str
                );
                continue;
            }
            seen_names.insert(name_str.clone());

            if config.tools.is_enabled(&name_str) {
                let tool_config = config.tools.get_tool(&name_str);
                tools.push((
                    adapter.name(),
                    AnyAdapter::Plugin(std::sync::Arc::from(adapter)),
                    tool_config,
                ));
            }
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
        let mut tools = ToolsConfig::default();
        tools.set_tool("claude-code".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Symlink),
        });
        tools.set_tool("cursor".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Generate),
        });
        tools.set_tool("opencode".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Symlink),
        });
        tools.set_tool("windsurf".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Symlink),
        });
        tools.set_tool("codex".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Symlink),
        });
        AisyncConfig {
            schema_version: 1,
            defaults: DefaultsConfig {
                sync_strategy: SyncStrategy::Symlink,
            },
            tools,
        }
    }

    #[test]
    fn test_plan_returns_actions_for_all_enabled_tools() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        let config = all_enabled_config();
        let report = SyncEngine::plan(&config, dir.path()).unwrap();

        assert_eq!(report.results.len(), 5);
        let tools: Vec<ToolKind> = report.results.iter().map(|r| r.tool.clone()).collect();
        assert!(tools.contains(&ToolKind::ClaudeCode));
        assert!(tools.contains(&ToolKind::Cursor));
        assert!(tools.contains(&ToolKind::OpenCode));
        assert!(tools.contains(&ToolKind::Windsurf));
        assert!(tools.contains(&ToolKind::Codex));
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
            tools: {
                let mut t = ToolsConfig::default();
                t.set_tool("claude-code".into(), ToolConfig {
                    enabled: true,
                    sync_strategy: None,
                });
                t.set_tool("cursor".into(), ToolConfig {
                    enabled: false,
                    sync_strategy: None,
                });
                t.set_tool("opencode".into(), ToolConfig {
                    enabled: false,
                    sync_strategy: None,
                });
                t.set_tool("windsurf".into(), ToolConfig {
                    enabled: false,
                    sync_strategy: None,
                });
                t.set_tool("codex".into(), ToolConfig {
                    enabled: false,
                    sync_strategy: None,
                });
                t
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

        assert_eq!(status.tools.len(), 5);
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
    fn test_plan_deduplicates_agents_md_with_codex_and_opencode() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        // Enable both codex and opencode (both target AGENTS.md)
        let mut tools = ToolsConfig::default();
        tools.set_tool("codex".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Symlink),
        });
        tools.set_tool("opencode".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Symlink),
        });
        // Disable others to simplify
        tools.set_tool("claude-code".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });
        tools.set_tool("cursor".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });
        tools.set_tool("windsurf".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });

        let config = AisyncConfig {
            schema_version: 1,
            defaults: DefaultsConfig {
                sync_strategy: SyncStrategy::Symlink,
            },
            tools,
        };

        let report = SyncEngine::plan(&config, dir.path()).unwrap();

        // Both tools should appear in results
        assert_eq!(report.results.len(), 2);

        // Count total AGENTS.md CreateSymlink actions across all results
        let agents_symlink_count: usize = report
            .results
            .iter()
            .flat_map(|r| &r.actions)
            .filter(|a| match a {
                SyncAction::CreateSymlink { link, .. } => {
                    link.file_name().map_or(false, |n| n == "AGENTS.md")
                }
                _ => false,
            })
            .count();

        assert_eq!(
            agents_symlink_count, 1,
            "expected exactly one AGENTS.md symlink action, got {agents_symlink_count}"
        );
    }

    #[test]
    fn test_plan_no_dedup_for_different_paths() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        // Enable windsurf and cursor (different output paths)
        let mut tools = ToolsConfig::default();
        tools.set_tool("windsurf".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Generate),
        });
        tools.set_tool("cursor".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Generate),
        });
        tools.set_tool("claude-code".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });
        tools.set_tool("opencode".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });
        tools.set_tool("codex".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });

        let config = AisyncConfig {
            schema_version: 1,
            defaults: DefaultsConfig {
                sync_strategy: SyncStrategy::Generate,
            },
            tools,
        };

        let report = SyncEngine::plan(&config, dir.path()).unwrap();

        // Both tools should have actions (no false dedup)
        for result in &report.results {
            let has_create = result
                .actions
                .iter()
                .any(|a| matches!(a, SyncAction::CreateFile { .. } | SyncAction::GenerateMdc { .. }));
            assert!(
                has_create,
                "expected create action for {:?}, got {:?}",
                result.tool, result.actions
            );
        }
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

    // ---- plan_for_tools tests ----

    #[test]
    fn test_plan_for_tools_returns_only_requested_tools() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        let mut tools = ToolsConfig::default();
        tools.set_tool("windsurf".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Generate),
        });
        tools.set_tool("cursor".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Generate),
        });
        tools.set_tool("claude-code".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Symlink),
        });
        // Disable others
        tools.set_tool("opencode".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });
        tools.set_tool("codex".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });

        let config = AisyncConfig {
            schema_version: 1,
            defaults: DefaultsConfig::default(),
            tools,
        };

        let report =
            SyncEngine::plan_for_tools(&config, dir.path(), &[ToolKind::Windsurf]).unwrap();

        // Only Windsurf should be in results
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].tool, ToolKind::Windsurf);
    }

    #[test]
    fn test_plan_for_tools_empty_tools_returns_empty() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        let config = all_enabled_config();
        let report = SyncEngine::plan_for_tools(&config, dir.path(), &[]).unwrap();

        assert!(report.results.is_empty(), "empty only_tools should return empty report");
    }

    #[test]
    fn test_plan_for_tools_deduplicates_across_all_enabled() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        // Enable both codex and opencode (both target AGENTS.md)
        let mut tools = ToolsConfig::default();
        tools.set_tool("codex".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Symlink),
        });
        tools.set_tool("opencode".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Symlink),
        });
        tools.set_tool("claude-code".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });
        tools.set_tool("cursor".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });
        tools.set_tool("windsurf".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });

        let config = AisyncConfig {
            schema_version: 1,
            defaults: DefaultsConfig {
                sync_strategy: SyncStrategy::Symlink,
            },
            tools,
        };

        // Request only Codex, but deduplication should still consider OpenCode
        let report =
            SyncEngine::plan_for_tools(&config, dir.path(), &[ToolKind::Codex]).unwrap();

        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].tool, ToolKind::Codex);

        // Codex should have its AGENTS.md action removed by deduplication
        // (OpenCode comes first in all_builtin() order and claims AGENTS.md)
        let agents_symlink_count = report
            .results
            .iter()
            .flat_map(|r| &r.actions)
            .filter(|a| match a {
                SyncAction::CreateSymlink { link, .. } => {
                    link.file_name().map_or(false, |n| n == "AGENTS.md")
                }
                _ => false,
            })
            .count();

        assert_eq!(
            agents_symlink_count, 0,
            "Codex's AGENTS.md should be deduplicated away since OpenCode claims it first"
        );
    }

    #[test]
    fn test_plan_for_tools_existing_plan_unchanged() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        let config = all_enabled_config();

        // Full plan
        let full_report = SyncEngine::plan(&config, dir.path()).unwrap();

        // Partial plan requesting all tools should match full plan
        let all_tools: Vec<ToolKind> = full_report.results.iter().map(|r| r.tool.clone()).collect();
        let partial_report =
            SyncEngine::plan_for_tools(&config, dir.path(), &all_tools).unwrap();

        assert_eq!(full_report.results.len(), partial_report.results.len());
        for (full, partial) in full_report.results.iter().zip(partial_report.results.iter()) {
            assert_eq!(full.tool, partial.tool);
            assert_eq!(full.actions.len(), partial.actions.len());
        }
    }

    // --- TOML adapter integration tests ---

    #[test]
    fn test_enabled_tools_includes_toml_adapters() {
        let dir = TempDir::new().unwrap();

        // Create TOML adapter
        let adapters_dir = dir.path().join(".ai/adapters");
        std::fs::create_dir_all(&adapters_dir).unwrap();
        let toml = r#"
name = "aider"
display_name = "Aider"

[sync]
strategy = "symlink"
instruction_path = "AIDER.md"
"#;
        std::fs::write(adapters_dir.join("aider.toml"), toml).unwrap();

        let mut config = all_enabled_config();
        config.tools.set_tool("aider".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Symlink),
        });
        let tools = SyncEngine::enabled_tools(&config, dir.path());
        let tool_kinds: Vec<ToolKind> = tools.iter().map(|(k, _, _)| k.clone()).collect();
        assert!(
            tool_kinds.contains(&ToolKind::Custom("aider".to_string())),
            "enabled_tools should include TOML-defined aider adapter"
        );
    }

    #[test]
    fn test_plan_includes_toml_adapter_generate() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        // Create TOML adapter with Generate strategy
        let adapters_dir = dir.path().join(".ai/adapters");
        std::fs::create_dir_all(&adapters_dir).unwrap();
        let toml = r#"
name = "aider"
display_name = "Aider"

[sync]
strategy = "generate"
instruction_path = ".aider/rules/project.md"

[template]
content = "{{content}}"
"#;
        std::fs::write(adapters_dir.join("aider.toml"), toml).unwrap();

        let mut config = all_enabled_config();
        config.tools.set_tool("aider".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Generate),
        });
        let report = SyncEngine::plan(&config, dir.path()).unwrap();

        let aider_result = report
            .results
            .iter()
            .find(|r| r.tool == ToolKind::Custom("aider".to_string()));
        assert!(aider_result.is_some(), "aider should appear in sync plan");

        let aider = aider_result.unwrap();
        let has_create = aider
            .actions
            .iter()
            .any(|a| matches!(a, SyncAction::CreateFile { .. }));
        assert!(has_create, "aider Generate strategy should produce CreateFile action");
    }

    #[test]
    fn test_plan_includes_toml_adapter_symlink() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        // Create TOML adapter with Symlink strategy
        let adapters_dir = dir.path().join(".ai/adapters");
        std::fs::create_dir_all(&adapters_dir).unwrap();
        let toml = r#"
name = "aider"
display_name = "Aider"

[sync]
strategy = "symlink"
instruction_path = "AIDER.md"
"#;
        std::fs::write(adapters_dir.join("aider.toml"), toml).unwrap();

        let mut config = all_enabled_config();
        config.tools.set_tool("aider".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Symlink),
        });
        let report = SyncEngine::plan(&config, dir.path()).unwrap();

        let aider_result = report
            .results
            .iter()
            .find(|r| r.tool == ToolKind::Custom("aider".to_string()));
        assert!(aider_result.is_some(), "aider should appear in sync plan");

        let aider = aider_result.unwrap();
        let has_symlink = aider
            .actions
            .iter()
            .any(|a| matches!(a, SyncAction::CreateSymlink { .. }));
        assert!(has_symlink, "aider Symlink strategy should produce CreateSymlink action");
    }

    #[test]
    fn test_status_includes_toml_adapter() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        // Create TOML adapter
        let adapters_dir = dir.path().join(".ai/adapters");
        std::fs::create_dir_all(&adapters_dir).unwrap();
        let toml = r#"
name = "aider"
display_name = "Aider"

[sync]
strategy = "symlink"
instruction_path = "AIDER.md"
"#;
        std::fs::write(adapters_dir.join("aider.toml"), toml).unwrap();

        let mut config = all_enabled_config();
        config.tools.set_tool("aider".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Symlink),
        });
        let status = SyncEngine::status(&config, dir.path()).unwrap();

        let aider_status = status
            .tools
            .iter()
            .find(|t| t.tool == ToolKind::Custom("aider".to_string()));
        assert!(aider_status.is_some(), "aider should appear in status report");
        assert_eq!(aider_status.unwrap().drift, DriftState::Missing);
    }

    // --- Skills/Agents integration tests ---

    fn cursor_only_config() -> AisyncConfig {
        let mut tools = ToolsConfig::default();
        tools.set_tool("cursor".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Generate),
        });
        tools.set_tool("claude-code".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });
        tools.set_tool("opencode".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });
        tools.set_tool("windsurf".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });
        tools.set_tool("codex".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });
        AisyncConfig {
            schema_version: 1,
            defaults: DefaultsConfig {
                sync_strategy: SyncStrategy::Generate,
            },
            tools,
        }
    }

    #[test]
    fn test_plan_cursor_with_skills_produces_write_skill_file_actions() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        // Create a canonical skill
        let skill_dir = dir.path().join(".ai/skills/my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# My Skill").unwrap();

        let config = cursor_only_config();
        let report = SyncEngine::plan(&config, dir.path()).unwrap();

        let cursor_result = report.results.iter().find(|r| r.tool == ToolKind::Cursor).unwrap();
        let has_write_skill = cursor_result.actions.iter().any(|a| {
            matches!(a, SyncAction::WriteSkillFile { .. })
        });
        assert!(
            has_write_skill,
            "Cursor with .ai/skills/ should produce WriteSkillFile actions, got: {:?}",
            cursor_result.actions
        );
    }

    #[test]
    fn test_plan_cursor_with_agents_produces_write_agent_file_actions() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        // Create a canonical agent
        let agents_dir = dir.path().join(".ai/agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(agents_dir.join("my-agent.md"), "# My Agent").unwrap();

        let config = cursor_only_config();
        let report = SyncEngine::plan(&config, dir.path()).unwrap();

        let cursor_result = report.results.iter().find(|r| r.tool == ToolKind::Cursor).unwrap();
        let has_write_agent = cursor_result.actions.iter().any(|a| {
            matches!(a, SyncAction::WriteAgentFile { .. })
        });
        assert!(
            has_write_agent,
            "Cursor with .ai/agents/ should produce WriteAgentFile actions, got: {:?}",
            cursor_result.actions
        );
    }

    #[test]
    fn test_plan_cursor_hooks_routes_to_cursor_hooks_json() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        // Create a hooks.toml in the canonical TOML format used by HookEngine
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(
            ai_dir.join("hooks.toml"),
            r#"[[PostToolUse]]

[[PostToolUse.hooks]]
type = "command"
command = "cargo fmt"
"#,
        ).unwrap();

        let config = cursor_only_config();
        let report = SyncEngine::plan(&config, dir.path()).unwrap();

        let cursor_result = report.results.iter().find(|r| r.tool == ToolKind::Cursor).unwrap();
        let hook_translation = cursor_result.actions.iter().find(|a| {
            matches!(a, SyncAction::WriteHookTranslation { .. })
        });
        assert!(
            hook_translation.is_some(),
            "Cursor with hooks.toml should produce WriteHookTranslation (not WarnUnsupportedHooks), got: {:?}",
            cursor_result.actions
        );

        // Verify the path is .cursor/hooks.json
        if let Some(SyncAction::WriteHookTranslation { path, .. }) = hook_translation {
            assert!(
                path.ends_with(".cursor/hooks.json"),
                "hook translation path should be .cursor/hooks.json, got: {:?}",
                path
            );
        }
    }

    #[test]
    fn test_plan_non_cursor_adapters_no_skill_agent_actions() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        // Create canonical skills and agents
        let skill_dir = dir.path().join(".ai/skills/my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# My Skill").unwrap();

        let agents_dir = dir.path().join(".ai/agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(agents_dir.join("my-agent.md"), "# My Agent").unwrap();

        // Only non-Cursor tools
        let mut tools = ToolsConfig::default();
        tools.set_tool("claude-code".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Symlink),
        });
        tools.set_tool("opencode".into(), ToolConfig {
            enabled: true,
            sync_strategy: Some(SyncStrategy::Symlink),
        });
        tools.set_tool("cursor".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });
        tools.set_tool("windsurf".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });
        tools.set_tool("codex".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });
        let config = AisyncConfig {
            schema_version: 1,
            defaults: DefaultsConfig { sync_strategy: SyncStrategy::Symlink },
            tools,
        };

        let report = SyncEngine::plan(&config, dir.path()).unwrap();

        for result in &report.results {
            let has_skill_agent_action = result.actions.iter().any(|a| {
                matches!(
                    a,
                    SyncAction::WriteSkillFile { .. }
                        | SyncAction::WriteAgentFile { .. }
                        | SyncAction::RemoveSkillDir { .. }
                )
            });
            assert!(
                !has_skill_agent_action,
                "non-Cursor tool {:?} should produce no skill/agent actions, got: {:?}",
                result.tool, result.actions
            );
        }
    }

    #[test]
    fn test_execute_write_skill_file_creates_nested_file() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        // Create canonical skill
        let skill_dir = dir.path().join(".ai/skills/my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# My Skill Content").unwrap();

        let config = cursor_only_config();
        let plan = SyncEngine::plan(&config, dir.path()).unwrap();
        let result = SyncEngine::execute(&plan, dir.path()).unwrap();
        assert!(!result.has_errors(), "execute should not produce errors: {:?}", result);

        // Verify the skill file was written at .cursor/skills/aisync-my-skill/SKILL.md
        let skill_output = dir.path().join(".cursor/skills/aisync-my-skill/SKILL.md");
        assert!(
            skill_output.exists(),
            "WriteSkillFile should create .cursor/skills/aisync-my-skill/SKILL.md"
        );
        let content = std::fs::read_to_string(&skill_output).unwrap();
        assert_eq!(content, "# My Skill Content");
    }

    #[test]
    fn test_toml_adapter_disabled_by_config() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions");

        // Create TOML adapter
        let adapters_dir = dir.path().join(".ai/adapters");
        std::fs::create_dir_all(&adapters_dir).unwrap();
        let toml = r#"
name = "aider"
display_name = "Aider"

[sync]
instruction_path = "AIDER.md"
"#;
        std::fs::write(adapters_dir.join("aider.toml"), toml).unwrap();

        // Explicitly disable the aider tool
        let mut tools = ToolsConfig::default();
        tools.set_tool("aider".into(), ToolConfig {
            enabled: false,
            sync_strategy: None,
        });
        let config = AisyncConfig {
            schema_version: 1,
            defaults: DefaultsConfig {
                sync_strategy: SyncStrategy::Symlink,
            },
            tools,
        };

        let report = SyncEngine::plan(&config, dir.path()).unwrap();
        let aider_result = report
            .results
            .iter()
            .find(|r| r.tool == ToolKind::Custom("aider".to_string()));
        assert!(aider_result.is_none(), "disabled aider should not appear in sync plan");
    }
}
