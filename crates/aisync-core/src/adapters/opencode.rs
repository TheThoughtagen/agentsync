use std::path::{Path, PathBuf};

use crate::adapter::{DetectionResult, OpenCodeAdapter, ToolAdapter};
use crate::config::SyncStrategy;
use crate::adapter::AdapterError;
use crate::types::{Confidence, DriftState, RuleFile, SyncAction, ToolKind, ToolSyncStatus, content_hash};

/// The relative symlink target path from project root to canonical instructions.
const CANONICAL_REL: &str = ".ai/instructions.md";
/// The tool-specific file name at project root.
const TOOL_FILE: &str = "AGENTS.md";

impl OpenCodeAdapter {
    /// Plan sync when conditionals are active (processed content differs from raw).
    fn plan_sync_with_conditionals(
        &self,
        link_path: &Path,
        processed_content: &str,
    ) -> Result<Vec<SyncAction>, AdapterError> {
        if let Ok(meta) = link_path.symlink_metadata() {
            if meta.file_type().is_symlink() {
                return Ok(vec![SyncAction::CreateFile {
                    path: link_path.to_path_buf(),
                    content: processed_content.to_string(),
                }]);
            }

            let existing = std::fs::read_to_string(link_path).unwrap_or_default();
            if existing == processed_content {
                return Ok(vec![]);
            }
            return Ok(vec![SyncAction::CreateFile {
                path: link_path.to_path_buf(),
                content: processed_content.to_string(),
            }]);
        }

        Ok(vec![SyncAction::CreateFile {
            path: link_path.to_path_buf(),
            content: processed_content.to_string(),
        }])
    }
}

impl ToolAdapter for OpenCodeAdapter {
    fn name(&self) -> ToolKind {
        ToolKind::OpenCode
    }

    fn display_name(&self) -> &str {
        "OpenCode"
    }

    fn native_instruction_path(&self) -> &str {
        "AGENTS.md"
    }

    fn conditional_tags(&self) -> &[&str] {
        &["opencode-only"]
    }

    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AdapterError> {
        let mut markers = Vec::new();
        let opencode_json = project_root.join("opencode.json");
        let agents_md = project_root.join(TOOL_FILE);

        let has_opencode = opencode_json.exists();
        let has_agents = agents_md.exists();

        if has_opencode {
            markers.push(opencode_json);
        }
        if has_agents {
            markers.push(agents_md);
        }

        let detected = has_opencode || has_agents;
        let confidence = if has_opencode {
            Confidence::High
        } else {
            Confidence::Medium
        };

        Ok(DetectionResult {
            tool: ToolKind::OpenCode,
            detected,
            confidence,
            markers_found: markers,
            version_hint: None,
        })
    }

    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AdapterError> {
        let path = project_root.join(TOOL_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path).map_err(|e| AdapterError::DetectionFailed(format!(
                "failed to read {}: {e}",
                path.display()
            )))?;
        Ok(Some(content))
    }

    fn plan_sync(
        &self,
        project_root: &Path,
        canonical_content: &str,
        strategy: SyncStrategy,
    ) -> Result<Vec<SyncAction>, AdapterError> {
        let link_path = project_root.join(TOOL_FILE);
        let target_rel = Path::new(CANONICAL_REL);

        // Determine whether conditionals changed the content
        let raw_path = project_root.join(CANONICAL_REL);
        let conditionals_active = match std::fs::read_to_string(&raw_path) {
            Ok(raw_content) => canonical_content != raw_content,
            Err(_) => false,
        };

        if conditionals_active || strategy == SyncStrategy::Copy {
            // Conditionals applied or copy strategy: write a regular file with content
            return self.plan_sync_with_conditionals(&link_path, canonical_content);
        }

        // No conditionals + symlink strategy: use symlink

        if link_path.exists() || link_path.symlink_metadata().is_ok() {
            if let Ok(meta) = link_path.symlink_metadata() {
                if meta.file_type().is_symlink() {
                    let current_target =
                        std::fs::read_link(&link_path).map_err(|e| AdapterError::DetectionFailed(format!(
                                "failed to read symlink: {e}"
                            )))?;
                    if current_target == target_rel {
                        return Ok(vec![]);
                    }
                    return Ok(vec![SyncAction::RemoveAndRelink {
                        link: link_path,
                        target: target_rel.to_path_buf(),
                    }]);
                }
                return Ok(vec![SyncAction::SkipExistingFile {
                    path: link_path,
                    reason: format!("{} is a regular file, not managed by aisync", TOOL_FILE),
                }]);
            }
        }

        Ok(vec![SyncAction::CreateSymlink {
            link: link_path,
            target: target_rel.to_path_buf(),
        }])
    }

    fn plan_memory_sync(
        &self,
        project_root: &Path,
        memory_files: &[PathBuf],
    ) -> Result<Vec<SyncAction>, AdapterError> {
        if memory_files.is_empty() {
            return Ok(vec![]);
        }

        let references: Vec<String> = memory_files
            .iter()
            .filter_map(|path| {
                let name = path.file_stem()?.to_string_lossy().to_string();
                let rel = path
                    .strip_prefix(project_root)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| format!(".ai/memory/{}.md", name));
                Some(format!("- [{}]({})", name, rel))
            })
            .collect();

        Ok(vec![SyncAction::UpdateMemoryReferences {
            path: project_root.join(TOOL_FILE),
            references,
            marker_start: "<!-- aisync:memory -->".to_string(),
            marker_end: "<!-- /aisync:memory -->".to_string(),
        }])
    }

    fn plan_rules_sync(
        &self,
        project_root: &Path,
        rules: &[RuleFile],
    ) -> Result<Vec<SyncAction>, AdapterError> {
        crate::adapters::plan_single_file_rules_sync(
            project_root.join(self.native_instruction_path()),
            rules,
        )
    }

    fn translate_hooks(
        &self,
        hooks: &crate::types::HooksConfig,
    ) -> Result<crate::types::HookTranslation, AdapterError> {
        fn opencode_event_name(event: &str) -> Option<&'static str> {
            match event {
                "PreToolUse" => Some("tool.execute.before"),
                "PostToolUse" => Some("tool.execute.after"),
                "Stop" => Some("session.idle"),
                _ => None,
            }
        }

        let mut lines = vec![
            "// OpenCode plugin — generated by aisync sync. Do not edit.".to_string(),
            "// See: https://opencode.ai/docs/plugins/".to_string(),
            "export const AisyncHooks = async ({ $ }) => {".to_string(),
            "  return {".to_string(),
        ];

        for (event, groups) in &hooks.events {
            if let Some(oc_event) = opencode_event_name(event) {
                lines.push(format!("    \"{}\": async (input, output) => {{", oc_event));
                for group in groups {
                    for hook in &group.hooks {
                        // Strip Claude Code env vars for project-relative paths
                        let translated = hook.command
                            .replace("$CLAUDE_PROJECT_DIR/", "")
                            .replace("${CLAUDE_PROJECT_DIR}/", "");
                        // Escape any single quotes in the command
                        let escaped = translated.replace('\'', "'\\''");
                        lines.push(format!("      await $`{escaped}`;"));
                    }
                }
                lines.push("    },".to_string());
            } else {
                lines.push(format!(
                    "    // Unsupported: {} (no OpenCode equivalent)",
                    event
                ));
            }
        }

        lines.push("  };".to_string());
        lines.push("};".to_string());

        let content = lines.join("\n");
        Ok(crate::types::HookTranslation::Supported {
            tool: ToolKind::OpenCode,
            content,
            format: "js".to_string(),
        })
    }

    fn plan_mcp_sync(
        &self,
        _project_root: &Path,
        _mcp_config: &crate::types::McpConfig,
    ) -> Result<Vec<SyncAction>, AdapterError> {
        Ok(vec![SyncAction::WarnUnsupportedDimension {
            tool: ToolKind::OpenCode,
            dimension: "mcp".into(),
            reason: "OpenCode does not support MCP server configuration".into(),
        }])
    }

    fn sync_status(
        &self,
        project_root: &Path,
        canonical_hash: &str,
        strategy: SyncStrategy,
    ) -> Result<ToolSyncStatus, AdapterError> {
        let path = project_root.join(TOOL_FILE);

        let meta = match path.symlink_metadata() {
            Ok(m) => m,
            Err(_) => {
                return Ok(ToolSyncStatus {
                    tool: ToolKind::OpenCode,
                    strategy,
                    drift: DriftState::Missing,
                    details: None,
                });
            }
        };

        if meta.file_type().is_symlink() {
            if !path.exists() {
                return Ok(ToolSyncStatus {
                    tool: ToolKind::OpenCode,
                    strategy,
                    drift: DriftState::DanglingSymlink,
                    details: Some("symlink target does not exist".to_string()),
                });
            }

            let content = std::fs::read(&path).map_err(|e| AdapterError::DetectionFailed(format!(
                    "failed to read {}: {e}",
                    path.display()
                )))?;
            let hash = content_hash(&content);
            if hash == canonical_hash {
                return Ok(ToolSyncStatus {
                    tool: ToolKind::OpenCode,
                    strategy,
                    drift: DriftState::InSync,
                    details: None,
                });
            }
            return Ok(ToolSyncStatus {
                tool: ToolKind::OpenCode,
                strategy,
                drift: DriftState::Drifted {
                    reason: "content hash mismatch".to_string(),
                },
                details: Some(format!("expected {canonical_hash}, got {hash}")),
            });
        }

        // Regular file -- hash and compare
        let content = std::fs::read(&path).map_err(|e| AdapterError::DetectionFailed(format!(
                "failed to read {}: {e}",
                path.display()
            )))?;
        let hash = content_hash(&content);
        if strategy == SyncStrategy::Copy {
            let drift = if hash == canonical_hash {
                DriftState::InSync
            } else {
                DriftState::Drifted {
                    reason: "content hash mismatch".to_string(),
                }
            };
            return Ok(ToolSyncStatus {
                tool: ToolKind::OpenCode,
                strategy,
                drift,
                details: if hash != canonical_hash {
                    Some(format!("expected {canonical_hash}, got {hash}"))
                } else {
                    None
                },
            });
        }
        // Symlink strategy but found regular file
        let drift = if hash == canonical_hash {
            DriftState::Drifted {
                reason: "file is not a symlink (wrong strategy)".to_string(),
            }
        } else {
            DriftState::Drifted {
                reason: "content hash mismatch and not a symlink".to_string(),
            }
        };
        Ok(ToolSyncStatus {
            tool: ToolKind::OpenCode,
            strategy,
            drift,
            details: Some(format!("regular file, hash: {hash}")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_name_returns_opencode() {
        assert_eq!(OpenCodeAdapter.name(), ToolKind::OpenCode);
    }

    #[test]
    fn test_detects_opencode_json() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("opencode.json"), "{}").unwrap();

        let result = OpenCodeAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_agents_md_only_medium_confidence() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "# Agents").unwrap();

        let result = OpenCodeAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::Medium);
    }

    #[test]
    fn test_both_markers_high_confidence() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("opencode.json"), "{}").unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "# Agents").unwrap();

        let result = OpenCodeAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
        assert_eq!(result.markers_found.len(), 2);
    }

    #[test]
    fn test_not_detected_empty_dir() {
        let dir = TempDir::new().unwrap();

        let result = OpenCodeAdapter.detect(dir.path()).unwrap();
        assert!(!result.detected);
        assert!(result.markers_found.is_empty());
    }

    // --- read_instructions tests ---

    #[test]
    fn test_read_instructions_reads_content() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "# Agent Instructions").unwrap();

        let content = OpenCodeAdapter.read_instructions(dir.path()).unwrap();
        assert_eq!(content, Some("# Agent Instructions".to_string()));
    }

    #[test]
    fn test_read_instructions_returns_none_when_missing() {
        let dir = TempDir::new().unwrap();

        let content = OpenCodeAdapter.read_instructions(dir.path()).unwrap();
        assert_eq!(content, None);
    }

    // --- plan_sync tests ---

    #[test]
    fn test_plan_sync_creates_symlink_when_missing() {
        let dir = TempDir::new().unwrap();

        let actions = OpenCodeAdapter
            .plan_sync(dir.path(), "content", SyncStrategy::Symlink)
            .unwrap();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::CreateSymlink { link, target } => {
                assert_eq!(link, &dir.path().join("AGENTS.md"));
                assert_eq!(target, Path::new(".ai/instructions.md"));
            }
            other => panic!("expected CreateSymlink, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_plan_sync_correct_symlink_returns_empty() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("instructions.md"), "content").unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                Path::new(".ai/instructions.md"),
                dir.path().join("AGENTS.md"),
            )
            .unwrap();
        }

        let actions = OpenCodeAdapter
            .plan_sync(dir.path(), "content", SyncStrategy::Symlink)
            .unwrap();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_plan_sync_regular_file_returns_skip() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "user content").unwrap();

        let actions = OpenCodeAdapter
            .plan_sync(dir.path(), "content", SyncStrategy::Symlink)
            .unwrap();
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], SyncAction::SkipExistingFile { .. }));
    }

    // --- sync_status tests ---

    #[test]
    fn test_sync_status_missing() {
        let dir = TempDir::new().unwrap();

        let status = OpenCodeAdapter
            .sync_status(dir.path(), "abc123", SyncStrategy::Symlink)
            .unwrap();
        assert_eq!(status.tool, ToolKind::OpenCode);
        assert_eq!(status.drift, DriftState::Missing);
    }

    #[cfg(unix)]
    #[test]
    fn test_sync_status_in_sync() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir(&ai_dir).unwrap();
        let content = "canonical content";
        std::fs::write(ai_dir.join("instructions.md"), content).unwrap();
        let hash = content_hash(content.as_bytes());

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                Path::new(".ai/instructions.md"),
                dir.path().join("AGENTS.md"),
            )
            .unwrap();
        }

        let status = OpenCodeAdapter
            .sync_status(dir.path(), &hash, SyncStrategy::Symlink)
            .unwrap();
        assert_eq!(status.drift, DriftState::InSync);
    }

    // --- plan_memory_sync tests ---

    #[cfg(unix)]
    #[test]
    fn test_plan_memory_sync_returns_update_memory_references() {
        let dir = TempDir::new().unwrap();
        let memory_dir = dir.path().join(".ai/memory");
        std::fs::create_dir_all(&memory_dir).unwrap();
        std::fs::write(memory_dir.join("debugging.md"), "# Debugging").unwrap();
        std::fs::write(memory_dir.join("patterns.md"), "# Patterns").unwrap();

        let memory_files = vec![
            memory_dir.join("debugging.md"),
            memory_dir.join("patterns.md"),
        ];
        let actions = OpenCodeAdapter
            .plan_memory_sync(dir.path(), &memory_files)
            .unwrap();

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::UpdateMemoryReferences {
                path,
                references,
                marker_start,
                marker_end,
            } => {
                assert!(path.ends_with("AGENTS.md"));
                assert_eq!(references.len(), 2);
                assert!(references[0].contains(".ai/memory/debugging.md"));
                assert!(references[1].contains(".ai/memory/patterns.md"));
                assert_eq!(marker_start, "<!-- aisync:memory -->");
                assert_eq!(marker_end, "<!-- /aisync:memory -->");
            }
            other => panic!("expected UpdateMemoryReferences, got {other:?}"),
        }
    }

    #[test]
    fn test_plan_memory_sync_empty_files_returns_empty() {
        let dir = TempDir::new().unwrap();

        let actions = OpenCodeAdapter.plan_memory_sync(dir.path(), &[]).unwrap();
        assert!(actions.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn test_sync_status_dangling_symlink() {
        let dir = TempDir::new().unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                Path::new(".ai/instructions.md"),
                dir.path().join("AGENTS.md"),
            )
            .unwrap();
        }

        let status = OpenCodeAdapter
            .sync_status(dir.path(), "abc123", SyncStrategy::Symlink)
            .unwrap();
        assert_eq!(status.drift, DriftState::DanglingSymlink);
    }

    // --- plan_rules_sync tests ---

    #[test]
    fn test_plan_rules_sync_returns_update_memory_references() {
        use crate::types::RuleFile;
        use crate::types::RuleMetadata;
        use std::path::PathBuf;

        let dir = TempDir::new().unwrap();
        let rules = vec![RuleFile {
            name: "coding-standards".to_string(),
            metadata: RuleMetadata {
                description: Some("Coding standards".to_string()),
                globs: vec!["*.rs".to_string()],
                always_apply: true,
            },
            content: "Use snake_case for variables.".to_string(),
            source_path: PathBuf::from(".ai/rules/coding-standards.md"),
        }];

        let actions = OpenCodeAdapter
            .plan_rules_sync(dir.path(), &rules)
            .unwrap();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::UpdateMemoryReferences {
                path,
                references,
                marker_start,
                marker_end,
            } => {
                assert_eq!(path, &dir.path().join("AGENTS.md"));
                assert_eq!(marker_start, "<!-- aisync:rules -->");
                assert_eq!(marker_end, "<!-- /aisync:rules -->");
                assert_eq!(references.len(), 1);
                assert!(references[0].contains("## Rule: coding-standards"));
                assert!(references[0].contains("Use snake_case for variables."));
            }
            other => panic!("expected UpdateMemoryReferences, got {other:?}"),
        }
    }

    #[test]
    fn test_plan_rules_sync_empty_rules_returns_empty() {
        let dir = TempDir::new().unwrap();
        let actions = OpenCodeAdapter.plan_rules_sync(dir.path(), &[]).unwrap();
        assert!(actions.is_empty());
    }

    // --- translate_hooks tests ---

    #[test]
    fn test_translate_hooks_produces_js_plugin_stub() {
        use crate::types::{HookGroup, HookHandler, HookTranslation, HooksConfig};
        use std::collections::BTreeMap;

        let mut events = BTreeMap::new();
        events.insert(
            "PreToolUse".to_string(),
            vec![HookGroup {
                matcher: Some("Edit".to_string()),
                hooks: vec![HookHandler {
                    hook_type: "command".to_string(),
                    command: "npm run lint".to_string(),
                    timeout: Some(10000),
                }],
            }],
        );
        events.insert(
            "PostToolUse".to_string(),
            vec![HookGroup {
                matcher: None,
                hooks: vec![HookHandler {
                    hook_type: "command".to_string(),
                    command: "cargo fmt".to_string(),
                    timeout: None,
                }],
            }],
        );
        let config = HooksConfig { events };

        let result = OpenCodeAdapter.translate_hooks(&config).unwrap();
        match result {
            HookTranslation::Supported {
                tool,
                content,
                format,
            } => {
                assert_eq!(tool, ToolKind::OpenCode);
                assert_eq!(format, "js");
                assert!(content.contains("export const AisyncHooks"), "should use ESM named export");
                assert!(!content.contains("module.exports"), "should not use CommonJS");
                assert!(content.contains("tool.execute.before"));
                assert!(content.contains("tool.execute.after"));
                assert!(content.contains("npm run lint"));
                assert!(content.contains("cargo fmt"));
            }
            other => panic!("expected Supported, got {other:?}"),
        }
    }

    #[test]
    fn test_translate_hooks_skips_unsupported_events() {
        use crate::types::{HookGroup, HookHandler, HookTranslation, HooksConfig};
        use std::collections::BTreeMap;

        let mut events = BTreeMap::new();
        events.insert(
            "Notification".to_string(),
            vec![HookGroup {
                matcher: None,
                hooks: vec![HookHandler {
                    hook_type: "command".to_string(),
                    command: "notify-send done".to_string(),
                    timeout: None,
                }],
            }],
        );
        events.insert(
            "SubagentStop".to_string(),
            vec![HookGroup {
                matcher: None,
                hooks: vec![HookHandler {
                    hook_type: "command".to_string(),
                    command: "echo subagent".to_string(),
                    timeout: None,
                }],
            }],
        );
        let config = HooksConfig { events };

        let result = OpenCodeAdapter.translate_hooks(&config).unwrap();
        match result {
            HookTranslation::Supported { content, .. } => {
                assert!(content.contains("Unsupported: Notification"));
                assert!(content.contains("Unsupported: SubagentStop"));
            }
            other => panic!("expected Supported, got {other:?}"),
        }
    }

    #[test]
    fn test_translate_hooks_maps_stop_event() {
        use crate::types::{HookGroup, HookHandler, HookTranslation, HooksConfig};
        use std::collections::BTreeMap;

        let mut events = BTreeMap::new();
        events.insert(
            "Stop".to_string(),
            vec![HookGroup {
                matcher: None,
                hooks: vec![HookHandler {
                    hook_type: "command".to_string(),
                    command: "echo stopped".to_string(),
                    timeout: None,
                }],
            }],
        );
        let config = HooksConfig { events };

        let result = OpenCodeAdapter.translate_hooks(&config).unwrap();
        match result {
            HookTranslation::Supported { content, .. } => {
                assert!(content.contains("session.idle"));
            }
            other => panic!("expected Supported, got {other:?}"),
        }
    }

    // --- plan_mcp_sync tests ---

    #[test]
    fn test_plan_mcp_sync_returns_unsupported_warning() {
        use crate::types::{McpConfig, McpServer};
        use std::collections::BTreeMap;

        let dir = TempDir::new().unwrap();
        let config = McpConfig {
            servers: BTreeMap::from([(
                "fs".to_string(),
                McpServer {
                    command: "npx".to_string(),
                    args: vec![],
                    env: BTreeMap::new(),
                },
            )]),
        };

        let actions = OpenCodeAdapter
            .plan_mcp_sync(dir.path(), &config)
            .unwrap();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::WarnUnsupportedDimension {
                tool,
                dimension,
                reason,
            } => {
                assert_eq!(*tool, ToolKind::OpenCode);
                assert_eq!(dimension, "mcp");
                assert!(reason.contains("does not support MCP"));
            }
            other => panic!("expected WarnUnsupportedDimension, got {other:?}"),
        }
    }
}
