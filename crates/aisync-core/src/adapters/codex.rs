use std::path::{Path, PathBuf};

use crate::adapter::{CodexAdapter, DetectionResult, ToolAdapter};
use crate::config::SyncStrategy;
use crate::adapter::AdapterError;
use crate::types::{
    Confidence, DriftState, HookTranslation, HooksConfig, RuleFile, SyncAction, ToolKind,
    ToolSyncStatus, content_hash,
};

/// The relative symlink target path from project root to canonical instructions.
const CANONICAL_REL: &str = ".ai/instructions.md";
/// The tool-specific file name at project root.
const TOOL_FILE: &str = "AGENTS.md";

impl CodexAdapter {
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

impl ToolAdapter for CodexAdapter {
    fn name(&self) -> ToolKind {
        ToolKind::Codex
    }

    fn display_name(&self) -> &str {
        "Codex"
    }

    fn native_instruction_path(&self) -> &str {
        TOOL_FILE
    }

    fn conditional_tags(&self) -> &[&str] {
        &["codex-only"]
    }

    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AdapterError> {
        let codex_dir = project_root.join(".codex");

        if codex_dir.is_dir() {
            Ok(DetectionResult {
                tool: ToolKind::Codex,
                detected: true,
                confidence: Confidence::High,
                markers_found: vec![codex_dir],
                version_hint: None,
            })
        } else {
            Ok(DetectionResult {
                tool: ToolKind::Codex,
                detected: false,
                confidence: Confidence::High,
                markers_found: vec![],
                version_hint: None,
            })
        }
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

        // Check content size limit (Codex has 32 KiB limit)
        let mut size_warning = None;
        let byte_size = canonical_content.len();
        if byte_size > 32_768 {
            size_warning = Some(SyncAction::WarnContentSize {
                tool: ToolKind::Codex,
                path: link_path.clone(),
                actual_size: byte_size,
                limit: 32_768,
                unit: "bytes".to_string(),
            });
        }

        // Determine whether conditionals changed the content
        let raw_path = project_root.join(CANONICAL_REL);
        let conditionals_active = match std::fs::read_to_string(&raw_path) {
            Ok(raw_content) => canonical_content != raw_content,
            Err(_) => false,
        };

        if conditionals_active || strategy == SyncStrategy::Copy {
            let mut actions = Vec::new();
            if let Some(warning) = size_warning {
                actions.push(warning);
            }
            actions.extend(self.plan_sync_with_conditionals(&link_path, canonical_content)?);
            return Ok(actions);
        }

        // Helper to prepend size warning if present
        let prepend_warning = |mut actions: Vec<SyncAction>, warning: Option<SyncAction>| -> Vec<SyncAction> {
            if let Some(w) = warning {
                actions.insert(0, w);
            }
            actions
        };

        // No conditionals + symlink strategy: use symlink
        if link_path.exists() || link_path.symlink_metadata().is_ok() {
            if let Ok(meta) = link_path.symlink_metadata() {
                if meta.file_type().is_symlink() {
                    let current_target =
                        std::fs::read_link(&link_path).map_err(|e| AdapterError::DetectionFailed(format!(
                                "failed to read symlink: {e}"
                            )))?;
                    if current_target == target_rel {
                        return Ok(prepend_warning(vec![], size_warning));
                    }
                    return Ok(prepend_warning(vec![SyncAction::RemoveAndRelink {
                        link: link_path,
                        target: target_rel.to_path_buf(),
                    }], size_warning));
                }
                return Ok(prepend_warning(vec![SyncAction::SkipExistingFile {
                    path: link_path,
                    reason: format!("{} is a regular file, not managed by aisync", TOOL_FILE),
                }], size_warning));
            }
        }

        Ok(prepend_warning(vec![SyncAction::CreateSymlink {
            link: link_path,
            target: target_rel.to_path_buf(),
        }], size_warning))
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
                    tool: ToolKind::Codex,
                    strategy,
                    drift: DriftState::Missing,
                    details: None,
                });
            }
        };

        if meta.file_type().is_symlink() {
            if !path.exists() {
                return Ok(ToolSyncStatus {
                    tool: ToolKind::Codex,
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
                    tool: ToolKind::Codex,
                    strategy,
                    drift: DriftState::InSync,
                    details: None,
                });
            }
            return Ok(ToolSyncStatus {
                tool: ToolKind::Codex,
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
                tool: ToolKind::Codex,
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
            tool: ToolKind::Codex,
            strategy,
            drift,
            details: Some(format!("regular file, hash: {hash}")),
        })
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

    fn translate_hooks(&self, _hooks: &HooksConfig) -> Result<HookTranslation, AdapterError> {
        Ok(HookTranslation::Unsupported {
            tool: ToolKind::Codex,
            reason: "Codex does not support hooks".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- detect tests ---

    #[test]
    fn test_name_returns_codex() {
        assert_eq!(CodexAdapter.name(), ToolKind::Codex);
    }

    #[test]
    fn test_detects_codex_dir() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".codex")).unwrap();

        let result = CodexAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
        assert_eq!(result.markers_found.len(), 1);
    }

    #[test]
    fn test_not_detected_empty_dir() {
        let dir = TempDir::new().unwrap();

        let result = CodexAdapter.detect(dir.path()).unwrap();
        assert!(!result.detected);
        assert!(result.markers_found.is_empty());
    }

    #[test]
    fn test_agents_md_alone_not_detected() {
        // AGENTS.md alone should NOT trigger Codex detection
        // (that's OpenCode's medium-confidence detection territory)
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "# Agents").unwrap();

        let result = CodexAdapter.detect(dir.path()).unwrap();
        assert!(!result.detected);
    }

    // --- read_instructions tests ---

    #[test]
    fn test_read_instructions_reads_content() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "# Agent Instructions").unwrap();

        let content = CodexAdapter.read_instructions(dir.path()).unwrap();
        assert_eq!(content, Some("# Agent Instructions".to_string()));
    }

    #[test]
    fn test_read_instructions_returns_none_when_missing() {
        let dir = TempDir::new().unwrap();

        let content = CodexAdapter.read_instructions(dir.path()).unwrap();
        assert_eq!(content, None);
    }

    // --- plan_sync tests ---

    #[test]
    fn test_plan_sync_creates_symlink_when_missing() {
        let dir = TempDir::new().unwrap();

        let actions = CodexAdapter
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

        std::os::unix::fs::symlink(
            Path::new(".ai/instructions.md"),
            dir.path().join("AGENTS.md"),
        )
        .unwrap();

        let actions = CodexAdapter
            .plan_sync(dir.path(), "content", SyncStrategy::Symlink)
            .unwrap();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_plan_sync_regular_file_returns_skip() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "user content").unwrap();

        let actions = CodexAdapter
            .plan_sync(dir.path(), "content", SyncStrategy::Symlink)
            .unwrap();
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], SyncAction::SkipExistingFile { .. }));
    }

    // --- sync_status tests ---

    #[test]
    fn test_sync_status_missing() {
        let dir = TempDir::new().unwrap();

        let status = CodexAdapter
            .sync_status(dir.path(), "abc123", SyncStrategy::Symlink)
            .unwrap();
        assert_eq!(status.tool, ToolKind::Codex);
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

        std::os::unix::fs::symlink(
            Path::new(".ai/instructions.md"),
            dir.path().join("AGENTS.md"),
        )
        .unwrap();

        let status = CodexAdapter
            .sync_status(dir.path(), &hash, SyncStrategy::Symlink)
            .unwrap();
        assert_eq!(status.drift, DriftState::InSync);
    }

    #[cfg(unix)]
    #[test]
    fn test_sync_status_dangling_symlink() {
        let dir = TempDir::new().unwrap();

        std::os::unix::fs::symlink(
            Path::new(".ai/instructions.md"),
            dir.path().join("AGENTS.md"),
        )
        .unwrap();

        let status = CodexAdapter
            .sync_status(dir.path(), "abc123", SyncStrategy::Symlink)
            .unwrap();
        assert_eq!(status.drift, DriftState::DanglingSymlink);
    }

    // --- plan_memory_sync tests ---

    #[cfg(unix)]
    #[test]
    fn test_plan_memory_sync_returns_update_memory_references() {
        let dir = TempDir::new().unwrap();
        let memory_dir = dir.path().join(".ai/memory");
        std::fs::create_dir_all(&memory_dir).unwrap();
        std::fs::write(memory_dir.join("debugging.md"), "# Debugging").unwrap();

        let memory_files = vec![memory_dir.join("debugging.md")];
        let actions = CodexAdapter
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
                assert_eq!(references.len(), 1);
                assert!(references[0].contains(".ai/memory/debugging.md"));
                assert_eq!(marker_start, "<!-- aisync:memory -->");
                assert_eq!(marker_end, "<!-- /aisync:memory -->");
            }
            other => panic!("expected UpdateMemoryReferences, got {other:?}"),
        }
    }

    #[test]
    fn test_plan_memory_sync_empty_files_returns_empty() {
        let dir = TempDir::new().unwrap();

        let actions = CodexAdapter.plan_memory_sync(dir.path(), &[]).unwrap();
        assert!(actions.is_empty());
    }

    // --- plan_rules_sync tests ---

    #[test]
    fn test_plan_rules_sync_returns_update_memory_references() {
        use crate::types::RuleFile;
        use crate::types::RuleMetadata;
        use std::path::PathBuf;

        let dir = TempDir::new().unwrap();
        let rules = vec![RuleFile {
            name: "testing".to_string(),
            metadata: RuleMetadata {
                description: Some("Testing rules".to_string()),
                globs: vec![],
                always_apply: true,
            },
            content: "Write tests first.".to_string(),
            source_path: PathBuf::from(".ai/rules/testing.md"),
        }];

        let actions = CodexAdapter.plan_rules_sync(dir.path(), &rules).unwrap();
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
                assert!(references[0].contains("## Rule: testing"));
                assert!(references[0].contains("Write tests first."));
            }
            other => panic!("expected UpdateMemoryReferences, got {other:?}"),
        }
    }

    #[test]
    fn test_plan_rules_sync_empty_rules_returns_empty() {
        let dir = TempDir::new().unwrap();
        let actions = CodexAdapter.plan_rules_sync(dir.path(), &[]).unwrap();
        assert!(actions.is_empty());
    }

    // --- translate_hooks tests ---

    #[test]
    fn test_translate_hooks_returns_unsupported() {
        use crate::types::{HookGroup, HookHandler};
        use std::collections::BTreeMap;

        let mut events = BTreeMap::new();
        events.insert(
            "PreToolUse".to_string(),
            vec![HookGroup {
                matcher: None,
                hooks: vec![HookHandler {
                    hook_type: "command".to_string(),
                    command: "echo test".to_string(),
                    timeout: None,
                }],
            }],
        );
        let config = HooksConfig { events };

        let result = CodexAdapter.translate_hooks(&config).unwrap();
        match result {
            HookTranslation::Unsupported { tool, reason } => {
                assert_eq!(tool, ToolKind::Codex);
                assert!(reason.contains("Codex does not support hooks"));
            }
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    // --- default_sync_strategy test ---

    #[test]
    fn test_default_sync_strategy_is_symlink() {
        assert_eq!(CodexAdapter.default_sync_strategy(), SyncStrategy::Symlink);
    }

    #[test]
    fn test_conditional_tags() {
        assert_eq!(CodexAdapter.conditional_tags(), &["codex-only"]);
    }

    #[test]
    fn test_plan_sync_warns_on_large_content() {
        let dir = TempDir::new().unwrap();

        // Create content > 32 KiB (32_768 bytes)
        let large_content = "x".repeat(32_769);

        // Need .ai/instructions.md for conditionals check
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("instructions.md"), &large_content).unwrap();

        let actions = CodexAdapter
            .plan_sync(dir.path(), &large_content, SyncStrategy::Symlink)
            .unwrap();

        let warn_action = actions
            .iter()
            .find(|a| matches!(a, SyncAction::WarnContentSize { .. }));
        assert!(
            warn_action.is_some(),
            "expected WarnContentSize action for content > 32 KiB"
        );

        if let SyncAction::WarnContentSize {
            tool,
            actual_size,
            limit,
            unit,
            ..
        } = warn_action.unwrap()
        {
            assert_eq!(*tool, ToolKind::Codex);
            assert!(*actual_size > 32_768);
            assert_eq!(*limit, 32_768);
            assert_eq!(unit, "bytes");
        }

        // Warning should come before the main action
        let warn_idx = actions
            .iter()
            .position(|a| matches!(a, SyncAction::WarnContentSize { .. }))
            .unwrap();
        let main_idx = actions
            .iter()
            .position(|a| {
                matches!(
                    a,
                    SyncAction::CreateSymlink { .. } | SyncAction::CreateFile { .. }
                )
            })
            .unwrap();
        assert!(
            warn_idx < main_idx,
            "WarnContentSize should come before main action"
        );
    }

    #[test]
    fn test_plan_sync_no_warning_under_limit() {
        let dir = TempDir::new().unwrap();

        // Content under 32 KiB
        let small_content = "x".repeat(32_000);

        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("instructions.md"), &small_content).unwrap();

        let actions = CodexAdapter
            .plan_sync(dir.path(), &small_content, SyncStrategy::Symlink)
            .unwrap();

        let warn_action = actions
            .iter()
            .find(|a| matches!(a, SyncAction::WarnContentSize { .. }));
        assert!(
            warn_action.is_none(),
            "expected no WarnContentSize for content under 32 KiB"
        );
    }
}
