use assert_cmd::Command;
use assert_fs::TempDir;
use assert_fs::prelude::*;

/// Standard multi-tool aisync.toml configuration for tests.
/// Uses copy strategy on Windows since symlinks require elevated privileges.
#[cfg(windows)]
pub const STANDARD_CONFIG: &str = r#"schema_version = 1
[defaults]
sync_strategy = "copy"
[tools.claude-code]
enabled = true
[tools.opencode]
enabled = true
[tools.cursor]
enabled = true
"#;

#[cfg(not(windows))]
pub const STANDARD_CONFIG: &str = r#"schema_version = 1
[tools.claude-code]
enabled = true
[tools.opencode]
enabled = true
[tools.cursor]
enabled = true
"#;

/// Create a temporary project directory with aisync.toml and .ai/instructions.md.
pub fn setup_project(toml_content: &str, instructions: &str) -> TempDir {
    let temp = TempDir::new().unwrap();

    // Write aisync.toml
    temp.child("aisync.toml").write_str(toml_content).unwrap();

    // Create .ai/ directory and instructions.md
    temp.child(".ai").create_dir_all().unwrap();
    temp.child(".ai/instructions.md")
        .write_str(instructions)
        .unwrap();

    // Create other expected .ai/ subdirs
    temp.child(".ai/memory").create_dir_all().unwrap();
    temp.child(".ai/hooks").create_dir_all().unwrap();
    temp.child(".ai/commands").create_dir_all().unwrap();

    temp
}

/// Create an `aisync` command pointed at the cargo binary.
pub fn aisync_cmd() -> Command {
    Command::cargo_bin("aisync").unwrap()
}
