use assert_fs::prelude::*;
use predicates::prelude::*;

use crate::helpers::aisync_cmd;

#[test]
fn test_init_creates_ai_directory() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Create a CLAUDE.md so init has something to detect/import
    temp.child("CLAUDE.md").write_str("# My Project\n").unwrap();

    aisync_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success();

    // Verify .ai/instructions.md was created
    temp.child(".ai/instructions.md")
        .assert(predicate::path::exists());
    // Verify aisync.toml was created
    temp.child("aisync.toml").assert(predicate::path::exists());
}

#[test]
fn test_init_imports_existing_claude_md() {
    let temp = assert_fs::TempDir::new().unwrap();

    let content = "# Project Instructions\n\nThis is my project.\n";
    temp.child("CLAUDE.md").write_str(content).unwrap();

    aisync_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success();

    // In non-TTY mode, init imports from the first detected source
    let instructions = std::fs::read_to_string(temp.child(".ai/instructions.md").path()).unwrap();
    assert_eq!(instructions, content);
}

#[test]
fn test_init_idempotent() {
    let temp = assert_fs::TempDir::new().unwrap();

    temp.child("CLAUDE.md").write_str("# Project\n").unwrap();

    // First init
    aisync_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success();

    // Second init in non-TTY mode prints abort message (existing .ai/ found, no --force)
    aisync_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Aborted"));
}
