use std::path::Path;

use similar::TextDiff;

use crate::adapter::ToolAdapter;
use crate::conditional::ConditionalProcessor;
use crate::config::AisyncConfig;
use crate::error::{AisyncError, SyncError};
use crate::sync::SyncEngine;
use crate::types::ToolDiff;

/// Engine for computing diffs between canonical instructions and tool-native files.
pub struct DiffEngine;

impl DiffEngine {
    /// Compare canonical instructions against each enabled tool's native file.
    /// Returns a ToolDiff per enabled tool showing whether content differs.
    pub fn diff_all(
        config: &AisyncConfig,
        project_root: &Path,
    ) -> Result<Vec<ToolDiff>, AisyncError> {
        let canonical_path = project_root.join(".ai/instructions.md");
        let canonical_content = std::fs::read_to_string(&canonical_path).map_err(|_| {
            AisyncError::Sync(SyncError::CanonicalMissing {
                path: canonical_path.display().to_string(),
            })
        })?;

        let mut diffs = Vec::new();

        for (tool_kind, adapter, _) in SyncEngine::enabled_tools(config, project_root) {
            let tool_file = adapter.native_instruction_path().to_string();

            // Apply conditional processing for this tool
            let expected_content =
                ConditionalProcessor::process(&canonical_content, tool_kind.clone());

            // Read the tool's native content
            let native_content = match adapter.read_instructions(project_root) {
                Ok(Some(content)) => content,
                Ok(None) => String::new(),
                Err(_) => String::new(),
            };

            let text_diff = TextDiff::from_lines(&expected_content, &native_content);
            let unified = text_diff
                .unified_diff()
                .context_radius(3)
                .header(".ai/instructions.md", &tool_file)
                .to_string();

            let has_changes = text_diff.ratio() < 1.0;

            diffs.push(ToolDiff {
                tool: tool_kind,
                has_changes,
                unified_diff: unified,
                tool_file,
            });
        }

        Ok(diffs)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AisyncConfig, DefaultsConfig, SyncStrategy, ToolConfig, ToolsConfig};
    use crate::types::ToolKind;
    use tempfile::TempDir;

    fn setup_canonical(dir: &Path, content: &str) {
        let ai_dir = dir.join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("instructions.md"), content).unwrap();
    }

    fn all_enabled_config() -> AisyncConfig {
        AisyncConfig {
            schema_version: 1,
            defaults: DefaultsConfig {
                sync_strategy: SyncStrategy::Symlink,
            },
            tools: {
                let mut t = ToolsConfig::default();
                t.set_tool("claude-code".into(), ToolConfig {
                    enabled: true,
                    sync_strategy: Some(SyncStrategy::Symlink),
                });
                t.set_tool("cursor".into(), ToolConfig {
                    enabled: true,
                    sync_strategy: Some(SyncStrategy::Generate),
                });
                t.set_tool("opencode".into(), ToolConfig {
                    enabled: true,
                    sync_strategy: Some(SyncStrategy::Symlink),
                });
                t
            },
        }
    }

    #[test]
    fn test_diff_all_returns_no_changes_when_in_sync() {
        let dir = TempDir::new().unwrap();
        let content = "# Instructions\n\nAll tools see this.\n";
        setup_canonical(dir.path(), content);

        // Sync first to put everything in place
        let config = all_enabled_config();
        let plan = SyncEngine::plan(&config, dir.path()).unwrap();
        SyncEngine::execute(&plan, dir.path()).unwrap();

        let diffs = DiffEngine::diff_all(&config, dir.path()).unwrap();
        assert_eq!(diffs.len(), 5);

        // Claude and OpenCode are symlinks -- should be in sync
        let claude_diff = diffs
            .iter()
            .find(|d| d.tool == ToolKind::ClaudeCode)
            .unwrap();
        assert!(
            !claude_diff.has_changes,
            "Claude should be in sync, diff: {}",
            claude_diff.unified_diff
        );

        let opencode_diff = diffs.iter().find(|d| d.tool == ToolKind::OpenCode).unwrap();
        assert!(
            !opencode_diff.has_changes,
            "OpenCode should be in sync, diff: {}",
            opencode_diff.unified_diff
        );
    }

    #[test]
    fn test_diff_all_returns_changes_when_content_differs() {
        let dir = TempDir::new().unwrap();
        let content = "# Instructions\n\nOriginal content.\n";
        setup_canonical(dir.path(), content);

        // Create a different CLAUDE.md (not via sync)
        std::fs::write(dir.path().join("CLAUDE.md"), "# Different Content\n").unwrap();
        // Create a different AGENTS.md
        std::fs::write(dir.path().join("AGENTS.md"), "# Different Agents\n").unwrap();

        let config = all_enabled_config();
        let diffs = DiffEngine::diff_all(&config, dir.path()).unwrap();

        let claude_diff = diffs
            .iter()
            .find(|d| d.tool == ToolKind::ClaudeCode)
            .unwrap();
        assert!(claude_diff.has_changes, "Claude should have changes");
        assert!(
            !claude_diff.unified_diff.is_empty(),
            "Should have non-empty diff"
        );

        let opencode_diff = diffs.iter().find(|d| d.tool == ToolKind::OpenCode).unwrap();
        assert!(opencode_diff.has_changes, "OpenCode should have changes");
    }

    #[test]
    fn test_diff_all_missing_tool_file_shows_changes() {
        let dir = TempDir::new().unwrap();
        setup_canonical(dir.path(), "# Instructions\n");

        // No tool files exist
        let config = all_enabled_config();
        let diffs = DiffEngine::diff_all(&config, dir.path()).unwrap();

        for diff in &diffs {
            assert!(
                diff.has_changes,
                "Missing tool file should show changes for {:?}",
                diff.tool
            );
        }
    }
}
