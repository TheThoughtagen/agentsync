use std::path::Path;

use crate::adapter::{DetectionResult, OpenCodeAdapter, ToolAdapter};
use crate::error::AisyncError;
use crate::types::{Confidence, ToolKind};

impl ToolAdapter for OpenCodeAdapter {
    fn name(&self) -> ToolKind {
        ToolKind::OpenCode
    }

    fn detect(&self, _project_root: &Path) -> Result<DetectionResult, AisyncError> {
        // Stub: always returns not detected (RED phase)
        Ok(DetectionResult {
            tool: ToolKind::OpenCode,
            detected: false,
            confidence: Confidence::Medium,
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
    fn test_name_returns_opencode() {
        let adapter = OpenCodeAdapter;
        assert_eq!(adapter.name(), ToolKind::OpenCode);
    }

    #[test]
    fn test_detects_opencode_json() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("opencode.json"), "{}").unwrap();

        let result = OpenCodeAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
        assert!(result
            .markers_found
            .iter()
            .any(|p| p.ends_with("opencode.json")));
    }

    #[test]
    fn test_agents_md_only_medium_confidence() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "# Agents").unwrap();

        let result = OpenCodeAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::Medium);
        assert!(result
            .markers_found
            .iter()
            .any(|p| p.ends_with("AGENTS.md")));
    }

    #[test]
    fn test_both_markers_high_confidence() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("opencode.json"), "{}").unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "# Agents").unwrap();

        let result = OpenCodeAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
        assert_eq!(result.markers_found.len(), 2);
    }

    #[test]
    fn test_not_detected_empty_dir() {
        let dir = TempDir::new().unwrap();

        let result = OpenCodeAdapter.detect(dir.path()).unwrap();
        assert!(!result.detected);
        assert!(result.markers_found.is_empty());
    }
}
