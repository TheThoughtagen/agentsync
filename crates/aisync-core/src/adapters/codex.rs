use std::path::{Path, PathBuf};

use crate::adapter::{CodexAdapter, DetectionResult, ToolAdapter};
use crate::config::SyncStrategy;
use crate::error::AisyncError;
use crate::types::{Confidence, DriftState, SyncAction, ToolKind, ToolSyncStatus, content_hash};

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
    ) -> Result<Vec<SyncAction>, AisyncError> {
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

    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError> {
        let _ = project_root;
        Ok(DetectionResult {
            tool: ToolKind::Codex,
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
        strategy: SyncStrategy,
    ) -> Result<Vec<SyncAction>, AisyncError> {
        let link_path = project_root.join(TOOL_FILE);
        let target_rel = std::path::Path::new(CANONICAL_REL);

        // Determine whether conditionals changed the content
        let raw_path = project_root.join(CANONICAL_REL);
        let conditionals_active = match std::fs::read_to_string(&raw_path) {
            Ok(raw_content) => canonical_content != raw_content,
            Err(_) => false,
        };

        if conditionals_active || strategy == SyncStrategy::Copy {
            return self.plan_sync_with_conditionals(&link_path, canonical_content);
        }

        if link_path.exists() || link_path.symlink_metadata().is_ok() {
            if let Ok(meta) = link_path.symlink_metadata() {
                if meta.file_type().is_symlink() {
                    let current_target =
                        std::fs::read_link(&link_path).map_err(|e| AisyncError::Adapter {
                            tool: "codex".to_string(),
                            source: crate::error::AdapterError::DetectionFailed(format!(
                                "failed to read symlink: {e}"
                            )),
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
                    reason: format!("{} is a regular file, not managed by aisync", TOOL_FILE),
                }]);
            }
        }

        Ok(vec![SyncAction::CreateSymlink {
            link: link_path,
            target: target_rel.to_path_buf(),
        }])
    }

    fn sync_status(
        &self,
        project_root: &Path,
        canonical_hash: &str,
        strategy: SyncStrategy,
    ) -> Result<ToolSyncStatus, AisyncError> {
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

            let content = std::fs::read(&path).map_err(|e| AisyncError::Adapter {
                tool: "codex".to_string(),
                source: crate::error::AdapterError::DetectionFailed(format!(
                    "failed to read {}: {e}",
                    path.display()
                )),
            })?;
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

        // Regular file
        let content = std::fs::read(&path).map_err(|e| AisyncError::Adapter {
            tool: "codex".to_string(),
            source: crate::error::AdapterError::DetectionFailed(format!(
                "failed to read {}: {e}",
                path.display()
            )),
        })?;
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
            path: project_root.join(TOOL_FILE),
            references,
            marker_start: "<!-- aisync:memory -->".to_string(),
            marker_end: "<!-- /aisync:memory -->".to_string(),
        }])
    }
}
