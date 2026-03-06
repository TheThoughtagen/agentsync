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
