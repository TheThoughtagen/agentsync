use std::path::Path;

use crate::adapter::{ClaudeCodeAdapter, DetectionResult, ToolAdapter};
use crate::error::AisyncError;
use crate::types::{Confidence, ToolKind};

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_name_returns_claude_code() {
        let adapter = ClaudeCodeAdapter;
        assert_eq!(adapter.name(), ToolKind::ClaudeCode);
    }

    #[test]
    fn test_detects_claude_md() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();

        let result = ClaudeCodeAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
        assert!(result.markers_found.iter().any(|p| p.ends_with("CLAUDE.md")));
    }

    #[test]
    fn test_detects_claude_dir() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".claude")).unwrap();

        let result = ClaudeCodeAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
        assert!(result.markers_found.iter().any(|p| p.ends_with(".claude")));
    }

    #[test]
    fn test_detects_both_markers() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();
        std::fs::create_dir(dir.path().join(".claude")).unwrap();

        let result = ClaudeCodeAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
        assert_eq!(result.markers_found.len(), 2);
    }

    #[test]
    fn test_not_detected_empty_dir() {
        let dir = TempDir::new().unwrap();

        let result = ClaudeCodeAdapter.detect(dir.path()).unwrap();
        assert!(!result.detected);
        assert!(result.markers_found.is_empty());
    }
}
