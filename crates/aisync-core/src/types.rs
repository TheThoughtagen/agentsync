use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

/// Event emitted by the watch engine for logging/display.
#[derive(Debug, Clone)]
pub enum WatchEvent {
    ForwardSync {
        changed_path: PathBuf,
    },
    ReverseSync {
        tool: ToolKind,
        source_path: PathBuf,
    },
    Error {
        message: String,
    },
}

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Compute a hex-encoded SHA-256 hash of content bytes.
pub fn content_hash(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    hex::encode(hasher.finalize())
}

/// Identifies which AI coding tool is being managed.
///
/// Known tools have named variants; arbitrary tools use `Custom(String)`.
/// Serializes as lowercase hyphenated strings (e.g., "claude-code", "cursor").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ToolKind {
    ClaudeCode,
    Cursor,
    OpenCode,
    Windsurf,
    Codex,
    Custom(String),
}

impl ToolKind {
    /// Returns the canonical string representation of this tool kind.
    pub fn as_str(&self) -> &str {
        match self {
            ToolKind::ClaudeCode => "claude-code",
            ToolKind::Cursor => "cursor",
            ToolKind::OpenCode => "opencode",
            ToolKind::Windsurf => "windsurf",
            ToolKind::Codex => "codex",
            ToolKind::Custom(s) => s.as_str(),
        }
    }

    /// Returns a human-readable display name for this tool kind.
    ///
    /// For built-in tools, returns the conventional display name.
    /// For custom tools, returns the custom name as-is.
    pub fn display_name(&self) -> &str {
        match self {
            ToolKind::ClaudeCode => "Claude Code",
            ToolKind::Cursor => "Cursor",
            ToolKind::OpenCode => "OpenCode",
            ToolKind::Windsurf => "Windsurf",
            ToolKind::Codex => "Codex",
            ToolKind::Custom(s) => s.as_str(),
        }
    }
}

impl fmt::Display for ToolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ToolKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ToolKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "claude-code" => ToolKind::ClaudeCode,
            "cursor" => ToolKind::Cursor,
            "opencode" => ToolKind::OpenCode,
            "windsurf" => ToolKind::Windsurf,
            "codex" => ToolKind::Codex,
            _ => ToolKind::Custom(s),
        })
    }
}

/// Confidence level for tool detection results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
}

/// Configuration for hooks, keyed by event name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksConfig {
    #[serde(flatten)]
    pub events: BTreeMap<String, Vec<HookGroup>>,
}

/// A group of hooks that share an optional file matcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookGroup {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    pub hooks: Vec<HookHandler>,
}

/// A single hook handler with a type, command, and optional timeout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookHandler {
    #[serde(rename = "type")]
    pub hook_type: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

/// Result of translating hooks for a specific tool.
#[derive(Debug, Clone, Serialize)]
pub enum HookTranslation {
    Supported {
        tool: ToolKind,
        content: String,
        format: String,
    },
    Unsupported {
        tool: ToolKind,
        reason: String,
    },
}

/// Result of diffing a single tool's native file against canonical content.
#[derive(Debug, Clone)]
pub struct ToolDiff {
    pub tool: ToolKind,
    pub has_changes: bool,
    pub unified_diff: String,
    pub tool_file: String,
}

/// A planned sync action that can be displayed (dry-run) or executed.
#[derive(Debug, Clone, Serialize)]
pub enum SyncAction {
    CreateSymlink {
        link: PathBuf,
        target: PathBuf,
    },
    RemoveAndRelink {
        link: PathBuf,
        target: PathBuf,
    },
    GenerateMdc {
        output: PathBuf,
        content: String,
    },
    UpdateGitignore {
        path: PathBuf,
        entries: Vec<String>,
    },
    CreateDirectory {
        path: PathBuf,
    },
    CreateFile {
        path: PathBuf,
        content: String,
    },
    RemoveFile {
        path: PathBuf,
    },
    SkipExistingFile {
        path: PathBuf,
        reason: String,
    },
    // Memory actions
    CreateMemorySymlink {
        link: PathBuf,
        target: PathBuf,
    },
    UpdateMemoryReferences {
        path: PathBuf,
        references: Vec<String>,
        marker_start: String,
        marker_end: String,
    },
    // Hook actions
    WriteHookTranslation {
        path: PathBuf,
        content: String,
        tool: ToolKind,
    },
    WarnUnsupportedHooks {
        tool: ToolKind,
        reason: String,
    },
}

impl fmt::Display for SyncAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncAction::CreateSymlink { link, target } => {
                write!(
                    f,
                    "Would create symlink: {} -> {}",
                    link.display(),
                    target.display()
                )
            }
            SyncAction::RemoveAndRelink { link, target } => {
                write!(
                    f,
                    "Would remove and relink: {} -> {}",
                    link.display(),
                    target.display()
                )
            }
            SyncAction::GenerateMdc { output, .. } => {
                write!(f, "Would generate MDC file: {}", output.display())
            }
            SyncAction::UpdateGitignore { path, entries } => {
                write!(
                    f,
                    "Would update .gitignore at {} with {} entries",
                    path.display(),
                    entries.len()
                )
            }
            SyncAction::CreateDirectory { path } => {
                write!(f, "Would create directory: {}", path.display())
            }
            SyncAction::CreateFile { path, .. } => {
                write!(f, "Would create file: {}", path.display())
            }
            SyncAction::RemoveFile { path } => {
                write!(f, "Would remove file: {}", path.display())
            }
            SyncAction::SkipExistingFile { path, reason } => {
                write!(f, "Would skip {}: {}", path.display(), reason)
            }
            SyncAction::CreateMemorySymlink { link, target } => {
                write!(
                    f,
                    "Would create memory symlink: {} -> {}",
                    link.display(),
                    target.display()
                )
            }
            SyncAction::UpdateMemoryReferences {
                path, references, ..
            } => {
                write!(
                    f,
                    "Would update memory references in {} with {} entries",
                    path.display(),
                    references.len()
                )
            }
            SyncAction::WriteHookTranslation { path, tool, .. } => {
                write!(
                    f,
                    "Would write hook translation for {:?} to {}",
                    tool,
                    path.display()
                )
            }
            SyncAction::WarnUnsupportedHooks { tool, reason } => {
                write!(f, "Warning: hooks unsupported for {:?}: {}", tool, reason)
            }
        }
    }
}

/// Result of syncing a single tool.
#[derive(Debug, Clone, Serialize)]
pub struct ToolSyncResult {
    pub tool: ToolKind,
    pub actions: Vec<SyncAction>,
    pub error: Option<String>,
}

/// Overall sync report collecting results from all tools.
#[derive(Debug, Clone, Serialize)]
pub struct SyncReport {
    pub results: Vec<ToolSyncResult>,
}

impl SyncReport {
    pub fn has_errors(&self) -> bool {
        self.results.iter().any(|r| r.error.is_some())
    }

    pub fn exit_code(&self) -> i32 {
        if self.has_errors() { 1 } else { 0 }
    }
}

/// Drift state for a single tool's sync status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum DriftState {
    InSync,
    Drifted { reason: String },
    Missing,
    DanglingSymlink,
    NotConfigured,
}

/// Status of a single tool's sync state.
#[derive(Debug, Clone, Serialize)]
pub struct ToolSyncStatus {
    pub tool: ToolKind,
    pub strategy: crate::config::SyncStrategy,
    pub drift: DriftState,
    pub details: Option<String>,
}

/// Overall status report.
#[derive(Debug, Clone, Serialize)]
pub struct StatusReport {
    pub tools: Vec<ToolSyncStatus>,
    pub memory: Option<MemoryStatusReport>,
    pub hooks: Option<HookStatusReport>,
}

impl StatusReport {
    pub fn all_in_sync(&self) -> bool {
        self.tools
            .iter()
            .all(|t| t.drift == DriftState::InSync || t.drift == DriftState::NotConfigured)
    }
}

/// Memory sync status across tools.
#[derive(Debug, Clone, Serialize)]
pub struct MemoryStatusReport {
    pub file_count: usize,
    pub files: Vec<String>,
    pub per_tool: Vec<ToolMemoryStatus>,
}

/// Memory sync status for a single tool.
#[derive(Debug, Clone, Serialize)]
pub struct ToolMemoryStatus {
    pub tool: ToolKind,
    pub synced: bool,
    pub details: Option<String>,
}

/// Hook translation status across tools.
#[derive(Debug, Clone, Serialize)]
pub struct HookStatusReport {
    pub hook_count: usize,
    pub per_tool: Vec<ToolHookStatus>,
}

/// Hook translation status for a single tool.
#[derive(Debug, Clone, Serialize)]
pub struct ToolHookStatus {
    pub tool: ToolKind,
    pub supported: bool,
    pub translated: bool,
    pub details: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_kind_variants_exist() {
        let tools = [ToolKind::ClaudeCode, ToolKind::Cursor, ToolKind::OpenCode, ToolKind::Windsurf, ToolKind::Codex];
        assert_eq!(tools.len(), 5);
    }

    #[test]
    fn test_tool_kind_equality() {
        assert_eq!(ToolKind::ClaudeCode, ToolKind::ClaudeCode);
        assert_ne!(ToolKind::ClaudeCode, ToolKind::Cursor);
    }

    #[test]
    fn test_tool_kind_clone() {
        let t = ToolKind::Cursor;
        let t2 = t.clone(); // Clone (no longer Copy)
        assert_eq!(t, t2);
    }

    #[test]
    fn test_tool_kind_debug() {
        let debug = format!("{:?}", ToolKind::OpenCode);
        assert_eq!(debug, "OpenCode");
    }

    #[test]
    fn test_tool_kind_custom_variant() {
        let custom = ToolKind::Custom("aider".to_string());
        assert_eq!(custom, ToolKind::Custom("aider".to_string()));
        assert_ne!(custom, ToolKind::ClaudeCode);
        let debug = format!("{:?}", custom);
        assert!(debug.contains("Custom"));
        assert!(debug.contains("aider"));
    }

    #[test]
    fn test_tool_kind_as_str() {
        assert_eq!(ToolKind::ClaudeCode.as_str(), "claude-code");
        assert_eq!(ToolKind::Cursor.as_str(), "cursor");
        assert_eq!(ToolKind::OpenCode.as_str(), "opencode");
        assert_eq!(ToolKind::Windsurf.as_str(), "windsurf");
        assert_eq!(ToolKind::Codex.as_str(), "codex");
        assert_eq!(
            ToolKind::Custom("aider".to_string()).as_str(),
            "aider"
        );
    }

    #[test]
    fn test_tool_kind_display() {
        assert_eq!(format!("{}", ToolKind::ClaudeCode), "claude-code");
        assert_eq!(format!("{}", ToolKind::Cursor), "cursor");
        assert_eq!(format!("{}", ToolKind::OpenCode), "opencode");
        assert_eq!(format!("{}", ToolKind::Windsurf), "windsurf");
        assert_eq!(format!("{}", ToolKind::Codex), "codex");
        assert_eq!(
            format!("{}", ToolKind::Custom("aider".to_string())),
            "aider"
        );
    }

    #[test]
    fn test_tool_kind_serialize() {
        assert_eq!(
            serde_json::to_string(&ToolKind::ClaudeCode).unwrap(),
            "\"claude-code\""
        );
        assert_eq!(
            serde_json::to_string(&ToolKind::Cursor).unwrap(),
            "\"cursor\""
        );
        assert_eq!(
            serde_json::to_string(&ToolKind::OpenCode).unwrap(),
            "\"opencode\""
        );
        assert_eq!(
            serde_json::to_string(&ToolKind::Windsurf).unwrap(),
            "\"windsurf\""
        );
        assert_eq!(
            serde_json::to_string(&ToolKind::Codex).unwrap(),
            "\"codex\""
        );
        assert_eq!(
            serde_json::to_string(&ToolKind::Custom("aider".to_string())).unwrap(),
            "\"aider\""
        );
    }

    #[test]
    fn test_tool_kind_deserialize() {
        assert_eq!(
            serde_json::from_str::<ToolKind>("\"claude-code\"").unwrap(),
            ToolKind::ClaudeCode
        );
        assert_eq!(
            serde_json::from_str::<ToolKind>("\"cursor\"").unwrap(),
            ToolKind::Cursor
        );
        assert_eq!(
            serde_json::from_str::<ToolKind>("\"opencode\"").unwrap(),
            ToolKind::OpenCode
        );
        assert_eq!(
            serde_json::from_str::<ToolKind>("\"windsurf\"").unwrap(),
            ToolKind::Windsurf
        );
        assert_eq!(
            serde_json::from_str::<ToolKind>("\"codex\"").unwrap(),
            ToolKind::Codex
        );
        assert_eq!(
            serde_json::from_str::<ToolKind>("\"aider\"").unwrap(),
            ToolKind::Custom("aider".to_string())
        );
    }

    #[test]
    fn test_tool_kind_serde_roundtrip() {
        let variants = vec![
            ToolKind::ClaudeCode,
            ToolKind::Cursor,
            ToolKind::OpenCode,
            ToolKind::Windsurf,
            ToolKind::Codex,
            ToolKind::Custom("aider".to_string()),
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let back: ToolKind = serde_json::from_str(&json).unwrap();
            assert_eq!(v, back);
        }
    }

    #[test]
    fn test_tool_kind_deserialize_normalizes_known() {
        // Deserializing known tool names should produce the named variant, not Custom
        let claude: ToolKind = serde_json::from_str("\"claude-code\"").unwrap();
        assert!(matches!(claude, ToolKind::ClaudeCode));
        // Not Custom("claude-code")
        assert!(!matches!(claude, ToolKind::Custom(_)));
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
