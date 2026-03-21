use assert_fs::prelude::*;
use predicates::prelude::*;

use crate::helpers::{aisync_cmd, setup_project};

/// Config with Claude Code and Cursor enabled.
const CC_CURSOR_CONFIG: &str = r#"schema_version = 1
[tools.claude-code]
enabled = true
[tools.cursor]
enabled = true
sync_strategy = "generate"
"#;

/// Config with all three hook-capable tools enabled.
const ALL_HOOKS_CONFIG: &str = r#"schema_version = 1
[tools.claude-code]
enabled = true
[tools.cursor]
enabled = true
sync_strategy = "generate"
[tools.opencode]
enabled = true
"#;

// =============================================================================
// Hook import during init
// =============================================================================

#[test]
fn test_init_imports_hooks_from_claude_code_settings() {
    let temp = assert_fs::TempDir::new().unwrap();
    temp.child("CLAUDE.md").write_str("# Test").unwrap();
    temp.child(".claude").create_dir_all().unwrap();
    temp.child(".claude/settings.json")
        .write_str(r#"{
            "hooks": {
                "PostToolUse": [
                    {
                        "matcher": "Edit|Write",
                        "hooks": [{"type": "command", "command": "echo lint", "timeout": 30}]
                    }
                ]
            }
        }"#)
        .unwrap();

    aisync_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success();

    // hooks.toml should exist with imported hooks
    let hooks_toml = std::fs::read_to_string(temp.child(".ai/hooks.toml").path()).unwrap();
    assert!(hooks_toml.contains("PostToolUse"), "should import PostToolUse event");
    assert!(hooks_toml.contains("echo lint"), "should import hook command");
    // Timeout should be converted from 30s to 30000ms
    assert!(hooks_toml.contains("30000"), "timeout should be converted to milliseconds");
}

#[test]
fn test_init_skips_aisync_generated_cursor_hooks() {
    let temp = assert_fs::TempDir::new().unwrap();
    temp.child("CLAUDE.md").write_str("# Test").unwrap();
    temp.child(".claude").create_dir_all().unwrap();
    temp.child(".claude/settings.json")
        .write_str(r#"{
            "hooks": {
                "PostToolUse": [
                    {
                        "matcher": "Edit",
                        "hooks": [{"type": "command", "command": "echo lint", "timeout": 10}]
                    }
                ]
            }
        }"#)
        .unwrap();

    // Cursor hooks.json with aisync-generated content (should be skipped)
    temp.child(".cursor").create_dir_all().unwrap();
    temp.child(".cursor/hooks.json")
        .write_str(r#"{
            "version": 1,
            "hooks": {
                "postToolUse": [
                    {"command": ".cursor/hooks/aisync-normalize.sh echo lint", "matcher": "Write"}
                ]
            }
        }"#)
        .unwrap();

    aisync_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success();

    // Only 1 hook group should exist (from Claude Code, not duplicated from Cursor)
    let hooks_toml = std::fs::read_to_string(temp.child(".ai/hooks.toml").path()).unwrap();
    let group_count = hooks_toml.matches("[[PostToolUse]]").count();
    assert_eq!(group_count, 1, "should have exactly 1 hook group, not duplicated from Cursor");
}

// =============================================================================
// Hook translation: Claude Code → Cursor
// =============================================================================

#[test]
fn test_sync_translates_hooks_for_cursor() {
    let temp = setup_project(CC_CURSOR_CONFIG, "# Test\n");

    // Create hooks.toml with canonical hooks
    temp.child(".ai/hooks.toml")
        .write_str(r#"[[PostToolUse]]
matcher = "Edit|Write"

[[PostToolUse.hooks]]
type = "command"
command = "$CLAUDE_PROJECT_DIR/.claude/hooks/lint.sh"
timeout = 30000

[[PreToolUse]]
matcher = "Bash"

[[PreToolUse.hooks]]
type = "command"
command = "echo pre-check"
"#)
        .unwrap();

    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // Cursor hooks.json should exist
    let hooks_json = std::fs::read_to_string(temp.child(".cursor/hooks.json").path()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&hooks_json).unwrap();

    // Version field
    assert_eq!(parsed["version"], 1);

    // Event names should be camelCase
    assert!(parsed["hooks"]["postToolUse"].is_array(), "should have postToolUse");
    assert!(parsed["hooks"]["preToolUse"].is_array(), "should have preToolUse");

    // Matchers should be translated: Edit→Write, Bash→Shell
    let post_hook = &parsed["hooks"]["postToolUse"][0];
    assert_eq!(post_hook["matcher"], "Write", "Edit|Write should become Write (deduplicated)");

    let pre_hook = &parsed["hooks"]["preToolUse"][0];
    assert_eq!(pre_hook["matcher"], "Shell", "Bash should become Shell");

    // Commands should be shim-wrapped with project-relative paths
    let command = post_hook["command"].as_str().unwrap();
    assert!(command.starts_with(".cursor/hooks/aisync-normalize.sh"), "should be shim-wrapped");
    assert!(command.contains(".claude/hooks/lint.sh"), "should contain original script path");
    assert!(!command.contains("$CLAUDE_PROJECT_DIR"), "should strip env var");

    // Timeout should be converted from ms to seconds
    assert_eq!(post_hook["timeout"], 30, "30000ms should become 30s");
}

#[test]
fn test_sync_generates_normalize_shim() {
    let temp = setup_project(CC_CURSOR_CONFIG, "# Test\n");

    temp.child(".ai/hooks.toml")
        .write_str(r#"[[PostToolUse]]

[[PostToolUse.hooks]]
type = "command"
command = "echo test"
"#)
        .unwrap();

    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // Shim should exist
    let shim_path = temp.child(".cursor/hooks/aisync-normalize.sh");
    shim_path.assert(predicate::path::exists());

    // Shim should be executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::metadata(shim_path.path()).unwrap().permissions();
        assert!(perms.mode() & 0o111 != 0, "shim should be executable");
    }

    // Shim content should contain normalization logic
    let content = std::fs::read_to_string(shim_path.path()).unwrap();
    assert!(content.contains("tool_output"), "should handle tool_output normalization");
    assert!(content.contains("tool_result"), "should produce tool_result");
    assert!(content.contains("file_path"), "should handle file_path translation");
}

#[test]
fn test_sync_prompt_hooks_pass_through_without_shim() {
    let temp = setup_project(CC_CURSOR_CONFIG, "# Test\n");

    temp.child(".ai/hooks.toml")
        .write_str(r#"[[PreToolUse]]
matcher = "Write"

[[PreToolUse.hooks]]
type = "prompt"
command = "Check that this write follows coding standards"
"#)
        .unwrap();

    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    let hooks_json = std::fs::read_to_string(temp.child(".cursor/hooks.json").path()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&hooks_json).unwrap();

    let hook = &parsed["hooks"]["preToolUse"][0];
    assert_eq!(hook["type"], "prompt", "should preserve prompt type");
    assert_eq!(hook["prompt"], "Check that this write follows coding standards");
    assert!(hook.get("command").is_none(), "prompt hooks should not have command field");
}

// =============================================================================
// Hook translation: all three tools
// =============================================================================

#[test]
fn test_sync_hooks_for_all_tools() {
    let temp = setup_project(ALL_HOOKS_CONFIG, "# Test\n");

    temp.child(".ai/hooks.toml")
        .write_str(r#"[[PostToolUse]]
matcher = "Edit"

[[PostToolUse.hooks]]
type = "command"
command = "echo lint"
timeout = 10000
"#)
        .unwrap();

    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // Claude Code: settings.json with PascalCase events
    let cc_hooks = std::fs::read_to_string(temp.child(".claude/settings.json").path()).unwrap();
    let cc_parsed: serde_json::Value = serde_json::from_str(&cc_hooks).unwrap();
    assert!(cc_parsed["hooks"]["PostToolUse"].is_array());
    assert_eq!(cc_parsed["hooks"]["PostToolUse"][0]["hooks"][0]["timeout"], 10);

    // Cursor: hooks.json with camelCase events + shim
    let cursor_hooks = std::fs::read_to_string(temp.child(".cursor/hooks.json").path()).unwrap();
    let cursor_parsed: serde_json::Value = serde_json::from_str(&cursor_hooks).unwrap();
    assert!(cursor_parsed["hooks"]["postToolUse"].is_array());
    assert_eq!(cursor_parsed["hooks"]["postToolUse"][0]["matcher"], "Write");

    // OpenCode: JS plugin stub
    let oc_hooks =
        std::fs::read_to_string(temp.child(".opencode/plugins/aisync-hooks.js").path()).unwrap();
    assert!(oc_hooks.contains("tool.execute.after"));
    assert!(oc_hooks.contains("echo lint"));
}

// =============================================================================
// MCP env variable translation
// =============================================================================

#[test]
fn test_sync_mcp_translates_env_vars_for_cursor() {
    let temp = setup_project(CC_CURSOR_CONFIG, "# Test\n");

    // Canonical mcp.toml with ${VAR} format
    temp.child(".ai/mcp.toml")
        .write_str(r#"[servers.my-server]
command = "npx"
args = ["-y", "my-tool"]

[servers.my-server.env]
API_KEY = "${MY_API_KEY}"
HOME = "${HOME_DIR}"
"#)
        .unwrap();

    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // Cursor mcp.json should use ${env:VAR} format
    let cursor_mcp = std::fs::read_to_string(temp.child(".cursor/mcp.json").path()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&cursor_mcp).unwrap();
    assert_eq!(
        parsed["mcpServers"]["my-server"]["env"]["API_KEY"],
        "${env:MY_API_KEY}",
        "canonical ${{MY_API_KEY}} should become Cursor ${{env:MY_API_KEY}}"
    );
    assert_eq!(
        parsed["mcpServers"]["my-server"]["env"]["HOME"],
        "${env:HOME_DIR}"
    );

    // Claude Code mcp.json should keep canonical ${VAR} format
    let cc_mcp = std::fs::read_to_string(temp.child(".claude/.mcp.json").path()).unwrap();
    let cc_parsed: serde_json::Value = serde_json::from_str(&cc_mcp).unwrap();
    assert_eq!(
        cc_parsed["mcpServers"]["my-server"]["env"]["API_KEY"],
        "${MY_API_KEY}",
        "Claude Code should keep canonical ${{VAR}} format"
    );
}

#[test]
fn test_init_imports_cursor_mcp_env_vars() {
    let temp = assert_fs::TempDir::new().unwrap();
    temp.child("CLAUDE.md").write_str("# Test").unwrap();
    temp.child(".cursor").create_dir_all().unwrap();

    // Cursor mcp.json with ${env:VAR} format
    temp.child(".cursor/mcp.json")
        .write_str(r#"{
            "mcpServers": {
                "github": {
                    "command": "npx",
                    "args": ["-y", "server-github"],
                    "env": {"TOKEN": "${env:GITHUB_TOKEN}"}
                }
            }
        }"#)
        .unwrap();

    aisync_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success();

    // Canonical mcp.toml should have ${VAR} format (not ${env:VAR})
    let mcp_toml = std::fs::read_to_string(temp.child(".ai/mcp.toml").path()).unwrap();
    assert!(
        mcp_toml.contains("${GITHUB_TOKEN}"),
        "Cursor ${{env:GITHUB_TOKEN}} should be imported as canonical ${{GITHUB_TOKEN}}"
    );
    assert!(
        !mcp_toml.contains("${env:"),
        "canonical format should not contain ${{env:}}"
    );
}

// =============================================================================
// Cursor-only hook events
// =============================================================================

#[test]
fn test_sync_cursor_only_events_translated() {
    let temp = setup_project(CC_CURSOR_CONFIG, "# Test\n");

    temp.child(".ai/hooks.toml")
        .write_str(r#"[[AfterFileEdit]]
matcher = "Write"

[[AfterFileEdit.hooks]]
type = "command"
command = "echo post-edit"

[[SessionStart]]

[[SessionStart.hooks]]
type = "command"
command = "echo session-start"
"#)
        .unwrap();

    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // Cursor should have these events in camelCase
    let hooks_json = std::fs::read_to_string(temp.child(".cursor/hooks.json").path()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&hooks_json).unwrap();
    assert!(parsed["hooks"]["afterFileEdit"].is_array(), "should translate AfterFileEdit");
    assert!(parsed["hooks"]["sessionStart"].is_array(), "should translate SessionStart");

    // Claude Code settings.json should NOT contain these events
    let cc_hooks = std::fs::read_to_string(temp.child(".claude/settings.json").path()).unwrap();
    let cc_parsed: serde_json::Value = serde_json::from_str(&cc_hooks).unwrap();
    assert!(
        cc_parsed["hooks"].get("AfterFileEdit").is_none(),
        "Claude Code should skip Cursor-only events"
    );
    assert!(
        cc_parsed["hooks"].get("SessionStart").is_none(),
        "Claude Code should skip Cursor-only events"
    );
}

// =============================================================================
// Skills sync
// =============================================================================

#[test]
fn test_sync_skills_to_cursor() {
    let temp = setup_project(CC_CURSOR_CONFIG, "# Test\n");

    // Create canonical skills
    temp.child(".ai/skills/lint-check/SKILL.md")
        .write_str("---\nname: lint-check\ndescription: Run lint checks\n---\n\nLint instructions here.")
        .unwrap();

    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // Cursor should have the skill synced
    let cursor_skill = temp.child(".cursor/skills/aisync-lint-check/SKILL.md");
    cursor_skill.assert(predicate::path::exists());
    let content = std::fs::read_to_string(cursor_skill.path()).unwrap();
    assert!(content.contains("Lint instructions here."));
}

// =============================================================================
// Normalize shim stdin translation
// =============================================================================

#[test]
fn test_normalize_shim_translates_tool_output_to_tool_result() {
    let temp = setup_project(CC_CURSOR_CONFIG, "# Test\n");

    temp.child(".ai/hooks.toml")
        .write_str("[[PostToolUse]]\n\n[[PostToolUse.hooks]]\ntype = \"command\"\ncommand = \"echo ok\"\n")
        .unwrap();

    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // Test the shim with Cursor-format stdin
    let shim_path = temp.child(".cursor/hooks/aisync-normalize.sh");

    let output = std::process::Command::new("bash")
        .arg(shim_path.path())
        .arg("cat")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .current_dir(temp.path())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(br#"{"tool_name":"Shell","tool_input":{"command":"git status"},"tool_output":"{\"exitCode\":0,\"stdout\":\"clean\"}"}"#).unwrap();
            drop(child.stdin.take());
            child.wait_with_output()
        })
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();

    // tool_output should be parsed into tool_result
    assert!(parsed.get("tool_output").is_none(), "tool_output should be removed");
    assert_eq!(parsed["tool_result"]["exitCode"], 0);
    assert_eq!(parsed["tool_result"]["stdout"], "clean");
}

#[test]
fn test_normalize_shim_copies_path_to_file_path() {
    let temp = setup_project(CC_CURSOR_CONFIG, "# Test\n");

    temp.child(".ai/hooks.toml")
        .write_str("[[PostToolUse]]\n\n[[PostToolUse.hooks]]\ntype = \"command\"\ncommand = \"echo ok\"\n")
        .unwrap();

    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    let shim_path = temp.child(".cursor/hooks/aisync-normalize.sh");

    let output = std::process::Command::new("bash")
        .arg(shim_path.path())
        .arg("cat")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .current_dir(temp.path())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(br#"{"tool_name":"Write","tool_input":{"path":"/tmp/test.py"},"tool_output":"ok"}"#).unwrap();
            drop(child.stdin.take());
            child.wait_with_output()
        })
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();

    // path should be copied to file_path
    assert_eq!(parsed["tool_input"]["file_path"], "/tmp/test.py");
    assert_eq!(parsed["tool_input"]["path"], "/tmp/test.py");
}
