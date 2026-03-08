use crate::adapter::ToolAdapter;
use crate::types::ToolKind;

/// Processes conditional sections in instruction content.
///
/// Supports markers like `<!-- aisync:claude-only -->` / `<!-- /aisync:claude-only -->`
/// to include or exclude sections based on the target tool.
pub struct ConditionalProcessor;

impl ConditionalProcessor {
    /// Process content for a specific tool, including matching sections
    /// and stripping non-matching sections. Marker lines are always removed.
    pub fn process(content: &str, tool: ToolKind) -> String {
        let matching_tags = Self::tool_tag_names(&tool);
        let mut output = Vec::new();
        let mut skip_depth: usize = 0;

        for line in content.lines() {
            if let Some(tag) = Self::parse_open_tag(line) {
                if skip_depth > 0 {
                    // Already inside a skipped section -- increase depth
                    skip_depth += 1;
                } else if matching_tags.contains(&tag.as_str()) {
                    // Matching tag: include content, strip marker
                } else {
                    // Non-matching tag: start skipping
                    skip_depth = 1;
                }
                continue; // Always strip marker lines
            }

            if let Some(_tag) = Self::parse_close_tag(line) {
                skip_depth = skip_depth.saturating_sub(1);
                // Always strip close marker lines
                continue;
            }

            if skip_depth == 0 {
                output.push(line);
            }
        }

        // Join with newlines, preserving trailing newline if input had one
        let mut result = output.join("\n");
        if content.ends_with('\n') {
            result.push('\n');
        }
        result
    }

    /// Returns the tag names that match a given tool, using adapter metadata.
    /// Custom tools with no adapter return empty tags.
    fn tool_tag_names(tool: &ToolKind) -> Vec<&'static str> {
        use crate::adapter::{ClaudeCodeAdapter, CodexAdapter, CursorAdapter, OpenCodeAdapter, WindsurfAdapter};
        match tool {
            ToolKind::ClaudeCode => ClaudeCodeAdapter.conditional_tags().to_vec(),
            ToolKind::Cursor => CursorAdapter.conditional_tags().to_vec(),
            ToolKind::OpenCode => OpenCodeAdapter.conditional_tags().to_vec(),
            ToolKind::Windsurf => WindsurfAdapter.conditional_tags().to_vec(),
            ToolKind::Codex => CodexAdapter.conditional_tags().to_vec(),
            ToolKind::Custom(_) => vec![],
        }
    }

    /// Parse an opening conditional tag from a line.
    /// Returns the tag name if the line matches `<!-- aisync:TAG -->`.
    fn parse_open_tag(line: &str) -> Option<String> {
        let trimmed = line.trim();
        if trimmed.starts_with("<!-- aisync:")
            && trimmed.ends_with(" -->")
            && !trimmed.starts_with("<!-- /aisync:")
        {
            let inner = &trimmed[12..trimmed.len() - 4]; // after "<!-- aisync:" and before " -->"
            if !inner.is_empty() && !inner.contains(' ') {
                return Some(inner.to_string());
            }
        }
        None
    }

    /// Parse a closing conditional tag from a line.
    /// Returns the tag name if the line matches `<!-- /aisync:TAG -->`.
    fn parse_close_tag(line: &str) -> Option<String> {
        let trimmed = line.trim();
        if trimmed.starts_with("<!-- /aisync:") && trimmed.ends_with(" -->") {
            let inner = &trimmed[13..trimmed.len() - 4]; // after "<!-- /aisync:" and before " -->"
            if !inner.is_empty() && !inner.contains(' ') {
                return Some(inner.to_string());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_only_section_kept_for_claude() {
        let input = "common\n<!-- aisync:claude-only -->\nclaude stuff\n<!-- /aisync:claude-only -->\nmore common\n";
        let result = ConditionalProcessor::process(input, ToolKind::ClaudeCode);
        assert!(result.contains("claude stuff"));
        assert!(result.contains("common"));
        assert!(result.contains("more common"));
    }

    #[test]
    fn test_claude_only_section_stripped_for_cursor() {
        let input = "common\n<!-- aisync:claude-only -->\nclaude stuff\n<!-- /aisync:claude-only -->\nmore common\n";
        let result = ConditionalProcessor::process(input, ToolKind::Cursor);
        assert!(!result.contains("claude stuff"));
        assert!(result.contains("common"));
        assert!(result.contains("more common"));
    }

    #[test]
    fn test_claude_only_section_stripped_for_opencode() {
        let input = "common\n<!-- aisync:claude-only -->\nclaude stuff\n<!-- /aisync:claude-only -->\nmore common\n";
        let result = ConditionalProcessor::process(input, ToolKind::OpenCode);
        assert!(!result.contains("claude stuff"));
        assert!(result.contains("common"));
    }

    #[test]
    fn test_cursor_only_section_kept_for_cursor() {
        let input = "common\n<!-- aisync:cursor-only -->\ncursor stuff\n<!-- /aisync:cursor-only -->\nmore common\n";
        let result = ConditionalProcessor::process(input, ToolKind::Cursor);
        assert!(result.contains("cursor stuff"));
        assert!(result.contains("common"));
    }

    #[test]
    fn test_cursor_only_section_stripped_for_claude() {
        let input = "common\n<!-- aisync:cursor-only -->\ncursor stuff\n<!-- /aisync:cursor-only -->\nmore common\n";
        let result = ConditionalProcessor::process(input, ToolKind::ClaudeCode);
        assert!(!result.contains("cursor stuff"));
        assert!(result.contains("common"));
    }

    #[test]
    fn test_opencode_only_section_kept_for_opencode() {
        let input = "common\n<!-- aisync:opencode-only -->\nopencode stuff\n<!-- /aisync:opencode-only -->\nmore common\n";
        let result = ConditionalProcessor::process(input, ToolKind::OpenCode);
        assert!(result.contains("opencode stuff"));
    }

    #[test]
    fn test_opencode_only_section_stripped_for_others() {
        let input = "common\n<!-- aisync:opencode-only -->\nopencode stuff\n<!-- /aisync:opencode-only -->\nmore common\n";
        let result_claude = ConditionalProcessor::process(input, ToolKind::ClaudeCode);
        let result_cursor = ConditionalProcessor::process(input, ToolKind::Cursor);
        assert!(!result_claude.contains("opencode stuff"));
        assert!(!result_cursor.contains("opencode stuff"));
    }

    #[test]
    fn test_common_content_preserved_for_all_tools() {
        let input = "# Title\n\nThis is common content.\n\n## Usage\n\nAll tools see this.\n";
        for tool in [ToolKind::ClaudeCode, ToolKind::Cursor, ToolKind::OpenCode] {
            let result = ConditionalProcessor::process(input, tool.clone());
            assert_eq!(
                result, input,
                "common content should be unchanged for {:?}",
                tool
            );
        }
    }

    #[test]
    fn test_multiple_conditional_sections() {
        let input = "header\n<!-- aisync:claude-only -->\nclaude\n<!-- /aisync:claude-only -->\nmiddle\n<!-- aisync:cursor-only -->\ncursor\n<!-- /aisync:cursor-only -->\nfooter\n";
        let result = ConditionalProcessor::process(input, ToolKind::ClaudeCode);
        assert!(result.contains("header"));
        assert!(result.contains("claude"));
        assert!(result.contains("middle"));
        assert!(!result.contains("cursor"));
        assert!(result.contains("footer"));
    }

    #[test]
    fn test_no_conditional_sections_returned_unchanged() {
        let input = "just regular content\nwith multiple lines\n";
        let result = ConditionalProcessor::process(input, ToolKind::ClaudeCode);
        assert_eq!(result, input);
    }

    #[test]
    fn test_nested_conditionals_inner_tags_treated_as_text_when_skipped() {
        // When inside a skipped section, inner tags are skipped too (not treated as matching)
        let input = "start\n<!-- aisync:cursor-only -->\nouter cursor\n<!-- aisync:claude-only -->\nnested claude\n<!-- /aisync:claude-only -->\n<!-- /aisync:cursor-only -->\nend\n";
        let result = ConditionalProcessor::process(input, ToolKind::ClaudeCode);
        // Claude skips cursor-only, so everything inside (including nested claude-only) is skipped
        assert!(!result.contains("outer cursor"));
        assert!(!result.contains("nested claude"));
        assert!(result.contains("start"));
        assert!(result.contains("end"));
    }

    #[test]
    fn test_marker_lines_always_stripped() {
        let input =
            "before\n<!-- aisync:claude-only -->\ncontent\n<!-- /aisync:claude-only -->\nafter\n";
        let result = ConditionalProcessor::process(input, ToolKind::ClaudeCode);
        assert!(!result.contains("<!-- aisync:"));
        assert!(!result.contains("<!-- /aisync:"));
        assert!(result.contains("content"));
    }

    #[test]
    fn test_claude_code_only_alias() {
        let input = "before\n<!-- aisync:claude-code-only -->\nclaude code content\n<!-- /aisync:claude-code-only -->\nafter\n";
        let result = ConditionalProcessor::process(input, ToolKind::ClaudeCode);
        assert!(result.contains("claude code content"));
        let result_cursor = ConditionalProcessor::process(input, ToolKind::Cursor);
        assert!(!result_cursor.contains("claude code content"));
    }
}
