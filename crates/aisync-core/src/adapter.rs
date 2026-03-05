use std::path::{Path, PathBuf};

use crate::error::AisyncError;
use crate::types::{Confidence, ToolKind};

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

/// Trait for AI tool adapters that can detect their tool's presence.
///
/// Phase 1 trait is intentionally lean: detect() + name() only.
pub trait ToolAdapter {
    /// Returns which tool this adapter handles.
    fn name(&self) -> ToolKind;

    /// Detect whether this tool is configured in the given project directory.
    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError>;
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
}
