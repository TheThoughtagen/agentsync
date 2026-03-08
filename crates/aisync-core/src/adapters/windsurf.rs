use std::path::{Path, PathBuf};

use crate::adapter::{DetectionResult, ToolAdapter, WindsurfAdapter};
use crate::config::SyncStrategy;
use crate::error::AisyncError;
use crate::types::{
    Confidence, DriftState, HookTranslation, HooksConfig, SyncAction, ToolKind, ToolSyncStatus,
    content_hash,
};

/// The output path relative to project root for generated .md file.
const WINDSURF_REL: &str = ".windsurf/rules/project.md";

/// YAML frontmatter prefix for generated Windsurf .md files.
const WINDSURF_FRONTMATTER: &str =
    "---\ntrigger: always_on\ndescription: Project instructions synced by aisync\n---\n\n";

/// Generate the full Windsurf .md file content with frontmatter.
fn generate_windsurf_content(canonical_content: &str) -> String {
    format!("{WINDSURF_FRONTMATTER}{canonical_content}")
}

impl ToolAdapter for WindsurfAdapter {
    fn name(&self) -> ToolKind {
        ToolKind::Windsurf
    }

    fn display_name(&self) -> &str {
        "Windsurf"
    }

    fn native_instruction_path(&self) -> &str {
        WINDSURF_REL
    }

    fn conditional_tags(&self) -> &[&str] {
        &["windsurf-only"]
    }

    fn default_sync_strategy(&self) -> SyncStrategy {
        SyncStrategy::Generate
    }

    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError> {
        let mut markers = Vec::new();
        let mut version_hint = None;
        let windsurf_rules_dir = project_root.join(".windsurf").join("rules");
        let windsurfrules_file = project_root.join(".windsurfrules");

        if windsurf_rules_dir.is_dir() {
            markers.push(windsurf_rules_dir);
        }
        if windsurfrules_file.exists() {
            markers.push(windsurfrules_file);
            version_hint = Some(
                "legacy format (.windsurfrules) -- consider migrating to .windsurf/rules/".into(),
            );
        }

        let detected = !markers.is_empty();
        Ok(DetectionResult {
            tool: ToolKind::Windsurf,
            detected,
            confidence: Confidence::High,
            markers_found: markers,
            version_hint,
        })
    }

    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AisyncError> {
        let path = project_root.join(WINDSURF_REL);
        if !path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| AisyncError::Adapter {
            tool: "windsurf".to_string(),
            source: crate::error::AdapterError::DetectionFailed(format!(
                "failed to read {}: {e}",
                path.display()
            )),
        })?;

        // Strip YAML frontmatter: content between --- and ---
        let body = if let Some(after_open) = raw.strip_prefix("---") {
            if let Some(end_idx) = after_open.find("---") {
                let after_frontmatter = &after_open[end_idx + 3..];
                after_frontmatter.trim_start_matches('\n').to_string()
            } else {
                raw
            }
        } else {
            raw
        };

        Ok(Some(body))
    }

    fn plan_sync(
        &self,
        project_root: &Path,
        canonical_content: &str,
        _strategy: SyncStrategy,
    ) -> Result<Vec<SyncAction>, AisyncError> {
        // Windsurf always uses Generate strategy
        let output_path = project_root.join(WINDSURF_REL);
        let expected_content = generate_windsurf_content(canonical_content);

        let mut actions = Vec::new();

        // Ensure directory exists
        let rules_dir = project_root.join(".windsurf").join("rules");
        if !rules_dir.is_dir() {
            actions.push(SyncAction::CreateDirectory { path: rules_dir });
        }

        if output_path.exists() {
            let existing =
                std::fs::read_to_string(&output_path).map_err(|e| AisyncError::Adapter {
                    tool: "windsurf".to_string(),
                    source: crate::error::AdapterError::DetectionFailed(format!(
                        "failed to read {}: {e}",
                        output_path.display()
                    )),
                })?;
            if existing == expected_content {
                // Idempotent: no action needed
                return Ok(vec![]);
            }
        }

        actions.push(SyncAction::CreateFile {
            path: output_path,
            content: expected_content,
        });

        Ok(actions)
    }

    fn sync_status(
        &self,
        project_root: &Path,
        canonical_hash: &str,
        _strategy: SyncStrategy,
    ) -> Result<ToolSyncStatus, AisyncError> {
        let path = project_root.join(WINDSURF_REL);

        if !path.exists() {
            return Ok(ToolSyncStatus {
                tool: ToolKind::Windsurf,
                strategy: SyncStrategy::Generate,
                drift: DriftState::Missing,
                details: None,
            });
        }

        let actual_content = std::fs::read(&path).map_err(|e| AisyncError::Adapter {
            tool: "windsurf".to_string(),
            source: crate::error::AdapterError::DetectionFailed(format!(
                "failed to read {}: {e}",
                path.display()
            )),
        })?;

        // Strip frontmatter and hash body only
        let actual_str = String::from_utf8_lossy(&actual_content);
        let body = if let Some(after_open) = actual_str.strip_prefix("---") {
            if let Some(end_idx) = after_open.find("---") {
                let after = &after_open[end_idx + 3..];
                after.trim_start_matches('\n').to_string()
            } else {
                actual_str.to_string()
            }
        } else {
            actual_str.to_string()
        };

        let body_hash = content_hash(body.as_bytes());
        let actual_hash = content_hash(&actual_content);

        if body_hash == canonical_hash {
            Ok(ToolSyncStatus {
                tool: ToolKind::Windsurf,
                strategy: SyncStrategy::Generate,
                drift: DriftState::InSync,
                details: None,
            })
        } else {
            Ok(ToolSyncStatus {
                tool: ToolKind::Windsurf,
                strategy: SyncStrategy::Generate,
                drift: DriftState::Drifted {
                    reason: "content hash mismatch".to_string(),
                },
                details: Some(format!(
                    "file hash: {actual_hash}, body hash: {body_hash}, expected: {canonical_hash}"
                )),
            })
        }
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
                let rel = path
                    .strip_prefix(project_root)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| format!(".ai/memory/{}.md", name));
                Some(format!("- [{}]({})", name, rel))
            })
            .collect();

        Ok(vec![SyncAction::UpdateMemoryReferences {
            path: project_root.join(WINDSURF_REL),
            references,
            marker_start: "<!-- aisync:memory -->".to_string(),
            marker_end: "<!-- /aisync:memory -->".to_string(),
        }])
    }

    fn translate_hooks(&self, _hooks: &HooksConfig) -> Result<HookTranslation, AisyncError> {
        Ok(HookTranslation::Unsupported {
            tool: ToolKind::Windsurf,
            reason: "Windsurf does not support hooks".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- detect tests ---

    #[test]
    fn test_name_returns_windsurf() {
        assert_eq!(WindsurfAdapter.name(), ToolKind::Windsurf);
    }

    #[test]
    fn test_detects_windsurf_rules_dir() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".windsurf/rules")).unwrap();

        let result = WindsurfAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_detects_legacy_windsurfrules() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".windsurfrules"), "rules here").unwrap();

        let result = WindsurfAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert!(result.version_hint.as_ref().unwrap().contains("legacy"));
    }

    #[test]
    fn test_detects_both_markers() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".windsurf/rules")).unwrap();
        std::fs::write(dir.path().join(".windsurfrules"), "rules").unwrap();

        let result = WindsurfAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.markers_found.len(), 2);
    }

    #[test]
    fn test_not_detected_empty_dir() {
        let dir = TempDir::new().unwrap();

        let result = WindsurfAdapter.detect(dir.path()).unwrap();
        assert!(!result.detected);
    }

    // --- read_instructions tests ---

    #[test]
    fn test_read_instructions_strips_frontmatter() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".windsurf").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        let content =
            "---\ntrigger: always_on\ndescription: test\n---\n\n# Instructions";
        std::fs::write(rules_dir.join("project.md"), content).unwrap();

        let result = WindsurfAdapter.read_instructions(dir.path()).unwrap();
        assert_eq!(result, Some("# Instructions".to_string()));
    }

    #[test]
    fn test_read_instructions_returns_none_when_missing() {
        let dir = TempDir::new().unwrap();

        let result = WindsurfAdapter.read_instructions(dir.path()).unwrap();
        assert_eq!(result, None);
    }

    // --- plan_sync tests ---

    #[test]
    fn test_plan_sync_generates_with_frontmatter() {
        let dir = TempDir::new().unwrap();

        let actions = WindsurfAdapter
            .plan_sync(dir.path(), "# My instructions", SyncStrategy::Generate)
            .unwrap();

        assert!(actions.len() >= 1);

        let create_action = actions
            .iter()
            .find(|a| matches!(a, SyncAction::CreateFile { .. }));
        assert!(create_action.is_some(), "expected CreateFile action");

        if let SyncAction::CreateFile { content, .. } = create_action.unwrap() {
            assert!(content.contains("trigger: always_on"));
            assert!(content.contains("description: Project instructions synced by aisync"));
            assert!(content.contains("# My instructions"));
        }
    }

    #[test]
    fn test_plan_sync_creates_directory() {
        let dir = TempDir::new().unwrap();

        let actions = WindsurfAdapter
            .plan_sync(dir.path(), "# Instructions", SyncStrategy::Generate)
            .unwrap();

        let dir_action = actions
            .iter()
            .find(|a| matches!(a, SyncAction::CreateDirectory { .. }));
        assert!(dir_action.is_some(), "expected CreateDirectory action");
    }

    #[test]
    fn test_plan_sync_returns_empty_when_content_unchanged() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".windsurf").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        let canonical = "# My instructions";
        let expected = generate_windsurf_content(canonical);
        std::fs::write(rules_dir.join("project.md"), &expected).unwrap();

        let actions = WindsurfAdapter
            .plan_sync(dir.path(), canonical, SyncStrategy::Generate)
            .unwrap();
        assert!(
            actions.is_empty(),
            "expected no actions for unchanged content"
        );
    }

    #[test]
    fn test_plan_sync_generates_when_content_different() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".windsurf").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::write(rules_dir.join("project.md"), "old content").unwrap();

        let actions = WindsurfAdapter
            .plan_sync(dir.path(), "new instructions", SyncStrategy::Generate)
            .unwrap();
        assert!(!actions.is_empty());
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, SyncAction::CreateFile { .. }))
        );
    }

    // --- sync_status tests ---

    #[test]
    fn test_sync_status_missing() {
        let dir = TempDir::new().unwrap();

        let status = WindsurfAdapter
            .sync_status(dir.path(), "abc123", SyncStrategy::Generate)
            .unwrap();
        assert_eq!(status.tool, ToolKind::Windsurf);
        assert_eq!(status.drift, DriftState::Missing);
    }

    #[test]
    fn test_sync_status_in_sync() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".windsurf").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        let canonical = "# My instructions";
        let content = generate_windsurf_content(canonical);
        std::fs::write(rules_dir.join("project.md"), &content).unwrap();

        let canonical_hash = content_hash(canonical.as_bytes());
        let status = WindsurfAdapter
            .sync_status(dir.path(), &canonical_hash, SyncStrategy::Generate)
            .unwrap();
        assert_eq!(status.drift, DriftState::InSync);
    }

    #[test]
    fn test_sync_status_drifted() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".windsurf").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        let content = generate_windsurf_content("old instructions");
        std::fs::write(rules_dir.join("project.md"), &content).unwrap();

        let wrong_hash = content_hash(b"different content");
        let status = WindsurfAdapter
            .sync_status(dir.path(), &wrong_hash, SyncStrategy::Generate)
            .unwrap();
        assert!(matches!(status.drift, DriftState::Drifted { .. }));
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
        let actions = WindsurfAdapter
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
                assert!(path.to_string_lossy().contains(".windsurf/rules/project.md"));
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

        let actions = WindsurfAdapter.plan_memory_sync(dir.path(), &[]).unwrap();
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

        let result = WindsurfAdapter.translate_hooks(&config).unwrap();
        match result {
            HookTranslation::Unsupported { tool, reason } => {
                assert_eq!(tool, ToolKind::Windsurf);
                assert!(reason.contains("Windsurf does not support hooks"));
            }
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    // --- default_sync_strategy test ---

    #[test]
    fn test_default_sync_strategy_is_generate() {
        assert_eq!(WindsurfAdapter.default_sync_strategy(), SyncStrategy::Generate);
    }

    #[test]
    fn test_conditional_tags() {
        assert_eq!(WindsurfAdapter.conditional_tags(), &["windsurf-only"]);
    }
}
