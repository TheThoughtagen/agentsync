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

// Minimal ToolAdapter implementations for Task 1 compilation.
// Full detection logic is implemented in Task 2 (adapters/ module).

impl ToolAdapter for ClaudeCodeAdapter {
    fn name(&self) -> ToolKind {
        ToolKind::ClaudeCode
    }

    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError> {
        let mut markers = Vec::new();
        let claude_md = project_root.join("CLAUDE.md");
        let claude_dir = project_root.join(".claude");

        if claude_md.exists() {
            markers.push(claude_md);
        }
        if claude_dir.is_dir() {
            markers.push(claude_dir);
        }

        let detected = !markers.is_empty();
        Ok(DetectionResult {
            tool: ToolKind::ClaudeCode,
            detected,
            confidence: Confidence::High,
            markers_found: markers,
            version_hint: None,
        })
    }
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
}

impl ToolAdapter for OpenCodeAdapter {
    fn name(&self) -> ToolKind {
        ToolKind::OpenCode
    }

    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError> {
        let mut markers = Vec::new();
        let opencode_json = project_root.join("opencode.json");
        let agents_md = project_root.join("AGENTS.md");

        let has_opencode = opencode_json.exists();
        let has_agents = agents_md.exists();

        if has_opencode {
            markers.push(opencode_json);
        }
        if has_agents {
            markers.push(agents_md);
        }

        let detected = has_opencode || has_agents;
        let confidence = if has_opencode {
            Confidence::High
        } else {
            Confidence::Medium
        };

        Ok(DetectionResult {
            tool: ToolKind::OpenCode,
            detected,
            confidence,
            markers_found: markers,
            version_hint: None,
        })
    }
}
