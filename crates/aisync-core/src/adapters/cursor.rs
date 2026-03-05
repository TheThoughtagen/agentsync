use std::path::Path;

use crate::adapter::{CursorAdapter, DetectionResult, ToolAdapter};
use crate::config::SyncStrategy;
use crate::error::AisyncError;
use crate::types::{content_hash, Confidence, DriftState, SyncAction, ToolKind, ToolSyncStatus};

/// The output path relative to project root for generated .mdc file.
const MDC_REL: &str = ".cursor/rules/project.mdc";

/// YAML frontmatter prefix for generated .mdc files.
const MDC_FRONTMATTER: &str = "---\ndescription: Project instructions synced by aisync\nglobs: \"**\"\nalwaysApply: true\n---\n\n";

/// Generate the full .mdc file content with frontmatter.
fn generate_mdc_content(canonical_content: &str) -> String {
    format!("{MDC_FRONTMATTER}{canonical_content}")
}

impl ToolAdapter for CursorAdapter {
    fn name(&self) -> ToolKind {
        ToolKind::Cursor
    }

    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError> {
        let mut markers = Vec::new();
        let mut version_hint = None;
        let cursor_rules_dir = project_root.join(".cursor").join("rules");
        let cursorrules_file = project_root.join(".cursorrules");

        if cursor_rules_dir.is_dir() {
            markers.push(cursor_rules_dir);
        }
        if cursorrules_file.exists() {
            markers.push(cursorrules_file);
            version_hint = Some(
                "legacy format (.cursorrules) — consider migrating to .cursor/rules/".into(),
            );
        }

        let detected = !markers.is_empty();
        Ok(DetectionResult {
            tool: ToolKind::Cursor,
            detected,
            confidence: Confidence::High,
            markers_found: markers,
            version_hint,
        })
    }

    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AisyncError> {
        let path = project_root.join(MDC_REL);
        if !path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| AisyncError::Adapter {
            tool: "cursor".to_string(),
            source: crate::error::AdapterError::DetectionFailed(format!(
                "failed to read {}: {e}",
                path.display()
            )),
        })?;

        // Strip YAML frontmatter: content between --- and ---
        let body = if raw.starts_with("---") {
            // Find the closing ---
            if let Some(end_idx) = raw[3..].find("---") {
                let after_frontmatter = &raw[3 + end_idx + 3..];
                // Strip leading newlines after frontmatter
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
        // Cursor always uses Generate strategy
        let output_path = project_root.join(MDC_REL);
        let expected_content = generate_mdc_content(canonical_content);

        let mut actions = Vec::new();

        // Ensure directory exists
        let rules_dir = project_root.join(".cursor").join("rules");
        if !rules_dir.is_dir() {
            actions.push(SyncAction::CreateDirectory {
                path: rules_dir,
            });
        }

        if output_path.exists() {
            // Compare existing content
            let existing = std::fs::read_to_string(&output_path).map_err(|e| {
                AisyncError::Adapter {
                    tool: "cursor".to_string(),
                    source: crate::error::AdapterError::DetectionFailed(format!(
                        "failed to read {}: {e}",
                        output_path.display()
                    )),
                }
            })?;
            if existing == expected_content {
                // Idempotent: no action needed
                return Ok(vec![]);
            }
        }

        actions.push(SyncAction::GenerateMdc {
            output: output_path,
            content: expected_content,
        });

        Ok(actions)
    }

    fn sync_status(
        &self,
        project_root: &Path,
        canonical_hash: &str,
    ) -> Result<ToolSyncStatus, AisyncError> {
        let path = project_root.join(MDC_REL);

        if !path.exists() {
            return Ok(ToolSyncStatus {
                tool: ToolKind::Cursor,
                strategy: SyncStrategy::Generate,
                drift: DriftState::Missing,
                details: None,
            });
        }

        let actual_content = std::fs::read(&path).map_err(|e| AisyncError::Adapter {
            tool: "cursor".to_string(),
            source: crate::error::AdapterError::DetectionFailed(format!(
                "failed to read {}: {e}",
                path.display()
            )),
        })?;
        let actual_hash = content_hash(&actual_content);

        // For Cursor, we compare the hash of the entire .mdc file (including frontmatter)
        // against the canonical hash. But since the .mdc includes frontmatter, we need to
        // reconstruct expected content and compare hashes.
        // The canonical_hash passed in is of the canonical content (without frontmatter).
        // So we'll hash what we'd generate and compare.
        // However, we don't have canonical_content here, only canonical_hash.
        // We'll compare the actual file hash against a stored/expected value.
        // For simplicity: read the body, hash it, compare to canonical_hash.

        // Strip frontmatter and hash body only
        let actual_str = String::from_utf8_lossy(&actual_content);
        let body = if actual_str.starts_with("---") {
            if let Some(end_idx) = actual_str[3..].find("---") {
                let after = &actual_str[3 + end_idx + 3..];
                after.trim_start_matches('\n').to_string()
            } else {
                actual_str.to_string()
            }
        } else {
            actual_str.to_string()
        };

        let body_hash = content_hash(body.as_bytes());

        if body_hash == canonical_hash {
            Ok(ToolSyncStatus {
                tool: ToolKind::Cursor,
                strategy: SyncStrategy::Generate,
                drift: DriftState::InSync,
                details: None,
            })
        } else {
            Ok(ToolSyncStatus {
                tool: ToolKind::Cursor,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_name_returns_cursor() {
        assert_eq!(CursorAdapter.name(), ToolKind::Cursor);
    }

    #[test]
    fn test_detects_cursor_rules_dir() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".cursor/rules")).unwrap();

        let result = CursorAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_detects_legacy_cursorrules() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".cursorrules"), "rules here").unwrap();

        let result = CursorAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert!(result.version_hint.as_ref().unwrap().contains("legacy"));
    }

    #[test]
    fn test_detects_both_markers() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".cursor/rules")).unwrap();
        std::fs::write(dir.path().join(".cursorrules"), "rules").unwrap();

        let result = CursorAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.markers_found.len(), 2);
    }

    #[test]
    fn test_not_detected_empty_dir() {
        let dir = TempDir::new().unwrap();

        let result = CursorAdapter.detect(dir.path()).unwrap();
        assert!(!result.detected);
    }

    // --- read_instructions tests ---

    #[test]
    fn test_read_instructions_strips_frontmatter() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".cursor").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        let mdc_content = "---\ndescription: test\nglobs: \"**\"\nalwaysApply: true\n---\n\n# Instructions";
        std::fs::write(rules_dir.join("project.mdc"), mdc_content).unwrap();

        let content = CursorAdapter.read_instructions(dir.path()).unwrap();
        assert_eq!(content, Some("# Instructions".to_string()));
    }

    #[test]
    fn test_read_instructions_returns_none_when_missing() {
        let dir = TempDir::new().unwrap();

        let content = CursorAdapter.read_instructions(dir.path()).unwrap();
        assert_eq!(content, None);
    }

    // --- plan_sync tests ---

    #[test]
    fn test_plan_sync_generates_mdc_with_frontmatter() {
        let dir = TempDir::new().unwrap();

        let actions = CursorAdapter
            .plan_sync(dir.path(), "# My instructions", SyncStrategy::Generate)
            .unwrap();

        // Should include CreateDirectory + GenerateMdc
        assert!(actions.len() >= 1);

        let mdc_action = actions.iter().find(|a| matches!(a, SyncAction::GenerateMdc { .. }));
        assert!(mdc_action.is_some(), "expected GenerateMdc action");

        if let SyncAction::GenerateMdc { content, .. } = mdc_action.unwrap() {
            assert!(content.contains("description: Project instructions synced by aisync"));
            assert!(content.contains("globs: \"**\""));
            assert!(content.contains("alwaysApply: true"));
            assert!(content.contains("# My instructions"));
        }
    }

    #[test]
    fn test_plan_sync_returns_empty_when_content_unchanged() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".cursor").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        let canonical = "# My instructions";
        let expected = generate_mdc_content(canonical);
        std::fs::write(rules_dir.join("project.mdc"), &expected).unwrap();

        let actions = CursorAdapter
            .plan_sync(dir.path(), canonical, SyncStrategy::Generate)
            .unwrap();
        assert!(actions.is_empty(), "expected no actions for unchanged content");
    }

    #[test]
    fn test_plan_sync_generates_when_content_different() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".cursor").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::write(rules_dir.join("project.mdc"), "old content").unwrap();

        let actions = CursorAdapter
            .plan_sync(dir.path(), "new instructions", SyncStrategy::Generate)
            .unwrap();
        assert!(!actions.is_empty());
        assert!(actions.iter().any(|a| matches!(a, SyncAction::GenerateMdc { .. })));
    }

    // --- sync_status tests ---

    #[test]
    fn test_sync_status_missing() {
        let dir = TempDir::new().unwrap();

        let status = CursorAdapter.sync_status(dir.path(), "abc123").unwrap();
        assert_eq!(status.tool, ToolKind::Cursor);
        assert_eq!(status.drift, DriftState::Missing);
    }

    #[test]
    fn test_sync_status_in_sync() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".cursor").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        let canonical = "# My instructions";
        let mdc_content = generate_mdc_content(canonical);
        std::fs::write(rules_dir.join("project.mdc"), &mdc_content).unwrap();

        let canonical_hash = content_hash(canonical.as_bytes());
        let status = CursorAdapter.sync_status(dir.path(), &canonical_hash).unwrap();
        assert_eq!(status.drift, DriftState::InSync);
    }

    #[test]
    fn test_sync_status_drifted() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".cursor").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        let mdc_content = generate_mdc_content("old instructions");
        std::fs::write(rules_dir.join("project.mdc"), &mdc_content).unwrap();

        let wrong_hash = content_hash(b"different content");
        let status = CursorAdapter.sync_status(dir.path(), &wrong_hash).unwrap();
        assert!(matches!(status.drift, DriftState::Drifted { .. }));
    }
}
