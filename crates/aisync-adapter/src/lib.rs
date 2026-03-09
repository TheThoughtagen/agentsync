//! ToolAdapter trait and supporting types for building aisync adapters.
//!
//! Community developers can depend on this crate (plus `aisync-types`) to
//! implement custom tool adapters without pulling in all of `aisync-core`.

pub use aisync_types;

use std::path::{Path, PathBuf};

use aisync_types::{
    Confidence, DriftState, HookTranslation, HooksConfig, SyncAction, SyncStrategy, ToolKind,
    ToolSyncStatus,
};

/// Errors specific to individual tool adapters.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("detection failed: {0}")]
    DetectionFailed(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

/// Result of detecting a specific AI tool in a project directory.
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// Which tool was checked.
    pub tool: ToolKind,
    /// Whether the tool was detected.
    pub detected: bool,
    /// Confidence level of the detection.
    pub confidence: Confidence,
    /// Filesystem markers that were found (files/directories).
    pub markers_found: Vec<PathBuf>,
    /// Optional hint about the tool version or configuration format.
    pub version_hint: Option<String>,
}

/// Trait for AI tool adapters that can detect, read, sync, and check status.
///
/// All adapters must be Send + Sync to support the Plugin variant (Arc-wrapped).
pub trait ToolAdapter: Send + Sync {
    /// Returns which tool this adapter handles.
    fn name(&self) -> ToolKind;

    /// Human-readable display name (e.g., "Claude Code", "Cursor", "OpenCode").
    fn display_name(&self) -> &str;

    /// Relative path from project root to the tool's native instruction file.
    fn native_instruction_path(&self) -> &str;

    /// Conditional tag names that match this tool (e.g., ["claude-only", "claude-code-only"]).
    fn conditional_tags(&self) -> &[&str] {
        &[]
    }

    /// Entries to add to .gitignore when this tool is synced.
    fn gitignore_entries(&self) -> Vec<String> {
        vec![]
    }

    /// Relative paths to watch for reverse sync (defaults to native_instruction_path).
    fn watch_paths(&self) -> Vec<&str> {
        vec![self.native_instruction_path()]
    }

    /// Default sync strategy for this tool (overridable in config).
    fn default_sync_strategy(&self) -> SyncStrategy {
        SyncStrategy::Symlink
    }

    /// Detect whether this tool is configured in the given project directory.
    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AdapterError>;

    /// Read existing instructions from this tool's native format.
    /// Returns None if no instructions file exists.
    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AdapterError> {
        let _ = project_root;
        Ok(None)
    }

    /// Plan sync actions for this tool (does not execute).
    fn plan_sync(
        &self,
        project_root: &Path,
        canonical_content: &str,
        strategy: SyncStrategy,
    ) -> Result<Vec<SyncAction>, AdapterError> {
        let _ = (project_root, canonical_content, strategy);
        Ok(vec![])
    }

    /// Check sync status for this tool.
    fn sync_status(
        &self,
        project_root: &Path,
        canonical_hash: &str,
        strategy: SyncStrategy,
    ) -> Result<ToolSyncStatus, AdapterError> {
        let _ = (project_root, canonical_hash);
        Ok(ToolSyncStatus {
            tool: self.name(),
            strategy,
            drift: DriftState::NotConfigured,
            details: None,
        })
    }

    /// Plan memory sync actions for this tool.
    fn plan_memory_sync(
        &self,
        project_root: &Path,
        memory_files: &[PathBuf],
    ) -> Result<Vec<SyncAction>, AdapterError> {
        let _ = (project_root, memory_files);
        Ok(vec![]) // Default: no memory sync
    }

    /// Translate hooks to this tool's native format.
    fn translate_hooks(&self, hooks: &HooksConfig) -> Result<HookTranslation, AdapterError> {
        let _ = hooks;
        Ok(HookTranslation::Unsupported {
            tool: self.name(),
            reason: "hooks not supported by this tool".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection_result_fields() {
        let result = DetectionResult {
            tool: ToolKind::ClaudeCode,
            detected: true,
            confidence: Confidence::High,
            markers_found: vec![PathBuf::from("CLAUDE.md")],
            version_hint: Some("v1".to_string()),
        };
        assert_eq!(result.tool, ToolKind::ClaudeCode);
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
        assert_eq!(result.markers_found.len(), 1);
        assert_eq!(result.version_hint, Some("v1".to_string()));
    }

    #[test]
    fn test_adapter_error_detection_failed() {
        let err = AdapterError::DetectionFailed("no markers".to_string());
        assert!(format!("{err}").contains("no markers"));
    }

    #[test]
    fn test_adapter_error_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: AdapterError = io_err.into();
        assert!(matches!(err, AdapterError::Io(_)));
        assert!(format!("{err}").contains("file not found"));
    }

    #[test]
    fn test_adapter_error_other() {
        let err = AdapterError::Other("something went wrong".to_string());
        assert!(format!("{err}").contains("something went wrong"));
    }
}
