use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::adapter::{DetectionResult, ToolAdapter, WindsurfAdapter};
use crate::config::SyncStrategy;
use crate::adapter::AdapterError;
use crate::types::{
    Confidence, DriftState, HookTranslation, HooksConfig, RuleFile, RuleMetadata, SyncAction,
    ToolKind, ToolSyncStatus, content_hash,
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

/// Generate Windsurf-format YAML frontmatter for a rule file.
///
/// Maps canonical metadata to Windsurf trigger types:
/// - always_apply=true -> trigger: always_on
/// - always_apply=false + globs -> trigger: glob
/// - always_apply=false + description (no globs) -> trigger: model_decision
/// - fallback -> trigger: manual
fn generate_windsurf_rule_frontmatter(meta: &RuleMetadata) -> String {
    let mut fm = String::from("---\n");
    if meta.always_apply {
        fm.push_str("trigger: always_on\n");
    } else if !meta.globs.is_empty() {
        fm.push_str("trigger: glob\n");
        fm.push_str(&format!("globs: {}\n", meta.globs.join(", ")));
    } else if meta.description.is_some() {
        fm.push_str("trigger: model_decision\n");
    } else {
        fm.push_str("trigger: manual\n");
    }
    if let Some(desc) = &meta.description {
        fm.push_str(&format!("description: {}\n", desc));
    }
    fm.push_str("---\n\n");
    fm
}

/// Generate full Windsurf rule file content (frontmatter + body).
fn generate_windsurf_rule_content(meta: &RuleMetadata, body: &str) -> String {
    format!("{}{}", generate_windsurf_rule_frontmatter(meta), body)
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

    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AdapterError> {
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

    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AdapterError> {
        let path = project_root.join(WINDSURF_REL);
        if !path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| AdapterError::DetectionFailed(format!(
                "failed to read {}: {e}",
                path.display()
            )))?;

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
    ) -> Result<Vec<SyncAction>, AdapterError> {
        // Windsurf always uses Generate strategy
        let output_path = project_root.join(WINDSURF_REL);
        let expected_content = generate_windsurf_content(canonical_content);

        let mut actions = Vec::new();

        // Ensure directory exists
        let rules_dir = project_root.join(".windsurf").join("rules");
        if !rules_dir.is_dir() {
            actions.push(SyncAction::CreateDirectory { path: rules_dir });
        }

        // Check content size limit (Windsurf has 12K char limit)
        let char_count = canonical_content.chars().count();
        if char_count > 12_000 {
            actions.push(SyncAction::WarnContentSize {
                tool: ToolKind::Windsurf,
                path: output_path.clone(),
                actual_size: char_count,
                limit: 12_000,
                unit: "chars".to_string(),
            });
        }

        if output_path.exists() {
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
    ) -> Result<ToolSyncStatus, AdapterError> {
        let path = project_root.join(WINDSURF_REL);

        if !path.exists() {
            return Ok(ToolSyncStatus {
                tool: ToolKind::Windsurf,
                strategy: SyncStrategy::Generate,
                drift: DriftState::Missing,
                details: None,
            });
        }

        let actual_content = std::fs::read(&path).map_err(|e| AdapterError::DetectionFailed(format!(
                "failed to read {}: {e}",
                path.display()
            )))?;

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
            path: project_root.join(WINDSURF_REL),
            references,
            marker_start: "<!-- aisync:memory -->".to_string(),
            marker_end: "<!-- /aisync:memory -->".to_string(),
        }])
    }

    fn translate_hooks(&self, _hooks: &HooksConfig) -> Result<HookTranslation, AdapterError> {
        Ok(HookTranslation::Unsupported {
            tool: ToolKind::Windsurf,
            reason: "Windsurf does not support hooks".to_string(),
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
        let rules_dir = project_root.join(".windsurf").join("rules");

        // Ensure directory exists
        if !rules_dir.is_dir() {
            actions.push(SyncAction::CreateDirectory {
                path: rules_dir.clone(),
            });
        }

        // Build expected filenames set
        let expected: HashSet<String> = rules
            .iter()
            .map(|r| format!("aisync-{}.md", r.name))
            .collect();

        // Generate rule files
        for rule in rules {
            let filename = format!("aisync-{}.md", rule.name);
            let output = rules_dir.join(&filename);
            let content = generate_windsurf_rule_content(&rule.metadata, &rule.content);

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
                    if name.starts_with("aisync-") && name.ends_with(".md") && !expected.contains(&name) {
                        actions.push(SyncAction::RemoveFile {
                            path: entry.path(),
                        });
                    }
                }
            }
        }

        Ok(actions)
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
        let actions = WindsurfAdapter.plan_rules_sync(dir.path(), &[]).unwrap();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_plan_rules_sync_generates_md_with_windsurf_frontmatter() {
        let dir = TempDir::new().unwrap();
        let rules = vec![make_rule("my-rule", Some("A test rule"), vec![], true, "Rule body")];

        let actions = WindsurfAdapter.plan_rules_sync(dir.path(), &rules).unwrap();

        let create_action = actions.iter().find(|a| matches!(a, SyncAction::CreateRuleFile { .. }));
        assert!(create_action.is_some(), "expected CreateRuleFile action");

        if let SyncAction::CreateRuleFile { output, content, rule_name } = create_action.unwrap() {
            assert!(output.to_string_lossy().contains("aisync-my-rule.md"));
            assert_eq!(rule_name, "my-rule");
            assert!(content.contains("trigger: always_on"));
            assert!(content.contains("description: A test rule"));
            assert!(content.contains("Rule body"));
        }
    }

    #[test]
    fn test_plan_rules_sync_trigger_glob() {
        let dir = TempDir::new().unwrap();
        let rules = vec![make_rule("glob-rule", Some("Glob rule"), vec!["*.rs", "*.toml"], false, "Content")];

        let actions = WindsurfAdapter.plan_rules_sync(dir.path(), &rules).unwrap();
        let create_action = actions.iter().find(|a| matches!(a, SyncAction::CreateRuleFile { .. }));
        assert!(create_action.is_some());

        if let SyncAction::CreateRuleFile { content, .. } = create_action.unwrap() {
            assert!(content.contains("trigger: glob"));
            assert!(content.contains("globs: *.rs, *.toml"));
        }
    }

    #[test]
    fn test_plan_rules_sync_trigger_model_decision() {
        let dir = TempDir::new().unwrap();
        let rules = vec![make_rule("desc-rule", Some("Description only"), vec![], false, "Content")];

        let actions = WindsurfAdapter.plan_rules_sync(dir.path(), &rules).unwrap();
        let create_action = actions.iter().find(|a| matches!(a, SyncAction::CreateRuleFile { .. }));
        assert!(create_action.is_some());

        if let SyncAction::CreateRuleFile { content, .. } = create_action.unwrap() {
            assert!(content.contains("trigger: model_decision"));
        }
    }

    #[test]
    fn test_plan_rules_sync_trigger_manual() {
        let dir = TempDir::new().unwrap();
        let rules = vec![make_rule("manual-rule", None, vec![], false, "Content")];

        let actions = WindsurfAdapter.plan_rules_sync(dir.path(), &rules).unwrap();
        let create_action = actions.iter().find(|a| matches!(a, SyncAction::CreateRuleFile { .. }));
        assert!(create_action.is_some());

        if let SyncAction::CreateRuleFile { content, .. } = create_action.unwrap() {
            assert!(content.contains("trigger: manual"));
        }
    }

    #[test]
    fn test_plan_rules_sync_removes_stale_files() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".windsurf/rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::write(rules_dir.join("aisync-old-rule.md"), "stale").unwrap();

        let rules = vec![make_rule("new-rule", None, vec![], true, "New content")];
        let actions = WindsurfAdapter.plan_rules_sync(dir.path(), &rules).unwrap();

        let remove_action = actions.iter().find(|a| {
            if let SyncAction::RemoveFile { path } = a {
                path.to_string_lossy().contains("aisync-old-rule.md")
            } else {
                false
            }
        });
        assert!(remove_action.is_some(), "expected RemoveFile for stale aisync-old-rule.md");
    }

    #[test]
    fn test_plan_rules_sync_does_not_remove_non_aisync_files() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".windsurf/rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::write(rules_dir.join("my-custom.md"), "user content").unwrap();

        let rules = vec![make_rule("test", None, vec![], true, "Content")];
        let actions = WindsurfAdapter.plan_rules_sync(dir.path(), &rules).unwrap();

        let remove_action = actions.iter().find(|a| {
            if let SyncAction::RemoveFile { path } = a {
                path.to_string_lossy().contains("my-custom.md")
            } else {
                false
            }
        });
        assert!(remove_action.is_none(), "should NOT remove user-created files");
    }

    #[test]
    fn test_generate_windsurf_rule_frontmatter_always_on() {
        let meta = crate::types::RuleMetadata {
            description: Some("Always on".to_string()),
            globs: vec![],
            always_apply: true,
        };
        let fm = generate_windsurf_rule_frontmatter(&meta);
        assert!(fm.contains("trigger: always_on"));
        assert!(fm.contains("description: Always on"));
    }

    #[test]
    fn test_generate_windsurf_rule_frontmatter_glob() {
        let meta = crate::types::RuleMetadata {
            description: Some("Glob rule".to_string()),
            globs: vec!["*.rs".to_string()],
            always_apply: false,
        };
        let fm = generate_windsurf_rule_frontmatter(&meta);
        assert!(fm.contains("trigger: glob"));
        assert!(fm.contains("globs: *.rs"));
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

    #[test]
    fn test_plan_sync_warns_on_large_content() {
        let dir = TempDir::new().unwrap();

        // Create content with > 12,000 chars
        let large_content = "x".repeat(12_001);

        let actions = WindsurfAdapter
            .plan_sync(dir.path(), &large_content, SyncStrategy::Generate)
            .unwrap();

        let warn_action = actions
            .iter()
            .find(|a| matches!(a, SyncAction::WarnContentSize { .. }));
        assert!(
            warn_action.is_some(),
            "expected WarnContentSize action for content > 12K chars"
        );

        if let SyncAction::WarnContentSize {
            tool,
            actual_size,
            limit,
            unit,
            ..
        } = warn_action.unwrap()
        {
            assert_eq!(*tool, ToolKind::Windsurf);
            assert!(*actual_size > 12_000);
            assert_eq!(*limit, 12_000);
            assert_eq!(unit, "chars");
        }

        // Warning should come before CreateFile
        let warn_idx = actions
            .iter()
            .position(|a| matches!(a, SyncAction::WarnContentSize { .. }))
            .unwrap();
        let create_idx = actions
            .iter()
            .position(|a| matches!(a, SyncAction::CreateFile { .. }))
            .unwrap();
        assert!(
            warn_idx < create_idx,
            "WarnContentSize should come before CreateFile"
        );
    }

    #[test]
    fn test_plan_sync_no_warning_under_limit() {
        let dir = TempDir::new().unwrap();

        // Content under 12K chars
        let small_content = "x".repeat(11_999);

        let actions = WindsurfAdapter
            .plan_sync(dir.path(), &small_content, SyncStrategy::Generate)
            .unwrap();

        let warn_action = actions
            .iter()
            .find(|a| matches!(a, SyncAction::WarnContentSize { .. }));
        assert!(
            warn_action.is_none(),
            "expected no WarnContentSize for content under 12K chars"
        );
    }
}
