use std::path::{Path, PathBuf};

use crate::adapter::{DetectionResult, ToolAdapter, WindsurfAdapter};
use crate::config::SyncStrategy;
use crate::error::AisyncError;
use crate::types::{Confidence, DriftState, SyncAction, ToolKind, ToolSyncStatus};

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

    fn default_sync_strategy(&self) -> crate::config::SyncStrategy {
        crate::config::SyncStrategy::Generate
    }

    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError> {
        let _ = project_root;
        Ok(DetectionResult {
            tool: ToolKind::Windsurf,
            detected: false,
            confidence: Confidence::High,
            markers_found: vec![],
            version_hint: None,
        })
    }

    fn plan_sync(
        &self,
        project_root: &Path,
        canonical_content: &str,
        _strategy: SyncStrategy,
    ) -> Result<Vec<SyncAction>, AisyncError> {
        let output_path = project_root.join(WINDSURF_REL);
        let expected_content = generate_windsurf_content(canonical_content);

        let mut actions = Vec::new();

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
        _canonical_hash: &str,
        _strategy: SyncStrategy,
    ) -> Result<ToolSyncStatus, AisyncError> {
        let path = project_root.join(WINDSURF_REL);
        let drift = if path.exists() {
            DriftState::InSync
        } else {
            DriftState::Missing
        };
        Ok(ToolSyncStatus {
            tool: ToolKind::Windsurf,
            strategy: SyncStrategy::Generate,
            drift,
            details: None,
        })
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
}
