use std::path::Path;

use crate::adapter::{CursorAdapter, DetectionResult, ToolAdapter};
use crate::error::AisyncError;
use crate::types::{Confidence, ToolKind};

impl ToolAdapter for CursorAdapter {
    fn name(&self) -> ToolKind {
        ToolKind::Cursor
    }

    fn detect(&self, _project_root: &Path) -> Result<DetectionResult, AisyncError> {
        // Stub: always returns not detected (RED phase)
        Ok(DetectionResult {
            tool: ToolKind::Cursor,
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
    fn test_name_returns_cursor() {
        let adapter = CursorAdapter;
        assert_eq!(adapter.name(), ToolKind::Cursor);
    }

    #[test]
    fn test_detects_cursor_rules_dir() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".cursor/rules")).unwrap();

        let result = CursorAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
        assert!(result
            .markers_found
            .iter()
            .any(|p| p.ends_with(".cursor/rules")));
    }

    #[test]
    fn test_detects_legacy_cursorrules() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".cursorrules"), "rules here").unwrap();

        let result = CursorAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
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
        assert!(result.version_hint.as_ref().unwrap().contains("legacy"));
    }

    #[test]
    fn test_not_detected_empty_dir() {
        let dir = TempDir::new().unwrap();

        let result = CursorAdapter.detect(dir.path()).unwrap();
        assert!(!result.detected);
        assert!(result.markers_found.is_empty());
        assert!(result.version_hint.is_none());
    }
}
