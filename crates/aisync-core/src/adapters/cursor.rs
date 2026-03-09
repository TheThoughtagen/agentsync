use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::adapter::{CursorAdapter, DetectionResult, ToolAdapter};
use crate::config::SyncStrategy;
use crate::adapter::AdapterError;
use crate::types::{Confidence, DriftState, RuleFile, RuleMetadata, SyncAction, ToolKind, ToolSyncStatus, content_hash};

/// The output path relative to project root for generated .mdc file.
const MDC_REL: &str = ".cursor/rules/project.mdc";

/// YAML frontmatter prefix for generated .mdc files.
const MDC_FRONTMATTER: &str = "---\ndescription: Project instructions synced by aisync\nglobs: \"**\"\nalwaysApply: true\n---\n\n";

/// Generate the full .mdc file content with frontmatter.
fn generate_mdc_content(canonical_content: &str) -> String {
    format!("{MDC_FRONTMATTER}{canonical_content}")
}

/// Generate Cursor-format YAML frontmatter for a rule file.
///
/// Maps canonical metadata to Cursor frontmatter fields:
/// - description -> description:
/// - globs -> globs: "joined, string" (comma-separated, quoted)
/// - always_apply -> alwaysApply: (camelCase boolean)
fn generate_cursor_rule_frontmatter(meta: &RuleMetadata) -> String {
    let mut fm = String::from("---\n");
    if let Some(desc) = &meta.description {
        fm.push_str(&format!("description: {}\n", desc));
    }
    if !meta.globs.is_empty() {
        fm.push_str(&format!("globs: \"{}\"\n", meta.globs.join(", ")));
    }
    fm.push_str(&format!("alwaysApply: {}\n", meta.always_apply));
    fm.push_str("---\n\n");
    fm
}

/// Generate full Cursor rule file content (frontmatter + body).
fn generate_cursor_rule_content(meta: &RuleMetadata, body: &str) -> String {
    format!("{}{}", generate_cursor_rule_frontmatter(meta), body)
}

impl ToolAdapter for CursorAdapter {
    fn name(&self) -> ToolKind {
        ToolKind::Cursor
    }

    fn display_name(&self) -> &str {
        "Cursor"
    }

    fn native_instruction_path(&self) -> &str {
        ".cursor/rules/project.mdc"
    }

    fn conditional_tags(&self) -> &[&str] {
        &["cursor-only"]
    }

    fn default_sync_strategy(&self) -> crate::config::SyncStrategy {
        crate::config::SyncStrategy::Generate
    }

    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AdapterError> {
        let mut markers = Vec::new();
        let mut version_hint = None;
        let cursor_rules_dir = project_root.join(".cursor").join("rules");
        let cursorrules_file = project_root.join(".cursorrules");

        if cursor_rules_dir.is_dir() {
            markers.push(cursor_rules_dir);
        }
        if cursorrules_file.exists() {
            markers.push(cursorrules_file);
            version_hint =
                Some("legacy format (.cursorrules) — consider migrating to .cursor/rules/".into());
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

    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AdapterError> {
        let path = project_root.join(MDC_REL);
        if !path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| AdapterError::DetectionFailed(format!(
                "failed to read {}: {e}",
                path.display()
            )))?;

        // Strip YAML frontmatter: content between --- and ---
        let body = if let Some(after_open) = raw.strip_prefix("---") {
            // Find the closing ---
            if let Some(end_idx) = after_open.find("---") {
                let after_frontmatter = &after_open[end_idx + 3..];
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
    ) -> Result<Vec<SyncAction>, AdapterError> {
        // Cursor always uses Generate strategy
        let output_path = project_root.join(MDC_REL);
        let expected_content = generate_mdc_content(canonical_content);

        let mut actions = Vec::new();

        // Ensure directory exists
        let rules_dir = project_root.join(".cursor").join("rules");
        if !rules_dir.is_dir() {
            actions.push(SyncAction::CreateDirectory { path: rules_dir });
        }

        if output_path.exists() {
            // Compare existing content
            let existing =
                std::fs::read_to_string(&output_path).map_err(|e| AdapterError::DetectionFailed(format!(
                        "failed to read {}: {e}",
                        output_path.display()
                    )))?;
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
            path: project_root.join(MDC_REL),
            references,
            marker_start: "<!-- aisync:memory -->".to_string(),
            marker_end: "<!-- /aisync:memory -->".to_string(),
        }])
    }

    fn translate_hooks(
        &self,
        _hooks: &crate::types::HooksConfig,
    ) -> Result<crate::types::HookTranslation, AdapterError> {
        Ok(crate::types::HookTranslation::Unsupported {
            tool: ToolKind::Cursor,
            reason: "Cursor does not support hooks".to_string(),
        })
    }

    fn plan_rules_sync(
        &self,
        project_root: &Path,
        rules: &[RuleFile],
    ) -> Result<Vec<SyncAction>, AdapterError> {
        if rules.is_empty() {
            return Ok(vec![]);
        }

        let mut actions = Vec::new();
        let rules_dir = project_root.join(".cursor").join("rules");

        // Ensure directory exists
        if !rules_dir.is_dir() {
            actions.push(SyncAction::CreateDirectory {
                path: rules_dir.clone(),
            });
        }

        // Build expected filenames set
        let expected: HashSet<String> = rules
            .iter()
            .map(|r| format!("aisync-{}.mdc", r.name))
            .collect();

        // Generate rule files
        for rule in rules {
            let filename = format!("aisync-{}.mdc", rule.name);
            let output = rules_dir.join(&filename);
            let content = generate_cursor_rule_content(&rule.metadata, &rule.content);

            // Idempotent: skip if file already has the same content
            if output.exists() {
                if let Ok(existing) = std::fs::read_to_string(&output) {
                    if existing == content {
                        continue;
                    }
                }
            }

            actions.push(SyncAction::CreateRuleFile {
                output,
                content,
                rule_name: rule.name.clone(),
            });
        }

        // Scan for stale aisync-* files
        if rules_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&rules_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("aisync-") && name.ends_with(".mdc") && !expected.contains(&name) {
                        actions.push(SyncAction::RemoveFile {
                            path: entry.path(),
                        });
                    }
                }
            }
        }

        Ok(actions)
    }

    fn sync_status(
        &self,
        project_root: &Path,
        canonical_hash: &str,
        _strategy: SyncStrategy,
    ) -> Result<ToolSyncStatus, AdapterError> {
        let path = project_root.join(MDC_REL);

        if !path.exists() {
            return Ok(ToolSyncStatus {
                tool: ToolKind::Cursor,
                strategy: SyncStrategy::Generate,
                drift: DriftState::Missing,
                details: None,
            });
        }

        let actual_content = std::fs::read(&path).map_err(|e| AdapterError::DetectionFailed(format!(
                "failed to read {}: {e}",
                path.display()
            )))?;
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
        let mdc_content =
            "---\ndescription: test\nglobs: \"**\"\nalwaysApply: true\n---\n\n# Instructions";
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

        let mdc_action = actions
            .iter()
            .find(|a| matches!(a, SyncAction::GenerateMdc { .. }));
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
        assert!(
            actions.is_empty(),
            "expected no actions for unchanged content"
        );
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
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, SyncAction::GenerateMdc { .. }))
        );
    }

    // --- sync_status tests ---

    #[test]
    fn test_sync_status_missing() {
        let dir = TempDir::new().unwrap();

        let status = CursorAdapter
            .sync_status(dir.path(), "abc123", SyncStrategy::Generate)
            .unwrap();
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
        let status = CursorAdapter
            .sync_status(dir.path(), &canonical_hash, SyncStrategy::Generate)
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

        let memory_files = vec![memory_dir.join("debugging.md")];
        let actions = CursorAdapter
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
                assert!(path.to_string_lossy().contains(".cursor/rules/project.mdc"));
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

        let actions = CursorAdapter.plan_memory_sync(dir.path(), &[]).unwrap();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_sync_status_drifted() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".cursor").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        let mdc_content = generate_mdc_content("old instructions");
        std::fs::write(rules_dir.join("project.mdc"), &mdc_content).unwrap();

        let wrong_hash = content_hash(b"different content");
        let status = CursorAdapter
            .sync_status(dir.path(), &wrong_hash, SyncStrategy::Generate)
            .unwrap();
        assert!(matches!(status.drift, DriftState::Drifted { .. }));
    }

    // --- plan_rules_sync tests ---

    fn make_rule(name: &str, desc: Option<&str>, globs: Vec<&str>, always_apply: bool, content: &str) -> crate::types::RuleFile {
        crate::types::RuleFile {
            name: name.to_string(),
            metadata: crate::types::RuleMetadata {
                description: desc.map(|s| s.to_string()),
                globs: globs.into_iter().map(|s| s.to_string()).collect(),
                always_apply,
            },
            content: content.to_string(),
            source_path: std::path::PathBuf::from(format!(".ai/rules/{name}.md")),
        }
    }

    #[test]
    fn test_plan_rules_sync_empty_rules_returns_empty() {
        let dir = TempDir::new().unwrap();
        let actions = CursorAdapter.plan_rules_sync(dir.path(), &[]).unwrap();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_plan_rules_sync_generates_mdc_with_cursor_frontmatter() {
        let dir = TempDir::new().unwrap();
        let rules = vec![make_rule("my-rule", Some("A test rule"), vec!["*.rs", "*.toml"], false, "Rule body content")];

        let actions = CursorAdapter.plan_rules_sync(dir.path(), &rules).unwrap();

        let create_action = actions.iter().find(|a| matches!(a, SyncAction::CreateRuleFile { .. }));
        assert!(create_action.is_some(), "expected CreateRuleFile action");

        if let SyncAction::CreateRuleFile { output, content, rule_name } = create_action.unwrap() {
            assert!(output.to_string_lossy().contains("aisync-my-rule.mdc"));
            assert_eq!(rule_name, "my-rule");
            assert!(content.contains("description: A test rule"));
            assert!(content.contains("globs: \"*.rs, *.toml\""));
            assert!(content.contains("alwaysApply: false"));
            assert!(content.contains("Rule body content"));
        }
    }

    #[test]
    fn test_plan_rules_sync_creates_directory_if_missing() {
        let dir = TempDir::new().unwrap();
        let rules = vec![make_rule("test", None, vec![], true, "Content")];

        let actions = CursorAdapter.plan_rules_sync(dir.path(), &rules).unwrap();

        let dir_action = actions.iter().find(|a| matches!(a, SyncAction::CreateDirectory { .. }));
        assert!(dir_action.is_some(), "expected CreateDirectory action");
    }

    #[test]
    fn test_plan_rules_sync_removes_stale_files() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".cursor/rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        // Create a stale aisync-managed file that no longer has a source rule
        std::fs::write(rules_dir.join("aisync-old-rule.mdc"), "stale").unwrap();

        // Sync with a different set of rules (not containing "old-rule")
        let rules = vec![make_rule("new-rule", None, vec![], true, "New content")];
        let actions = CursorAdapter.plan_rules_sync(dir.path(), &rules).unwrap();

        let remove_action = actions.iter().find(|a| {
            if let SyncAction::RemoveFile { path } = a {
                path.to_string_lossy().contains("aisync-old-rule.mdc")
            } else {
                false
            }
        });
        assert!(remove_action.is_some(), "expected RemoveFile action for stale aisync-old-rule.mdc");
    }

    #[test]
    fn test_plan_rules_sync_idempotent_skip() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".cursor/rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        let rules = vec![make_rule("my-rule", Some("Desc"), vec![], true, "Body")];

        // First sync: should produce CreateRuleFile
        let actions = CursorAdapter.plan_rules_sync(dir.path(), &rules).unwrap();
        let create_action = actions.iter().find(|a| matches!(a, SyncAction::CreateRuleFile { .. }));
        assert!(create_action.is_some());

        // Write the expected file content
        if let SyncAction::CreateRuleFile { output, content, .. } = create_action.unwrap() {
            std::fs::write(output, content).unwrap();
        }

        // Second sync: should NOT produce CreateRuleFile (idempotent)
        let actions2 = CursorAdapter.plan_rules_sync(dir.path(), &rules).unwrap();
        let create_action2 = actions2.iter().find(|a| matches!(a, SyncAction::CreateRuleFile { .. }));
        assert!(create_action2.is_none(), "expected no CreateRuleFile for unchanged content");
    }

    #[test]
    fn test_plan_rules_sync_does_not_remove_non_aisync_files() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".cursor/rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        // User-created file (not aisync- prefixed)
        std::fs::write(rules_dir.join("my-custom.mdc"), "user content").unwrap();

        let rules = vec![make_rule("test", None, vec![], true, "Content")];
        let actions = CursorAdapter.plan_rules_sync(dir.path(), &rules).unwrap();

        let remove_action = actions.iter().find(|a| {
            if let SyncAction::RemoveFile { path } = a {
                path.to_string_lossy().contains("my-custom.mdc")
            } else {
                false
            }
        });
        assert!(remove_action.is_none(), "should NOT remove user-created rule files");
    }

    #[test]
    fn test_generate_cursor_rule_frontmatter_always_apply() {
        let meta = crate::types::RuleMetadata {
            description: Some("Always on rule".to_string()),
            globs: vec![],
            always_apply: true,
        };
        let fm = generate_cursor_rule_frontmatter(&meta);
        assert!(fm.contains("description: Always on rule"));
        assert!(fm.contains("alwaysApply: true"));
        assert!(!fm.contains("globs:"));
    }

    #[test]
    fn test_generate_cursor_rule_frontmatter_with_globs() {
        let meta = crate::types::RuleMetadata {
            description: Some("Glob rule".to_string()),
            globs: vec!["*.rs".to_string(), "*.toml".to_string()],
            always_apply: false,
        };
        let fm = generate_cursor_rule_frontmatter(&meta);
        assert!(fm.contains("globs: \"*.rs, *.toml\""));
        assert!(fm.contains("alwaysApply: false"));
    }

    // --- translate_hooks tests ---

    #[test]
    fn test_translate_hooks_returns_unsupported() {
        use crate::types::{HookGroup, HookHandler, HookTranslation, HooksConfig};
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

        let result = CursorAdapter.translate_hooks(&config).unwrap();
        match result {
            HookTranslation::Unsupported { tool, reason } => {
                assert_eq!(tool, ToolKind::Cursor);
                assert!(reason.contains("Cursor does not support hooks"));
            }
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }
}
