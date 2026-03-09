pub mod claude_code;
pub mod codex;
pub mod cursor;
pub mod opencode;
pub mod windsurf;

use std::collections::HashSet;
use std::path::PathBuf;

use crate::adapter::AdapterError;
use crate::types::{CommandFile, RuleFile, SyncAction};

/// Shared helper for single-file tools (Claude Code, OpenCode, Codex) that concatenate
/// rule content into a managed section rather than creating individual rule files.
pub(crate) fn plan_single_file_rules_sync(
    target_path: PathBuf,
    rules: &[RuleFile],
) -> Result<Vec<SyncAction>, AdapterError> {
    if rules.is_empty() {
        return Ok(vec![]);
    }

    let mut content = String::new();
    for rule in rules {
        if !rule.content.is_empty() {
            content.push_str(&format!("\n## Rule: {}\n\n", rule.name));
            content.push_str(&rule.content);
            content.push('\n');
        }
    }

    if content.is_empty() {
        return Ok(vec![]);
    }

    Ok(vec![SyncAction::UpdateMemoryReferences {
        path: target_path,
        references: vec![content],
        marker_start: "<!-- aisync:rules -->".to_string(),
        marker_end: "<!-- /aisync:rules -->".to_string(),
    }])
}

/// Shared helper for directory-based tools (Claude Code, Cursor) that sync
/// command files as individual `aisync-{name}.md` files in a commands directory.
pub(crate) fn plan_directory_commands_sync(
    commands_dir: PathBuf,
    commands: &[CommandFile],
) -> Result<Vec<SyncAction>, AdapterError> {
    if commands.is_empty() {
        return Ok(vec![]);
    }

    let mut actions = Vec::new();

    // Build expected filename set
    let expected: HashSet<String> = commands
        .iter()
        .map(|c| format!("aisync-{}.md", c.name))
        .collect();

    // Ensure directory exists
    if !commands_dir.is_dir() {
        actions.push(SyncAction::CreateDirectory {
            path: commands_dir.clone(),
        });
    }

    // Generate copy actions for each command
    for cmd in commands {
        let filename = format!("aisync-{}.md", cmd.name);
        let output = commands_dir.join(&filename);

        // Idempotent: skip if file already has the same content
        if output.exists() {
            if let Ok(existing) = std::fs::read_to_string(&output) {
                if existing == cmd.content {
                    continue;
                }
            }
        }

        actions.push(SyncAction::CopyCommandFile {
            source: cmd.source_path.clone(),
            output,
            command_name: cmd.name.clone(),
        });
    }

    // Scan for stale aisync-* command files
    if commands_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&commands_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("aisync-") && name.ends_with(".md") && !expected.contains(&name)
                {
                    actions.push(SyncAction::RemoveFile {
                        path: entry.path(),
                    });
                }
            }
        }
    }

    Ok(actions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_command(name: &str, content: &str) -> CommandFile {
        CommandFile {
            name: name.to_string(),
            content: content.to_string(),
            source_path: std::path::PathBuf::from(format!(".ai/commands/{name}.md")),
        }
    }

    // --- plan_directory_commands_sync tests ---

    #[test]
    fn test_commands_sync_empty_commands_returns_empty() {
        let dir = TempDir::new().unwrap();
        let actions = plan_directory_commands_sync(dir.path().join("commands"), &[]).unwrap();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_commands_sync_generates_copy_actions() {
        let dir = TempDir::new().unwrap();
        let commands_dir = dir.path().join(".claude/commands");
        std::fs::create_dir_all(&commands_dir).unwrap();

        let commands = vec![
            make_command("build", "Build the project"),
            make_command("test", "Run tests"),
        ];

        let actions = plan_directory_commands_sync(commands_dir, &commands).unwrap();

        let copy_actions: Vec<_> = actions
            .iter()
            .filter(|a| matches!(a, SyncAction::CopyCommandFile { .. }))
            .collect();
        assert_eq!(copy_actions.len(), 2, "should have 2 CopyCommandFile actions");

        // Verify aisync- prefix naming
        if let SyncAction::CopyCommandFile { output, command_name, .. } = &copy_actions[0] {
            assert!(output.to_string_lossy().contains("aisync-build.md") || output.to_string_lossy().contains("aisync-test.md"));
            assert!(command_name == "build" || command_name == "test");
        }
    }

    #[test]
    fn test_commands_sync_creates_directory_when_missing() {
        let dir = TempDir::new().unwrap();
        let commands_dir = dir.path().join(".claude/commands"); // does not exist

        let commands = vec![make_command("build", "Build the project")];
        let actions = plan_directory_commands_sync(commands_dir, &commands).unwrap();

        let dir_action = actions
            .iter()
            .find(|a| matches!(a, SyncAction::CreateDirectory { .. }));
        assert!(dir_action.is_some(), "expected CreateDirectory action when dir missing");
    }

    #[test]
    fn test_commands_sync_skips_identical_content() {
        let dir = TempDir::new().unwrap();
        let commands_dir = dir.path().join(".claude/commands");
        std::fs::create_dir_all(&commands_dir).unwrap();

        // Pre-write file with matching content
        std::fs::write(commands_dir.join("aisync-build.md"), "Build the project").unwrap();

        let commands = vec![make_command("build", "Build the project")];
        let actions = plan_directory_commands_sync(commands_dir, &commands).unwrap();

        let copy_actions: Vec<_> = actions
            .iter()
            .filter(|a| matches!(a, SyncAction::CopyCommandFile { .. }))
            .collect();
        assert!(copy_actions.is_empty(), "should skip when content is identical");
    }

    #[test]
    fn test_commands_sync_removes_stale_files() {
        let dir = TempDir::new().unwrap();
        let commands_dir = dir.path().join(".claude/commands");
        std::fs::create_dir_all(&commands_dir).unwrap();

        // Create stale aisync-managed file
        std::fs::write(commands_dir.join("aisync-old-cmd.md"), "stale").unwrap();

        // Sync with different commands
        let commands = vec![make_command("new-cmd", "New command")];
        let actions = plan_directory_commands_sync(commands_dir, &commands).unwrap();

        let remove_action = actions.iter().find(|a| {
            if let SyncAction::RemoveFile { path } = a {
                path.to_string_lossy().contains("aisync-old-cmd.md")
            } else {
                false
            }
        });
        assert!(remove_action.is_some(), "expected RemoveFile for stale aisync-old-cmd.md");
    }

    #[test]
    fn test_commands_sync_does_not_remove_non_aisync_files() {
        let dir = TempDir::new().unwrap();
        let commands_dir = dir.path().join(".claude/commands");
        std::fs::create_dir_all(&commands_dir).unwrap();

        // User-created file (not aisync- prefixed)
        std::fs::write(commands_dir.join("my-custom.md"), "user content").unwrap();

        let commands = vec![make_command("build", "Build")];
        let actions = plan_directory_commands_sync(commands_dir, &commands).unwrap();

        let remove_custom = actions.iter().find(|a| {
            if let SyncAction::RemoveFile { path } = a {
                path.to_string_lossy().contains("my-custom.md")
            } else {
                false
            }
        });
        assert!(remove_custom.is_none(), "should NOT remove user-created files");
    }
}
