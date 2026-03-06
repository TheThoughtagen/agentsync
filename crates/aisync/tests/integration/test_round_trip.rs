use assert_fs::prelude::*;

use crate::helpers::{STANDARD_CONFIG, aisync_cmd, setup_project};

#[test]
fn test_round_trip_claude_code() {
    let instructions = "# Project Instructions\n\nBuild great software.\n";
    let temp = setup_project(STANDARD_CONFIG, instructions);

    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // CLAUDE.md is a symlink to .ai/instructions.md, so content should be identical
    let claude_content = std::fs::read_to_string(temp.child("CLAUDE.md").path()).unwrap();
    assert_eq!(claude_content, instructions);
}

#[test]
fn test_round_trip_opencode() {
    let instructions = "# Project Instructions\n\nBuild great software.\n";
    let temp = setup_project(STANDARD_CONFIG, instructions);

    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // AGENTS.md is a symlink to .ai/instructions.md, so content should be identical
    let agents_content = std::fs::read_to_string(temp.child("AGENTS.md").path()).unwrap();
    assert_eq!(agents_content, instructions);
}

#[test]
fn test_round_trip_cursor() {
    let instructions = "# Project Instructions\n\nBuild great software.\n";
    let temp = setup_project(STANDARD_CONFIG, instructions);

    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // .cursor/rules/project.mdc is a generated file with YAML frontmatter
    let mdc_content =
        std::fs::read_to_string(temp.child(".cursor/rules/project.mdc").path()).unwrap();

    // Strip YAML frontmatter (between --- delimiters) to get the body
    let body = strip_mdc_frontmatter(&mdc_content);
    assert_eq!(body, instructions);
}

#[test]
fn test_round_trip_with_conditionals() {
    let instructions = "# Shared\n\n\
<!-- aisync:claude-only -->\n\
Claude-specific content here.\n\
<!-- /aisync:claude-only -->\n\
\n\
<!-- aisync:cursor-only -->\n\
Cursor-specific content here.\n\
<!-- /aisync:cursor-only -->\n\
\n\
Common footer.\n";

    let temp = setup_project(STANDARD_CONFIG, instructions);

    aisync_cmd()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // Claude should have claude-specific content but not cursor-specific
    let claude_content = std::fs::read_to_string(temp.child("CLAUDE.md").path()).unwrap();
    assert!(
        claude_content.contains("Claude-specific content here."),
        "Claude should see claude-only content"
    );
    assert!(
        !claude_content.contains("Cursor-specific content here."),
        "Claude should not see cursor-only content"
    );
    assert!(
        claude_content.contains("Common footer."),
        "Claude should see common content"
    );

    // Cursor should have cursor-specific content but not claude-specific
    let mdc_content =
        std::fs::read_to_string(temp.child(".cursor/rules/project.mdc").path()).unwrap();
    let cursor_body = strip_mdc_frontmatter(&mdc_content);
    assert!(
        cursor_body.contains("Cursor-specific content here."),
        "Cursor should see cursor-only content"
    );
    assert!(
        !cursor_body.contains("Claude-specific content here."),
        "Cursor should not see claude-only content"
    );
    assert!(
        cursor_body.contains("Common footer."),
        "Cursor should see common content"
    );

    // OpenCode should see neither claude-only nor cursor-only
    let opencode_content = std::fs::read_to_string(temp.child("AGENTS.md").path()).unwrap();
    assert!(
        !opencode_content.contains("Claude-specific content here."),
        "OpenCode should not see claude-only content"
    );
    assert!(
        !opencode_content.contains("Cursor-specific content here."),
        "OpenCode should not see cursor-only content"
    );
    assert!(
        opencode_content.contains("Common footer."),
        "OpenCode should see common content"
    );
}

/// Strip MDC YAML frontmatter (between `---` delimiters) and the blank line after it.
fn strip_mdc_frontmatter(content: &str) -> &str {
    // MDC format: "---\n...frontmatter...\n---\n\nbody"
    if let Some(rest) = content.strip_prefix("---\n") {
        if let Some(after_fm) = rest.find("---\n") {
            let body_start = after_fm + 4; // skip "---\n"
            let remaining = &rest[body_start..];
            // Strip the blank line between frontmatter and body
            remaining.strip_prefix('\n').unwrap_or(remaining)
        } else {
            content
        }
    } else {
        content
    }
}
