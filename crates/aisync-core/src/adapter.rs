use std::path::{Path, PathBuf};

use crate::error::AisyncError;
use crate::types::{Confidence, HookTranslation, HooksConfig, SyncAction, ToolKind};

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
pub trait ToolAdapter {
    /// Returns which tool this adapter handles.
    fn name(&self) -> ToolKind;

    /// Detect whether this tool is configured in the given project directory.
    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError>;

    /// Read existing instructions from this tool's native format.
    /// Returns None if no instructions file exists.
    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AisyncError> {
        let _ = project_root;
        todo!("Adapter read_instructions not yet implemented")
    }

    /// Plan sync actions for this tool (does not execute).
    fn plan_sync(
        &self,
        project_root: &Path,
        canonical_content: &str,
        strategy: crate::config::SyncStrategy,
    ) -> Result<Vec<crate::types::SyncAction>, AisyncError> {
        let _ = (project_root, canonical_content, strategy);
        todo!("Adapter plan_sync not yet implemented")
    }

    /// Check sync status for this tool.
    fn sync_status(
        &self,
        project_root: &Path,
        canonical_hash: &str,
    ) -> Result<crate::types::ToolSyncStatus, AisyncError> {
        let _ = (project_root, canonical_hash);
        todo!("Adapter sync_status not yet implemented")
    }

    /// Plan memory sync actions for this tool.
    fn plan_memory_sync(
        &self,
        project_root: &Path,
        memory_files: &[PathBuf],
    ) -> Result<Vec<SyncAction>, AisyncError> {
        let _ = (project_root, memory_files);
        Ok(vec![]) // Default: no memory sync
    }

    /// Translate hooks to this tool's native format.
    fn translate_hooks(
        &self,
        hooks: &HooksConfig,
    ) -> Result<HookTranslation, AisyncError> {
        let _ = hooks;
        Ok(HookTranslation::Unsupported {
            tool: self.name(),
            reason: "hooks not supported by this tool".into(),
        })
    }
}

/// Zero-sized adapter structs for compile-time dispatch.
#[derive(Debug, Clone)]
pub struct ClaudeCodeAdapter;

#[derive(Debug, Clone)]
pub struct CursorAdapter;

#[derive(Debug, Clone)]
pub struct OpenCodeAdapter;

/// Enum-based dispatch for all tool adapters.
///
/// Uses compile-time dispatch (enum) rather than dynamic dispatch (dyn Trait)
/// per research recommendation for a small, fixed set of adapters.
#[derive(Debug, Clone)]
pub enum AnyAdapter {
    ClaudeCode(ClaudeCodeAdapter),
    Cursor(CursorAdapter),
    OpenCode(OpenCodeAdapter),
}

impl AnyAdapter {
    /// Returns one instance of each adapter variant.
    pub fn all() -> Vec<AnyAdapter> {
        vec![
            AnyAdapter::ClaudeCode(ClaudeCodeAdapter),
            AnyAdapter::Cursor(CursorAdapter),
            AnyAdapter::OpenCode(OpenCodeAdapter),
        ]
    }
}

impl ToolAdapter for AnyAdapter {
    fn name(&self) -> ToolKind {
        match self {
            AnyAdapter::ClaudeCode(a) => a.name(),
            AnyAdapter::Cursor(a) => a.name(),
            AnyAdapter::OpenCode(a) => a.name(),
        }
    }

    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError> {
        match self {
            AnyAdapter::ClaudeCode(a) => a.detect(project_root),
            AnyAdapter::Cursor(a) => a.detect(project_root),
            AnyAdapter::OpenCode(a) => a.detect(project_root),
        }
    }

    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AisyncError> {
        match self {
            AnyAdapter::ClaudeCode(a) => a.read_instructions(project_root),
            AnyAdapter::Cursor(a) => a.read_instructions(project_root),
            AnyAdapter::OpenCode(a) => a.read_instructions(project_root),
        }
    }

    fn plan_sync(
        &self,
        project_root: &Path,
        canonical_content: &str,
        strategy: crate::config::SyncStrategy,
    ) -> Result<Vec<crate::types::SyncAction>, AisyncError> {
        match self {
            AnyAdapter::ClaudeCode(a) => a.plan_sync(project_root, canonical_content, strategy),
            AnyAdapter::Cursor(a) => a.plan_sync(project_root, canonical_content, strategy),
            AnyAdapter::OpenCode(a) => a.plan_sync(project_root, canonical_content, strategy),
        }
    }

    fn sync_status(
        &self,
        project_root: &Path,
        canonical_hash: &str,
    ) -> Result<crate::types::ToolSyncStatus, AisyncError> {
        match self {
            AnyAdapter::ClaudeCode(a) => a.sync_status(project_root, canonical_hash),
            AnyAdapter::Cursor(a) => a.sync_status(project_root, canonical_hash),
            AnyAdapter::OpenCode(a) => a.sync_status(project_root, canonical_hash),
        }
    }

    fn plan_memory_sync(
        &self,
        project_root: &Path,
        memory_files: &[PathBuf],
    ) -> Result<Vec<SyncAction>, AisyncError> {
        match self {
            AnyAdapter::ClaudeCode(a) => a.plan_memory_sync(project_root, memory_files),
            AnyAdapter::Cursor(a) => a.plan_memory_sync(project_root, memory_files),
            AnyAdapter::OpenCode(a) => a.plan_memory_sync(project_root, memory_files),
        }
    }

    fn translate_hooks(
        &self,
        hooks: &HooksConfig,
    ) -> Result<HookTranslation, AisyncError> {
        match self {
            AnyAdapter::ClaudeCode(a) => a.translate_hooks(hooks),
            AnyAdapter::Cursor(a) => a.translate_hooks(hooks),
            AnyAdapter::OpenCode(a) => a.translate_hooks(hooks),
        }
    }
}
