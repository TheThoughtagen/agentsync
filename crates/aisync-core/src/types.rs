use serde::{Deserialize, Serialize};

/// Identifies which AI coding tool is being managed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolKind {
    ClaudeCode,
    Cursor,
    OpenCode,
}

/// Confidence level for tool detection results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_kind_variants_exist() {
        let tools = [ToolKind::ClaudeCode, ToolKind::Cursor, ToolKind::OpenCode];
        assert_eq!(tools.len(), 3);
    }

    #[test]
    fn test_tool_kind_equality() {
        assert_eq!(ToolKind::ClaudeCode, ToolKind::ClaudeCode);
        assert_ne!(ToolKind::ClaudeCode, ToolKind::Cursor);
    }

    #[test]
    fn test_tool_kind_clone_copy() {
        let t = ToolKind::Cursor;
        let t2 = t; // Copy
        let t3 = t.clone(); // Clone
        assert_eq!(t, t2);
        assert_eq!(t, t3);
    }

    #[test]
    fn test_tool_kind_debug() {
        let debug = format!("{:?}", ToolKind::OpenCode);
        assert_eq!(debug, "OpenCode");
    }

    #[test]
    fn test_confidence_variants_exist() {
        let levels = [Confidence::High, Confidence::Medium];
        assert_eq!(levels.len(), 2);
    }

    #[test]
    fn test_confidence_equality() {
        assert_eq!(Confidence::High, Confidence::High);
        assert_ne!(Confidence::High, Confidence::Medium);
    }
}
