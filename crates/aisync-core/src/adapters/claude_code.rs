use std::path::Path;

use crate::adapter::{ClaudeCodeAdapter, DetectionResult, ToolAdapter};
use crate::error::AisyncError;
use crate::types::{Confidence, ToolKind};

impl ToolAdapter for ClaudeCodeAdapter {
    fn name(&self) -> ToolKind {
        ToolKind::ClaudeCode
    }

    fn detect(&self, _project_root: &Path) -> Result<DetectionResult, AisyncError> {
        // Stub: always returns not detected (RED phase)
        Ok(DetectionResult {
            tool: ToolKind::ClaudeCode,
            detected: false,
            confidence: Confidence::High,
            markers_found: Vec::new(),
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
