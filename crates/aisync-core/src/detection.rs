use std::path::Path;

use crate::adapter::{AnyAdapter, DetectionResult, ToolAdapter};
use crate::declarative::discover_toml_adapters;
use crate::error::DetectionError;

/// Engine that scans a project directory for AI tool markers.
pub struct DetectionEngine;

impl DetectionEngine {
    /// Scan a project directory and return detection results for all detected tools.
    ///
    /// Only returns results where `detected == true`. Returns an empty vec
    /// if no tools are found.
    pub fn scan(project_root: &Path) -> Result<Vec<DetectionResult>, DetectionError> {
        if !project_root.exists() || !project_root.is_dir() {
            return Err(DetectionError::ScanFailed {
                path: project_root.display().to_string(),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "path does not exist or is not a directory",
                ),
            });
        }

        let mut results = Vec::new();
        for adapter in AnyAdapter::all_builtin() {
            match adapter.detect(project_root) {
                Ok(result) if result.detected => results.push(result),
                Ok(_) => {} // Not detected, skip
                Err(e) => {
                    return Err(DetectionError::ScanFailed {
                        path: project_root.display().to_string(),
                        source: std::io::Error::other(format!("adapter error: {e}")),
                    });
                }
            }
        }

        // Include TOML-defined adapters in detection.
        // Errors from TOML adapters are non-fatal (user-provided, fail gracefully).
        for adapter in discover_toml_adapters(project_root) {
            match adapter.detect(project_root) {
                Ok(result) if result.detected => results.push(result),
                Ok(_) => {} // Not detected, skip
                Err(e) => {
                    eprintln!(
                        "Warning: TOML adapter detection error for {}: {e}",
                        adapter.display_name()
                    );
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Confidence, ToolKind};
    use tempfile::TempDir;

    #[test]
    fn test_empty_directory_returns_no_results() {
        let dir = TempDir::new().unwrap();
        let results = DetectionEngine::scan(dir.path()).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_claude_only_returns_one_result() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();

        let results = DetectionEngine::scan(dir.path()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool, ToolKind::ClaudeCode);
        assert_eq!(results[0].confidence, Confidence::High);
    }

    #[test]
    fn test_multi_tool_returns_all_three() {
        let dir = TempDir::new().unwrap();
        // Claude Code markers
        std::fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();
        // Cursor markers
        std::fs::create_dir_all(dir.path().join(".cursor/rules")).unwrap();
        // OpenCode markers
        std::fs::write(dir.path().join("opencode.json"), "{}").unwrap();

        let results = DetectionEngine::scan(dir.path()).unwrap();
        assert_eq!(results.len(), 3);

        let tools: Vec<ToolKind> = results.iter().map(|r| r.tool.clone()).collect();
        assert!(tools.contains(&ToolKind::ClaudeCode));
        assert!(tools.contains(&ToolKind::Cursor));
        assert!(tools.contains(&ToolKind::OpenCode));
    }

    #[test]
    fn test_ambiguous_agents_md_medium_confidence() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "# Agents").unwrap();

        let results = DetectionEngine::scan(dir.path()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool, ToolKind::OpenCode);
        assert_eq!(results[0].confidence, Confidence::Medium);
    }

    #[test]
    fn test_nonexistent_path_returns_error() {
        let result = DetectionEngine::scan(Path::new("/nonexistent/path/xyz"));
        assert!(result.is_err());
    }

    #[test]
    fn test_file_path_returns_error() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("not_a_dir.txt");
        std::fs::write(&file, "content").unwrap();

        let result = DetectionEngine::scan(&file);
        assert!(result.is_err());
    }

    #[test]
    fn test_cursor_legacy_detected_with_hint() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".cursorrules"), "rules").unwrap();

        let results = DetectionEngine::scan(dir.path()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool, ToolKind::Cursor);
        assert!(results[0].version_hint.as_ref().unwrap().contains("legacy"));
    }

    #[test]
    fn test_scan_includes_toml_adapters() {
        let dir = TempDir::new().unwrap();

        // Create CLAUDE.md for builtin detection
        std::fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();

        // Create TOML adapter with detection markers
        let adapters_dir = dir.path().join(".ai/adapters");
        std::fs::create_dir_all(&adapters_dir).unwrap();
        let toml = r#"
name = "aider"
display_name = "Aider"

[detection]
directories = [".aider"]

[sync]
instruction_path = ".aider/rules/project.md"
"#;
        std::fs::write(adapters_dir.join("aider.toml"), toml).unwrap();

        // Create the detection marker
        std::fs::create_dir(dir.path().join(".aider")).unwrap();

        let results = DetectionEngine::scan(dir.path()).unwrap();
        let tools: Vec<ToolKind> = results.iter().map(|r| r.tool.clone()).collect();
        assert!(tools.contains(&ToolKind::ClaudeCode), "should detect ClaudeCode");
        assert!(
            tools.contains(&ToolKind::Custom("aider".to_string())),
            "should detect TOML-defined aider adapter"
        );
    }

    #[test]
    fn test_scan_toml_adapter_not_detected_without_markers() {
        let dir = TempDir::new().unwrap();

        // Create TOML adapter but NOT its detection markers
        let adapters_dir = dir.path().join(".ai/adapters");
        std::fs::create_dir_all(&adapters_dir).unwrap();
        let toml = r#"
name = "aider"
display_name = "Aider"

[detection]
directories = [".aider"]

[sync]
instruction_path = ".aider/rules/project.md"
"#;
        std::fs::write(adapters_dir.join("aider.toml"), toml).unwrap();

        let results = DetectionEngine::scan(dir.path()).unwrap();
        let tools: Vec<ToolKind> = results.iter().map(|r| r.tool.clone()).collect();
        assert!(
            !tools.contains(&ToolKind::Custom("aider".to_string())),
            "should NOT detect aider without markers"
        );
    }
}
