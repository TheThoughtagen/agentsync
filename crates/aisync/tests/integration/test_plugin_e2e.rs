use assert_fs::prelude::*;
use predicates::prelude::*;

use crate::helpers::{aisync_cmd, setup_project, STANDARD_CONFIG};

/// Config with Claude Code and Cursor enabled (no OpenCode to avoid symlink conflicts).
const CC_CURSOR_CONFIG: &str = r#"schema_version = 1
[tools.claude-code]
enabled = true
[tools.cursor]
enabled = true
"#;

// =============================================================================
// Helper: create a mock Claude Code plugin in a given directory
// =============================================================================

fn create_mock_claude_plugin(dir: &assert_fs::TempDir) {
    // .claude-plugin/plugin.json
    dir.child(".claude-plugin/plugin.json")
        .write_str(
            r#"{
    "name": "test-plugin",
    "version": "1.0.0",
    "description": "A test plugin for E2E testing"
}"#,
        )
        .unwrap();

    // commands/test-cmd.md
    dir.child("commands/test-cmd.md")
        .write_str(
            "---\nname: test-cmd\ndescription: A test command\n---\n\nRun this test command body.\n",
        )
        .unwrap();

    // skills/test-skill/SKILL.md
    dir.child("skills/test-skill/SKILL.md")
        .write_str(
            "---\nname: test-skill\ndescription: A test skill\n---\n\nSkill instructions body.\n",
        )
        .unwrap();

    // hooks/hooks.json
    dir.child("hooks/hooks.json")
        .write_str(
            r#"{
    "hooks": {
        "PreToolUse": [
            {
                "matcher": "Bash",
                "hooks": [
                    {"type": "command", "command": "echo test", "timeout": 10}
                ]
            }
        ]
    }
}"#,
        )
        .unwrap();
}

// =============================================================================
// 1. Full import + export round-trip
// =============================================================================

#[test]
fn test_plugin_import_claude_code_and_export_round_trip() {
    // Create the source plugin in a separate temp dir
    let source_dir = assert_fs::TempDir::new().unwrap();
    create_mock_claude_plugin(&source_dir);

    // Set up an aisync project
    let project = setup_project(STANDARD_CONFIG, "# Test\n");

    // Import the plugin
    aisync_cmd()
        .args([
            "plugin",
            "import",
            source_dir.path().to_str().unwrap(),
            "--from",
            "claude-code",
            "--name",
            "test-plugin",
        ])
        .current_dir(project.path())
        .assert()
        .success();

    // Verify plugin.toml exists with correct metadata
    let plugin_toml_path = project.child(".ai/plugins/test-plugin/plugin.toml");
    plugin_toml_path.assert(predicate::path::exists());
    let plugin_toml = std::fs::read_to_string(plugin_toml_path.path()).unwrap();
    assert!(
        plugin_toml.contains("name = \"test-plugin\""),
        "plugin.toml should contain name"
    );
    assert!(
        plugin_toml.contains("version = \"1.0.0\""),
        "plugin.toml should contain version"
    );
    assert!(
        plugin_toml.contains("source_tool = \"claude-code\""),
        "plugin.toml should contain source_tool"
    );

    // Verify hooks.toml exists
    let hooks_toml_path = project.child(".ai/plugins/test-plugin/hooks.toml");
    hooks_toml_path.assert(predicate::path::exists());
    let hooks_toml = std::fs::read_to_string(hooks_toml_path.path()).unwrap();
    assert!(
        hooks_toml.contains("PreToolUse"),
        "hooks.toml should contain PreToolUse event"
    );
    assert!(
        hooks_toml.contains("echo test"),
        "hooks.toml should contain hook command"
    );

    // Verify commands were copied
    let cmd_path = project.child(".ai/plugins/test-plugin/commands/test-cmd.md");
    cmd_path.assert(predicate::path::exists());

    // Verify skills were copied
    let skill_path = project.child(".ai/plugins/test-plugin/skills/test-skill/SKILL.md");
    skill_path.assert(predicate::path::exists());

    // Now export back to Claude Code
    aisync_cmd()
        .args(["plugin", "export", "test-plugin", "--to", "claude-code"])
        .current_dir(project.path())
        .assert()
        .success();

    // Verify exported plugin structure
    let exported_plugin_json = project.child("plugins/test-plugin/.claude-plugin/plugin.json");
    exported_plugin_json.assert(predicate::path::exists());
    let plugin_json = std::fs::read_to_string(exported_plugin_json.path()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&plugin_json).unwrap();
    assert_eq!(parsed["name"], "test-plugin");
    assert_eq!(parsed["version"], "1.0.0");
    assert_eq!(parsed["description"], "A test plugin for E2E testing");

    // Verify exported commands
    let exported_cmd = project.child("plugins/test-plugin/commands/test-cmd.md");
    exported_cmd.assert(predicate::path::exists());
    let cmd_content = std::fs::read_to_string(exported_cmd.path()).unwrap();
    assert!(
        cmd_content.contains("Run this test command body."),
        "exported command should contain body"
    );

    // Verify exported skills
    let exported_skill = project.child("plugins/test-plugin/skills/test-skill/SKILL.md");
    exported_skill.assert(predicate::path::exists());
    let skill_content = std::fs::read_to_string(exported_skill.path()).unwrap();
    assert!(
        skill_content.contains("Skill instructions body."),
        "exported skill should contain body"
    );

    // Verify exported hooks
    let exported_hooks = project.child("plugins/test-plugin/hooks/hooks.json");
    exported_hooks.assert(predicate::path::exists());
    let hooks_json = std::fs::read_to_string(exported_hooks.path()).unwrap();
    let hooks_parsed: serde_json::Value = serde_json::from_str(&hooks_json).unwrap();
    assert!(
        hooks_parsed["hooks"]["PreToolUse"].is_array(),
        "exported hooks should contain PreToolUse"
    );
}

// =============================================================================
// 2. Import from Claude Code and export to Cursor (cross-tool)
// =============================================================================

#[test]
fn test_plugin_import_and_export_to_cursor() {
    let source_dir = assert_fs::TempDir::new().unwrap();
    create_mock_claude_plugin(&source_dir);

    let project = setup_project(STANDARD_CONFIG, "# Test\n");

    // Import
    aisync_cmd()
        .args([
            "plugin",
            "import",
            source_dir.path().to_str().unwrap(),
            "--from",
            "claude-code",
            "--name",
            "cross-test",
        ])
        .current_dir(project.path())
        .assert()
        .success();

    // Export to Cursor
    let output = aisync_cmd()
        .args(["plugin", "export", "cross-test", "--to", "cursor"])
        .current_dir(project.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "export to cursor should succeed");
    let stdout = String::from_utf8(output.stdout).unwrap();

    // No instructions.md was in the source plugin, so no .cursor/rules/cross-test.mdc
    let mdc_path = project.child(".cursor/rules/cross-test.mdc");
    assert!(
        !mdc_path.path().exists(),
        ".cursor/rules/cross-test.mdc should NOT exist since source had no instructions.md"
    );

    // Commands and skills should be reported as skipped
    let combined = format!("{}{}", stdout, String::from_utf8(output.stderr).unwrap_or_default());
    // The export output should mention skipped components
    assert!(
        combined.contains("Commands") || combined.contains("commands") || combined.contains("Skills") || combined.contains("skills"),
        "export output should mention skipped components: got: {}",
        combined
    );
}

// =============================================================================
// 3. Sync exports canonical plugins
// =============================================================================

#[test]
fn test_plugin_sync_exports_canonical_plugins() {
    let project = setup_project(CC_CURSOR_CONFIG, "# Test\n");

    // Create a canonical plugin with instructions
    project
        .child(".ai/plugins/my-plugin/plugin.toml")
        .write_str(
            r#"[metadata]
name = "my-plugin"
version = "0.1.0"
description = "My test plugin"
source_tool = "claude-code"

[components]
has_instructions = true
"#,
        )
        .unwrap();

    project
        .child(".ai/plugins/my-plugin/instructions.md")
        .write_str("Follow these plugin-specific instructions for coding.\n")
        .unwrap();

    // First sync
    aisync_cmd()
        .arg("sync")
        .current_dir(project.path())
        .assert()
        .success();

    // Verify Claude Code export: plugins/my-plugin/.claude-plugin/plugin.json
    let cc_plugin_json = project.child("plugins/my-plugin/.claude-plugin/plugin.json");
    cc_plugin_json.assert(predicate::path::exists());
    let cc_json = std::fs::read_to_string(cc_plugin_json.path()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&cc_json).unwrap();
    assert_eq!(parsed["name"], "my-plugin");

    // Verify Cursor export: .cursor/rules/my-plugin.mdc
    let cursor_mdc = project.child(".cursor/rules/my-plugin.mdc");
    cursor_mdc.assert(predicate::path::exists());
    let mdc_content = std::fs::read_to_string(cursor_mdc.path()).unwrap();
    assert!(
        mdc_content.contains("Follow these plugin-specific instructions"),
        "Cursor mdc should contain instructions content"
    );

    // Second sync should be idempotent (no errors)
    aisync_cmd()
        .arg("sync")
        .current_dir(project.path())
        .assert()
        .success();
}

// =============================================================================
// 4. Plugin list
// =============================================================================

#[test]
fn test_plugin_list() {
    let project = setup_project(STANDARD_CONFIG, "# Test\n");

    // Create two canonical plugins
    project
        .child(".ai/plugins/alpha-plugin/plugin.toml")
        .write_str(
            r#"[metadata]
name = "alpha-plugin"
version = "1.0.0"
description = "Alpha plugin"

[components]
"#,
        )
        .unwrap();

    project
        .child(".ai/plugins/beta-plugin/plugin.toml")
        .write_str(
            r#"[metadata]
name = "beta-plugin"
version = "2.0.0"
description = "Beta plugin"

[components]
"#,
        )
        .unwrap();

    let output = aisync_cmd()
        .args(["plugin", "list"])
        .current_dir(project.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "plugin list should succeed");
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(
        stdout.contains("alpha-plugin"),
        "output should contain alpha-plugin, got: {}",
        stdout
    );
    assert!(
        stdout.contains("beta-plugin"),
        "output should contain beta-plugin, got: {}",
        stdout
    );
}

// =============================================================================
// 5. Auto-detect source tool on import
// =============================================================================

#[test]
fn test_plugin_import_auto_detects_tool() {
    let source_dir = assert_fs::TempDir::new().unwrap();
    create_mock_claude_plugin(&source_dir);

    let project = setup_project(STANDARD_CONFIG, "# Test\n");

    // Import WITHOUT --from flag — should auto-detect claude-code
    aisync_cmd()
        .args([
            "plugin",
            "import",
            source_dir.path().to_str().unwrap(),
            "--name",
            "auto-detect-test",
        ])
        .current_dir(project.path())
        .assert()
        .success();

    // Verify plugin.toml has source_tool = "claude-code"
    let plugin_toml_path = project.child(".ai/plugins/auto-detect-test/plugin.toml");
    plugin_toml_path.assert(predicate::path::exists());
    let plugin_toml = std::fs::read_to_string(plugin_toml_path.path()).unwrap();
    assert!(
        plugin_toml.contains("source_tool = \"claude-code\""),
        "source_tool should be auto-detected as claude-code, got: {}",
        plugin_toml
    );
}
