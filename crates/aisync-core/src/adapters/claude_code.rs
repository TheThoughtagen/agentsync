use std::path::{Path, PathBuf};

use crate::adapter::{ClaudeCodeAdapter, DetectionResult, ToolAdapter};
use crate::config::SyncStrategy;
use crate::error::AisyncError;
use crate::memory::MemoryEngine;
use crate::types::{content_hash, Confidence, DriftState, SyncAction, ToolKind, ToolSyncStatus};

/// The relative symlink target path from project root to canonical instructions.
const CANONICAL_REL: &str = ".ai/instructions.md";
/// The tool-specific file name at project root.
const TOOL_FILE: &str = "CLAUDE.md";

impl ToolAdapter for ClaudeCodeAdapter {
    fn name(&self) -> ToolKind {
        ToolKind::ClaudeCode
    }

    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError> {
        let mut markers = Vec::new();
        let claude_md = project_root.join(TOOL_FILE);
        let claude_dir = project_root.join(".claude");

        if claude_md.exists() {
            markers.push(claude_md);
        }
        if claude_dir.is_dir() {
            markers.push(claude_dir);
        }

        let detected = !markers.is_empty();
        Ok(DetectionResult {
            tool: ToolKind::ClaudeCode,
            detected,
            confidence: Confidence::High,
            markers_found: markers,
            version_hint: None,
        })
    }

    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AisyncError> {
        let path = project_root.join(TOOL_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path).map_err(|e| AisyncError::Adapter {
            tool: "claude-code".to_string(),
            source: crate::error::AdapterError::DetectionFailed(format!(
                "failed to read {}: {e}",
                path.display()
            )),
        })?;
        Ok(Some(content))
    }

    fn plan_sync(
        &self,
        project_root: &Path,
        _canonical_content: &str,
        _strategy: SyncStrategy,
    ) -> Result<Vec<SyncAction>, AisyncError> {
        let link_path = project_root.join(TOOL_FILE);
        let target_rel = Path::new(CANONICAL_REL);

        // On Windows, always use Copy strategy (INST-04) -- handled by caller.
        // Here we handle symlink logic for Unix.

        if link_path.exists() || link_path.symlink_metadata().is_ok() {
            // File or symlink exists
            if let Ok(meta) = link_path.symlink_metadata() {
                if meta.file_type().is_symlink() {
                    // Check where the symlink points
                    let current_target = std::fs::read_link(&link_path).map_err(|e| {
                        AisyncError::Adapter {
                            tool: "claude-code".to_string(),
                            source: crate::error::AdapterError::DetectionFailed(format!(
                                "failed to read symlink: {e}"
                            )),
                        }
                    })?;
                    if current_target == target_rel {
                        // Already correct symlink -- idempotent, no action needed
                        return Ok(vec![]);
                    }
                    // Symlink points elsewhere -- remove and relink
                    return Ok(vec![SyncAction::RemoveAndRelink {
                        link: link_path,
                        target: target_rel.to_path_buf(),
                    }]);
                }
                // Regular file -- skip (user must decide interactively)
                return Ok(vec![SyncAction::SkipExistingFile {
                    path: link_path,
                    reason: format!(
                        "{} is a regular file, not managed by aisync",
                        TOOL_FILE
                    ),
                }]);
            }
        }

        // File doesn't exist -- create symlink
        Ok(vec![SyncAction::CreateSymlink {
            link: link_path,
            target: target_rel.to_path_buf(),
        }])
    }

    fn plan_memory_sync(
        &self,
        project_root: &Path,
        memory_files: &[PathBuf],
    ) -> Result<Vec<SyncAction>, AisyncError> {
        if memory_files.is_empty() {
            return Ok(vec![]);
        }

        let claude_memory = MemoryEngine::claude_memory_path(project_root)?;
        let target = project_root.join(".ai/memory");

        // Check if the symlink already exists and is correct
        if claude_memory.symlink_metadata().is_ok() {
            let meta = claude_memory.symlink_metadata().map_err(|e| {
                AisyncError::Adapter {
                    tool: "claude-code".to_string(),
                    source: crate::error::AdapterError::DetectionFailed(format!(
                        "failed to read symlink metadata: {e}"
                    )),
                }
            })?;

            if meta.file_type().is_symlink() {
                // Check if it points to the right target
                if let Ok(current_target) = std::fs::read_link(&claude_memory) {
                    // Canonicalize both for comparison
                    let canonical_target = target.canonicalize().unwrap_or_else(|_| target.clone());
                    let canonical_current = if current_target.is_absolute() {
                        current_target.canonicalize().unwrap_or(current_target)
                    } else {
                        // Resolve relative to symlink parent
                        if let Some(parent) = claude_memory.parent() {
                            parent.join(&current_target).canonicalize().unwrap_or(current_target)
                        } else {
                            current_target
                        }
                    };
                    if canonical_current == canonical_target {
                        return Ok(vec![]);
                    }
                }
            }

            if meta.is_dir() {
                // It's a real directory (not a symlink) with content
                return Ok(vec![SyncAction::SkipExistingFile {
                    path: claude_memory,
                    reason: "existing Claude memory found, run `aisync memory import claude` first".to_string(),
                }]);
            }
        }

        Ok(vec![SyncAction::CreateMemorySymlink {
            link: claude_memory,
            target,
        }])
    }

    fn translate_hooks(
        &self,
        hooks: &crate::types::HooksConfig,
    ) -> Result<crate::types::HookTranslation, AisyncError> {
        let mut hooks_obj = serde_json::Map::new();
        for (event, groups) in &hooks.events {
            let groups_json: Vec<serde_json::Value> = groups.iter().map(|g| {
                let mut obj = serde_json::Map::new();
                if let Some(matcher) = &g.matcher {
                    obj.insert("matcher".into(), serde_json::Value::String(matcher.clone()));
                }
                let hooks_arr: Vec<serde_json::Value> = g.hooks.iter().map(|h| {
                    let mut hook_obj = serde_json::Map::new();
                    hook_obj.insert("type".into(), serde_json::Value::String(h.hook_type.clone()));
                    hook_obj.insert("command".into(), serde_json::Value::String(h.command.clone()));
                    if let Some(timeout) = h.timeout {
                        // Convert milliseconds to seconds for Claude Code
                        hook_obj.insert("timeout".into(), serde_json::json!(timeout / 1000));
                    }
                    serde_json::Value::Object(hook_obj)
                }).collect();
                obj.insert("hooks".into(), serde_json::Value::Array(hooks_arr));
                serde_json::Value::Object(obj)
            }).collect();
            hooks_obj.insert(event.clone(), serde_json::Value::Array(groups_json));
        }
        let json = serde_json::json!({ "hooks": hooks_obj });
        let content = serde_json::to_string_pretty(&json)
            .map_err(|e| AisyncError::Adapter {
                tool: "claude-code".to_string(),
                source: crate::error::AdapterError::DetectionFailed(format!("JSON serialization failed: {e}")),
            })?;
        Ok(crate::types::HookTranslation::Supported {
            tool: ToolKind::ClaudeCode,
            content,
            format: "json".to_string(),
        })
    }

    fn sync_status(
        &self,
        project_root: &Path,
        canonical_hash: &str,
    ) -> Result<ToolSyncStatus, AisyncError> {
        let path = project_root.join(TOOL_FILE);

        // Check symlink metadata (doesn't follow symlinks)
        let meta = match path.symlink_metadata() {
            Ok(m) => m,
            Err(_) => {
                return Ok(ToolSyncStatus {
                    tool: ToolKind::ClaudeCode,
                    strategy: SyncStrategy::Symlink,
                    drift: DriftState::Missing,
                    details: None,
                });
            }
        };

        if meta.file_type().is_symlink() {
            // Check if target exists (follow the symlink)
            if !path.exists() {
                return Ok(ToolSyncStatus {
                    tool: ToolKind::ClaudeCode,
                    strategy: SyncStrategy::Symlink,
                    drift: DriftState::DanglingSymlink,
                    details: Some("symlink target does not exist".to_string()),
                });
            }

            // Read content via symlink and hash
            let content = std::fs::read(&path).map_err(|e| AisyncError::Adapter {
                tool: "claude-code".to_string(),
                source: crate::error::AdapterError::DetectionFailed(format!(
                    "failed to read {}: {e}",
                    path.display()
                )),
            })?;
            let hash = content_hash(&content);
            if hash == canonical_hash {
                return Ok(ToolSyncStatus {
                    tool: ToolKind::ClaudeCode,
                    strategy: SyncStrategy::Symlink,
                    drift: DriftState::InSync,
                    details: None,
                });
            }
            return Ok(ToolSyncStatus {
                tool: ToolKind::ClaudeCode,
                strategy: SyncStrategy::Symlink,
                drift: DriftState::Drifted {
                    reason: "content hash mismatch".to_string(),
                },
                details: Some(format!("expected {canonical_hash}, got {hash}")),
            });
        }

        // Regular file -- hash and compare
        let content = std::fs::read(&path).map_err(|e| AisyncError::Adapter {
            tool: "claude-code".to_string(),
            source: crate::error::AdapterError::DetectionFailed(format!(
                "failed to read {}: {e}",
                path.display()
            )),
        })?;
        let hash = content_hash(&content);
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
            tool: ToolKind::ClaudeCode,
            strategy: SyncStrategy::Symlink,
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
    fn test_name_returns_claude_code() {
        let adapter = ClaudeCodeAdapter;
        assert_eq!(adapter.name(), ToolKind::ClaudeCode);
    }

    #[test]
    fn test_detects_claude_md() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();

        let result = ClaudeCodeAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
        assert!(result.markers_found.iter().any(|p| p.ends_with("CLAUDE.md")));
    }

    #[test]
    fn test_detects_claude_dir() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".claude")).unwrap();

        let result = ClaudeCodeAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
        assert!(result.markers_found.iter().any(|p| p.ends_with(".claude")));
    }

    #[test]
    fn test_detects_both_markers() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();
        std::fs::create_dir(dir.path().join(".claude")).unwrap();

        let result = ClaudeCodeAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
        assert_eq!(result.markers_found.len(), 2);
    }

    #[test]
    fn test_not_detected_empty_dir() {
        let dir = TempDir::new().unwrap();

        let result = ClaudeCodeAdapter.detect(dir.path()).unwrap();
        assert!(!result.detected);
        assert!(result.markers_found.is_empty());
    }

    // --- read_instructions tests ---

    #[test]
    fn test_read_instructions_reads_content() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# My Instructions").unwrap();

        let content = ClaudeCodeAdapter.read_instructions(dir.path()).unwrap();
        assert_eq!(content, Some("# My Instructions".to_string()));
    }

    #[test]
    fn test_read_instructions_follows_symlinks() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("instructions.md"), "canonical content").unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                Path::new(".ai/instructions.md"),
                dir.path().join("CLAUDE.md"),
            )
            .unwrap();
        }

        let content = ClaudeCodeAdapter.read_instructions(dir.path()).unwrap();
        assert_eq!(content, Some("canonical content".to_string()));
    }

    #[test]
    fn test_read_instructions_returns_none_when_missing() {
        let dir = TempDir::new().unwrap();

        let content = ClaudeCodeAdapter.read_instructions(dir.path()).unwrap();
        assert_eq!(content, None);
    }

    // --- plan_sync tests ---

    #[test]
    fn test_plan_sync_creates_symlink_when_missing() {
        let dir = TempDir::new().unwrap();

        let actions = ClaudeCodeAdapter
            .plan_sync(dir.path(), "content", SyncStrategy::Symlink)
            .unwrap();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::CreateSymlink { link, target } => {
                assert_eq!(link, &dir.path().join("CLAUDE.md"));
                assert_eq!(target, Path::new(".ai/instructions.md"));
            }
            other => panic!("expected CreateSymlink, got {other:?}"),
        }
    }

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
                dir.path().join("CLAUDE.md"),
            )
            .unwrap();
        }

        let actions = ClaudeCodeAdapter
            .plan_sync(dir.path(), "content", SyncStrategy::Symlink)
            .unwrap();
        assert!(actions.is_empty(), "expected no actions for correct symlink");
    }

    #[test]
    fn test_plan_sync_regular_file_returns_skip() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "user content").unwrap();

        let actions = ClaudeCodeAdapter
            .plan_sync(dir.path(), "content", SyncStrategy::Symlink)
            .unwrap();
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], SyncAction::SkipExistingFile { .. }));
    }

    #[test]
    fn test_plan_sync_wrong_symlink_returns_relink() {
        let dir = TempDir::new().unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                Path::new("wrong/target.md"),
                dir.path().join("CLAUDE.md"),
            )
            .unwrap();
        }

        let actions = ClaudeCodeAdapter
            .plan_sync(dir.path(), "content", SyncStrategy::Symlink)
            .unwrap();
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], SyncAction::RemoveAndRelink { .. }));
    }

    // --- sync_status tests ---

    #[test]
    fn test_sync_status_missing() {
        let dir = TempDir::new().unwrap();

        let status = ClaudeCodeAdapter.sync_status(dir.path(), "abc123").unwrap();
        assert_eq!(status.tool, ToolKind::ClaudeCode);
        assert_eq!(status.drift, DriftState::Missing);
    }

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
                dir.path().join("CLAUDE.md"),
            )
            .unwrap();
        }

        let status = ClaudeCodeAdapter.sync_status(dir.path(), &hash).unwrap();
        assert_eq!(status.drift, DriftState::InSync);
    }

    #[test]
    fn test_sync_status_dangling_symlink() {
        let dir = TempDir::new().unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                Path::new(".ai/instructions.md"),
                dir.path().join("CLAUDE.md"),
            )
            .unwrap();
        }

        let status = ClaudeCodeAdapter.sync_status(dir.path(), "abc123").unwrap();
        assert_eq!(status.drift, DriftState::DanglingSymlink);
    }

    // --- plan_memory_sync tests ---

    #[test]
    fn test_plan_memory_sync_creates_symlink_when_memory_exists() {
        let dir = TempDir::new().unwrap();
        let memory_dir = dir.path().join(".ai/memory");
        std::fs::create_dir_all(&memory_dir).unwrap();
        std::fs::write(memory_dir.join("debugging.md"), "# Debugging").unwrap();

        let memory_files = vec![memory_dir.join("debugging.md")];
        let actions = ClaudeCodeAdapter
            .plan_memory_sync(dir.path(), &memory_files)
            .unwrap();

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::CreateMemorySymlink { link, target } => {
                assert!(link.to_string_lossy().contains(".claude/projects/"));
                assert!(link.to_string_lossy().ends_with("/memory"));
                assert_eq!(target, &dir.path().join(".ai/memory"));
            }
            other => panic!("expected CreateMemorySymlink, got {other:?}"),
        }
    }

    #[test]
    fn test_plan_memory_sync_empty_files_returns_empty() {
        let dir = TempDir::new().unwrap();

        let actions = ClaudeCodeAdapter
            .plan_memory_sync(dir.path(), &[])
            .unwrap();
        assert!(actions.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn test_plan_memory_sync_idempotent_correct_symlink() {
        let dir = TempDir::new().unwrap();
        let memory_dir = dir.path().join(".ai/memory");
        std::fs::create_dir_all(&memory_dir).unwrap();
        std::fs::write(memory_dir.join("topic.md"), "# Topic").unwrap();

        // Create the Claude memory symlink manually
        let claude_memory = crate::MemoryEngine::claude_memory_path(
            &dir.path().canonicalize().unwrap(),
        ).unwrap();
        if let Some(parent) = claude_memory.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let target = dir.path().canonicalize().unwrap().join(".ai/memory");
        std::os::unix::fs::symlink(&target, &claude_memory).unwrap();

        let memory_files = vec![memory_dir.join("topic.md")];
        let actions = ClaudeCodeAdapter
            .plan_memory_sync(&dir.path().canonicalize().unwrap(), &memory_files)
            .unwrap();
        assert!(actions.is_empty(), "expected no actions for existing correct symlink, got {:?}", actions);
    }

    #[test]
    fn test_plan_memory_sync_existing_dir_warns() {
        let dir = TempDir::new().unwrap();
        let memory_dir = dir.path().join(".ai/memory");
        std::fs::create_dir_all(&memory_dir).unwrap();
        std::fs::write(memory_dir.join("topic.md"), "# Topic").unwrap();

        // Create a real (non-symlink) memory directory at Claude's path
        let claude_memory = crate::MemoryEngine::claude_memory_path(
            &dir.path().canonicalize().unwrap(),
        ).unwrap();
        std::fs::create_dir_all(&claude_memory).unwrap();
        std::fs::write(claude_memory.join("existing.md"), "# Existing").unwrap();

        let memory_files = vec![memory_dir.join("topic.md")];
        let actions = ClaudeCodeAdapter
            .plan_memory_sync(&dir.path().canonicalize().unwrap(), &memory_files)
            .unwrap();

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::SkipExistingFile { reason, .. } => {
                assert!(reason.contains("import"), "reason should mention import: {reason}");
            }
            other => panic!("expected SkipExistingFile, got {other:?}"),
        }
    }

    // --- conditional content tests ---

    #[cfg(unix)]
    #[test]
    fn test_plan_sync_no_conditionals_creates_symlink() {
        // When canonical_content equals raw file content (no conditionals), plan_sync returns CreateSymlink
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        let raw_content = "# Instructions\n\nShared content\n";
        std::fs::write(ai_dir.join("instructions.md"), raw_content).unwrap();

        // canonical_content == raw content (no conditionals applied)
        let actions = ClaudeCodeAdapter
            .plan_sync(dir.path(), raw_content, SyncStrategy::Symlink)
            .unwrap();
        assert_eq!(actions.len(), 1);
        assert!(
            matches!(&actions[0], SyncAction::CreateSymlink { .. }),
            "expected CreateSymlink when no conditionals, got {:?}",
            actions[0]
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_plan_sync_conditionals_applied_creates_file() {
        // When canonical_content differs from raw file content (conditionals applied),
        // plan_sync returns CreateFile with the processed content
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        let raw_content = "# Instructions\n\n<!-- aisync:cursor-only -->\nCursor stuff\n<!-- /aisync:cursor-only -->\n\nShared\n";
        std::fs::write(ai_dir.join("instructions.md"), raw_content).unwrap();

        // Processed content for Claude (cursor-only section stripped)
        let processed = "# Instructions\n\n\nShared\n";

        let actions = ClaudeCodeAdapter
            .plan_sync(dir.path(), processed, SyncStrategy::Symlink)
            .unwrap();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::CreateFile { path, content } => {
                assert_eq!(path, &dir.path().join("CLAUDE.md"));
                assert_eq!(content, processed);
            }
            other => panic!("expected CreateFile, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_plan_sync_conditionals_idempotent_correct_file() {
        // When CLAUDE.md is already a regular file with correct processed content, returns empty
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        let raw_content = "# Instructions\n\n<!-- aisync:cursor-only -->\nCursor stuff\n<!-- /aisync:cursor-only -->\n\nShared\n";
        std::fs::write(ai_dir.join("instructions.md"), raw_content).unwrap();

        let processed = "# Instructions\n\n\nShared\n";
        // Write CLAUDE.md as a regular file with processed content
        std::fs::write(dir.path().join("CLAUDE.md"), processed).unwrap();

        let actions = ClaudeCodeAdapter
            .plan_sync(dir.path(), processed, SyncStrategy::Symlink)
            .unwrap();
        assert!(
            actions.is_empty(),
            "expected no actions when CLAUDE.md already has correct processed content, got {:?}",
            actions
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_plan_sync_symlink_but_conditionals_now_apply() {
        // When CLAUDE.md is a symlink but conditionals now apply, should remove + CreateFile
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        let raw_content = "# Instructions\n\n<!-- aisync:cursor-only -->\nCursor stuff\n<!-- /aisync:cursor-only -->\n\nShared\n";
        std::fs::write(ai_dir.join("instructions.md"), raw_content).unwrap();

        // Create existing symlink
        std::os::unix::fs::symlink(
            Path::new(".ai/instructions.md"),
            dir.path().join("CLAUDE.md"),
        )
        .unwrap();

        let processed = "# Instructions\n\n\nShared\n";
        let actions = ClaudeCodeAdapter
            .plan_sync(dir.path(), processed, SyncStrategy::Symlink)
            .unwrap();

        // Should have actions to handle the transition from symlink to file
        assert!(!actions.is_empty(), "expected actions to transition from symlink to file");
        // Should end with CreateFile
        let has_create_file = actions.iter().any(|a| matches!(a, SyncAction::CreateFile { .. }));
        assert!(has_create_file, "expected CreateFile action, got {:?}", actions);
    }

    #[test]
    fn test_sync_status_drifted() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("instructions.md"), "different content").unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                Path::new(".ai/instructions.md"),
                dir.path().join("CLAUDE.md"),
            )
            .unwrap();
        }

        let status = ClaudeCodeAdapter
            .sync_status(dir.path(), "wrong_hash_value")
            .unwrap();
        assert!(matches!(status.drift, DriftState::Drifted { .. }));
    }

    // --- translate_hooks tests ---

    #[test]
    fn test_translate_hooks_produces_valid_json() {
        use crate::types::{HookGroup, HookHandler, HooksConfig, HookTranslation};
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
        let config = HooksConfig { events };

        let result = ClaudeCodeAdapter.translate_hooks(&config).unwrap();
        match result {
            HookTranslation::Supported { tool, content, format } => {
                assert_eq!(tool, ToolKind::ClaudeCode);
                assert_eq!(format, "json");
                let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
                let hooks = &parsed["hooks"]["PreToolUse"];
                assert!(hooks.is_array());
                assert_eq!(hooks[0]["matcher"], "Edit");
                // Timeout should be in seconds (10000ms -> 10s)
                assert_eq!(hooks[0]["hooks"][0]["timeout"], 10);
                assert_eq!(hooks[0]["hooks"][0]["type"], "command");
                assert_eq!(hooks[0]["hooks"][0]["command"], "npm run lint");
            }
            other => panic!("expected Supported, got {other:?}"),
        }
    }

    #[test]
    fn test_translate_hooks_omits_matcher_when_none() {
        use crate::types::{HookGroup, HookHandler, HooksConfig, HookTranslation};
        use std::collections::BTreeMap;

        let mut events = BTreeMap::new();
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

        let result = ClaudeCodeAdapter.translate_hooks(&config).unwrap();
        match result {
            HookTranslation::Supported { content, .. } => {
                let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
                let group = &parsed["hooks"]["PostToolUse"][0];
                // matcher key should be absent, not null
                assert!(group.get("matcher").is_none(), "matcher should be absent when None");
            }
            other => panic!("expected Supported, got {other:?}"),
        }
    }
}
