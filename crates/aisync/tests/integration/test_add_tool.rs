use assert_fs::prelude::*;
use predicates::prelude::*;

use crate::helpers::{aisync_cmd, setup_project, STANDARD_CONFIG};

#[test]
fn test_add_tool_without_init() {
    let temp = assert_fs::TempDir::new().unwrap();

    // No aisync.toml in directory
    aisync_cmd()
        .args(["add-tool", "--tool", "windsurf"])
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("aisync init"));
}

#[test]
fn test_add_tool_specific_tool() {
    let temp = setup_project(STANDARD_CONFIG, "# Shared Instructions\n");

    // Create windsurf detection marker
    temp.child(".windsurf/rules").create_dir_all().unwrap();

    aisync_cmd()
        .args(["add-tool", "--tool", "windsurf"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Added"));

    // Verify aisync.toml now contains windsurf
    let toml_content = std::fs::read_to_string(temp.child("aisync.toml").path()).unwrap();
    assert!(
        toml_content.contains("[tools.windsurf]"),
        "aisync.toml should contain [tools.windsurf], got:\n{toml_content}"
    );

    // Verify partial sync ran -- windsurf uses generate strategy, so .windsurf/rules/project.md
    temp.child(".windsurf/rules/project.md")
        .assert(predicate::path::exists());
}

#[test]
fn test_add_tool_already_configured() {
    let temp = setup_project(STANDARD_CONFIG, "# Instructions\n");

    // cursor is already in STANDARD_CONFIG
    aisync_cmd()
        .args(["add-tool", "--tool", "cursor"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("already configured"));
}

#[test]
fn test_add_tool_non_interactive_lists_tools() {
    let temp = setup_project(STANDARD_CONFIG, "# Instructions\n");

    // Create windsurf and codex detection markers
    temp.child(".windsurf/rules").create_dir_all().unwrap();
    temp.child(".codex").create_dir_all().unwrap();

    // assert_cmd pipes stdin, so this is non-interactive
    aisync_cmd()
        .args(["add-tool"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("non-interactive")
                .or(predicate::str::contains("--tool")),
        );
}

#[test]
fn test_add_tool_unknown_tool() {
    let temp = setup_project(STANDARD_CONFIG, "# Instructions\n");

    aisync_cmd()
        .args(["add-tool", "--tool", "nonexistent"])
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown tool"));
}

#[test]
fn test_add_tool_partial_sync_only() {
    let temp = setup_project(STANDARD_CONFIG, "# Shared Instructions\nSome content here.\n");

    // First sync all existing tools
    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // Record modification time of an existing tool file
    let claude_md = temp.child("CLAUDE.md");
    claude_md.assert(predicate::path::exists());
    let before_mtime = std::fs::symlink_metadata(claude_md.path())
        .unwrap()
        .modified()
        .unwrap();

    // Small sleep to ensure mtime difference would be detectable
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Create windsurf marker and add tool
    temp.child(".windsurf/rules").create_dir_all().unwrap();

    aisync_cmd()
        .args(["add-tool", "--tool", "windsurf"])
        .current_dir(temp.path())
        .assert()
        .success();

    // Windsurf file should be created
    temp.child(".windsurf/rules/project.md")
        .assert(predicate::path::exists());

    // Existing symlink should NOT have been re-created
    let after_mtime = std::fs::symlink_metadata(claude_md.path())
        .unwrap()
        .modified()
        .unwrap();

    assert_eq!(
        before_mtime, after_mtime,
        "CLAUDE.md symlink should not have been modified by add-tool (partial sync)"
    );
}
