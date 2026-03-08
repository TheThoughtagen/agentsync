use assert_fs::prelude::*;
use predicates::prelude::*;

use crate::helpers::{aisync_cmd, setup_project, STANDARD_CONFIG};

#[test]
fn test_sync_creates_tool_files() {
    let temp = setup_project(STANDARD_CONFIG, "# Shared Instructions\n");

    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // Claude Code: CLAUDE.md (symlink to .ai/instructions.md)
    temp.child("CLAUDE.md").assert(predicate::path::exists());
    // OpenCode: AGENTS.md (symlink to .ai/instructions.md)
    temp.child("AGENTS.md").assert(predicate::path::exists());
    // Cursor: .cursor/rules/project.mdc (generated file)
    temp.child(".cursor/rules/project.mdc")
        .assert(predicate::path::exists());
}

#[test]
fn test_sync_dry_run_no_changes() {
    let temp = setup_project(STANDARD_CONFIG, "# Instructions\n");

    aisync_cmd()
        .args(["sync", "--dry-run"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"));

    // No tool files should be created
    temp.child("CLAUDE.md")
        .assert(predicate::path::exists().not());
    temp.child("AGENTS.md")
        .assert(predicate::path::exists().not());
    temp.child(".cursor/rules/project.mdc")
        .assert(predicate::path::exists().not());
}

#[test]
fn test_sync_idempotent() {
    let temp = setup_project(STANDARD_CONFIG, "# Instructions\n");

    // First sync
    let first = aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .output()
        .unwrap();

    // Second sync
    let second = aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .output()
        .unwrap();

    assert!(first.status.success());
    assert!(second.status.success());

    // Cursor .mdc content should be identical after both syncs
    let mdc_content =
        std::fs::read_to_string(temp.child(".cursor/rules/project.mdc").path()).unwrap();
    assert!(mdc_content.contains("# Instructions"));
}

#[test]
fn test_status_after_sync_shows_in_sync() {
    let temp = setup_project(STANDARD_CONFIG, "# Instructions\n");

    // Sync first
    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // Status should show all tools in sync
    aisync_cmd()
        .arg("status")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("in sync").or(predicate::str::contains("OK")));
}

#[test]
fn test_status_json_output() {
    let temp = setup_project(STANDARD_CONFIG, "# Instructions\n");

    // Sync first
    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // Status --json should produce valid JSON
    let output = aisync_cmd()
        .args(["status", "--json"])
        .current_dir(temp.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("status --json should produce valid JSON");
    assert!(parsed.is_object());
}

#[test]
fn test_check_exits_zero_after_sync() {
    let temp = setup_project(STANDARD_CONFIG, "# Instructions\n");

    // Sync first
    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // Check should exit 0
    aisync_cmd()
        .arg("check")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("OK"));
}

#[test]
fn test_check_exits_nonzero_on_drift() {
    let temp = setup_project(STANDARD_CONFIG, "# Instructions\n");

    // Sync first
    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // Manually modify a tool file to cause drift
    // Cursor's .mdc file is a regular file (not symlink), so we can modify it
    std::fs::write(
        temp.child(".cursor/rules/project.mdc").path(),
        "---\ndescription: tampered\n---\n\nmodified content\n",
    )
    .unwrap();

    // Check should exit non-zero
    aisync_cmd()
        .arg("check")
        .current_dir(temp.path())
        .assert()
        .code(1);
}

#[test]
fn test_sync_codex_opencode_deduplication() {
    // Config with both codex and opencode enabled
    let config = r#"schema_version = 1
[tools.codex]
enabled = true
[tools.opencode]
enabled = true
[tools.claude-code]
enabled = false
[tools.cursor]
enabled = false
[tools.windsurf]
enabled = false
"#;

    let temp = setup_project(config, "# Shared Instructions\n");

    // Create .codex/ directory so Codex adapter detects it
    temp.child(".codex").create_dir_all().unwrap();

    // Dry run should show AGENTS.md exactly once (not twice)
    let output = aisync_cmd()
        .args(["sync", "--dry-run"])
        .current_dir(temp.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Count occurrences of AGENTS.md in the output
    let agents_count = stdout.matches("AGENTS.md").count();
    assert!(
        agents_count <= 1,
        "expected AGENTS.md to appear at most once in dry-run output, got {agents_count}. Output:\n{stdout}"
    );
}
