pub mod claude_code;
pub mod codex;
pub mod cursor;
pub mod opencode;
pub mod windsurf;

use std::path::PathBuf;

use crate::adapter::AdapterError;
use crate::types::{RuleFile, SyncAction};

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
