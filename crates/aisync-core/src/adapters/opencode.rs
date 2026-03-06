use std::path::{Path, PathBuf};

use crate::adapter::{DetectionResult, OpenCodeAdapter, ToolAdapter};
use crate::config::SyncStrategy;
use crate::error::AisyncError;
use crate::types::{content_hash, Confidence, DriftState, SyncAction, ToolKind, ToolSyncStatus};

/// The relative symlink target path from project root to canonical instructions.
const CANONICAL_REL: &str = ".ai/instructions.md";
/// The tool-specific file name at project root.
const TOOL_FILE: &str = "AGENTS.md";

impl ToolAdapter for OpenCodeAdapter {
    fn name(&self) -> ToolKind {
        ToolKind::OpenCode
    }

    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError> {
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

    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AisyncError> {
        let path = project_root.join(TOOL_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path).map_err(|e| AisyncError::Adapter {
            tool: "opencode".to_string(),
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

        if link_path.exists() || link_path.symlink_metadata().is_ok() {
            if let Ok(meta) = link_path.symlink_metadata() {
                if meta.file_type().is_symlink() {
                    let current_target = std::fs::read_link(&link_path).map_err(|e| {
                        AisyncError::Adapter {
                            tool: "opencode".to_string(),
                            source: crate::error::AdapterError::DetectionFailed(format!(
                                "failed to read symlink: {e}"
                            )),
                        }
                    })?;
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
                    reason: format!(
                        "{} is a regular file, not managed by aisync",
                        TOOL_FILE
                    ),
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
    ) -> Result<Vec<SyncAction>, AisyncError> {
        if memory_files.is_empty() {
            return Ok(vec![]);
        }

        let references: Vec<String> = memory_files
            .iter()
            .filter_map(|path| {
                let name = path.file_stem()?.to_string_lossy().to_string();
                let rel = path.strip_prefix(project_root)
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

    fn translate_hooks(
        &self,
        hooks: &crate::types::HooksConfig,
    ) -> Result<crate::types::HookTranslation, AisyncError> {
        fn opencode_event_name(event: &str) -> Option<&'static str> {
            match event {
                "PreToolUse" => Some("tool.execute.before"),
                "PostToolUse" => Some("tool.execute.after"),
                "Stop" => Some("session.idle"),
                _ => None,
            }
        }

        let mut lines = vec![
            "// OpenCode plugin stub generated by aisync".to_string(),
            "// See: https://opencode.ai/docs/plugins/".to_string(),
            "module.exports = {".to_string(),
            "  hooks: {".to_string(),
        ];

        for (event, groups) in &hooks.events {
            if let Some(oc_event) = opencode_event_name(event) {
                lines.push(format!("    \"{}\": async (ctx) => {{", oc_event));
                for group in groups {
                    for hook in &group.hooks {
                        lines.push(format!("      // {}", hook.command));
                        lines.push(format!("      const {{ execSync }} = require(\"child_process\");"));
                        lines.push(format!("      execSync(\"{}\");", hook.command));
                    }
                }
                lines.push("    },".to_string());
            } else {
                lines.push(format!("    // Unsupported: {} (no OpenCode equivalent)", event));
            }
        }

        lines.push("  }".to_string());
        lines.push("};".to_string());

        let content = lines.join("\n");
        Ok(crate::types::HookTranslation::Supported {
            tool: ToolKind::OpenCode,
            content,
            format: "js".to_string(),
        })
    }

    fn sync_status(
        &self,
        project_root: &Path,
        canonical_hash: &str,
    ) -> Result<ToolSyncStatus, AisyncError> {
        let path = project_root.join(TOOL_FILE);

        let meta = match path.symlink_metadata() {
            Ok(m) => m,
            Err(_) => {
                return Ok(ToolSyncStatus {
                    tool: ToolKind::OpenCode,
                    strategy: SyncStrategy::Symlink,
                    drift: DriftState::Missing,
                    details: None,
                });
            }
        };

        if meta.file_type().is_symlink() {
            if !path.exists() {
                return Ok(ToolSyncStatus {
                    tool: ToolKind::OpenCode,
                    strategy: SyncStrategy::Symlink,
                    drift: DriftState::DanglingSymlink,
                    details: Some("symlink target does not exist".to_string()),
                });
            }

            let content = std::fs::read(&path).map_err(|e| AisyncError::Adapter {
                tool: "opencode".to_string(),
                source: crate::error::AdapterError::DetectionFailed(format!(
                    "failed to read {}: {e}",
                    path.display()
                )),
            })?;
            let hash = content_hash(&content);
            if hash == canonical_hash {
                return Ok(ToolSyncStatus {
                    tool: ToolKind::OpenCode,
                    strategy: SyncStrategy::Symlink,
                    drift: DriftState::InSync,
                    details: None,
                });
            }
            return Ok(ToolSyncStatus {
                tool: ToolKind::OpenCode,
                strategy: SyncStrategy::Symlink,
                drift: DriftState::Drifted {
                    reason: "content hash mismatch".to_string(),
                },
                details: Some(format!("expected {canonical_hash}, got {hash}")),
            });
        }

        // Regular file
        let content = std::fs::read(&path).map_err(|e| AisyncError::Adapter {
            tool: "opencode".to_string(),
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
            tool: ToolKind::OpenCode,
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

        let status = OpenCodeAdapter.sync_status(dir.path(), "abc123").unwrap();
        assert_eq!(status.tool, ToolKind::OpenCode);
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
                dir.path().join("AGENTS.md"),
            )
            .unwrap();
        }

        let status = OpenCodeAdapter.sync_status(dir.path(), &hash).unwrap();
        assert_eq!(status.drift, DriftState::InSync);
    }

    // --- plan_memory_sync tests ---

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
            SyncAction::UpdateMemoryReferences { path, references, marker_start, marker_end } => {
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

        let actions = OpenCodeAdapter
            .plan_memory_sync(dir.path(), &[])
            .unwrap();
        assert!(actions.is_empty());
    }

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

        let status = OpenCodeAdapter.sync_status(dir.path(), "abc123").unwrap();
        assert_eq!(status.drift, DriftState::DanglingSymlink);
    }

    // --- translate_hooks tests ---

    #[test]
    fn test_translate_hooks_produces_js_plugin_stub() {
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
            HookTranslation::Supported { tool, content, format } => {
                assert_eq!(tool, ToolKind::OpenCode);
                assert_eq!(format, "js");
                assert!(content.contains("module.exports"));
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
        use crate::types::{HookGroup, HookHandler, HooksConfig, HookTranslation};
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
        use crate::types::{HookGroup, HookHandler, HooksConfig, HookTranslation};
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
}
