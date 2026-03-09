use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Helper for serde default that returns `true`.
fn default_true() -> bool {
    true
}

/// Strategy for synchronizing configuration files between tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum SyncStrategy {
    #[default]
    Symlink,
    Copy,
    Generate,
}

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

/// Metadata for a rule file (description, globs, always_apply).
///
/// Maps to YAML frontmatter in canonical rule files. Serde-enabled for
/// serialization to JSON/YAML proxies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleMetadata {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub globs: Vec<String>,
    #[serde(default = "default_true")]
    pub always_apply: bool,
}

/// A canonical rule file with metadata and content.
///
/// Not serde-enabled — contains PathBuf for internal pipeline use only.
#[derive(Debug, Clone)]
pub struct RuleFile {
    pub name: String,
    pub metadata: RuleMetadata,
    pub content: String,
    pub source_path: PathBuf,
}

/// An MCP server entry with command, args, and environment variables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// MCP configuration containing a map of named servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub servers: BTreeMap<String, McpServer>,
}

/// A canonical command file with name, content, and source path.
///
/// Not serde-enabled — contains PathBuf for internal pipeline use only.
#[derive(Debug, Clone)]
pub struct CommandFile {
    pub name: String,
    pub content: String,
    pub source_path: PathBuf,
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
    WarnContentSize {
        tool: ToolKind,
        path: PathBuf,
        actual_size: usize,
        limit: usize,
        unit: String,
    },
    // Rule sync actions
    CreateRuleFile {
        output: PathBuf,
        content: String,
        rule_name: String,
    },
    // MCP sync actions
    WriteMcpConfig {
        output: PathBuf,
        content: String,
    },
    // Command sync actions
    CopyCommandFile {
        source: PathBuf,
        output: PathBuf,
        command_name: String,
    },
    // Dimension warnings
    WarnUnsupportedDimension {
        tool: ToolKind,
        dimension: String,
        reason: String,
    },
}

impl fmt::Display for SyncAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncAction::CreateSymlink { link, target } => {
                write!(
                    f,
                    "Create symlink: {} -> {}",
                    link.display(),
                    target.display()
                )
            }
            SyncAction::RemoveAndRelink { link, target } => {
                write!(
                    f,
                    "Remove and relink: {} -> {}",
                    link.display(),
                    target.display()
                )
            }
            SyncAction::GenerateMdc { output, .. } => {
                write!(f, "Generate MDC file: {}", output.display())
            }
            SyncAction::UpdateGitignore { path, entries } => {
                write!(
                    f,
                    "Update .gitignore at {} with {} entries",
                    path.display(),
                    entries.len()
                )
            }
            SyncAction::CreateDirectory { path } => {
                write!(f, "Create directory: {}", path.display())
            }
            SyncAction::CreateFile { path, .. } => {
                write!(f, "Create file: {}", path.display())
            }
            SyncAction::RemoveFile { path } => {
                write!(f, "Remove file: {}", path.display())
            }
            SyncAction::SkipExistingFile { path, reason } => {
                write!(f, "Skip {}: {}", path.display(), reason)
            }
            SyncAction::CreateMemorySymlink { link, target } => {
                write!(
                    f,
                    "Create memory symlink: {} -> {}",
                    link.display(),
                    target.display()
                )
            }
            SyncAction::UpdateMemoryReferences {
                path, references, ..
            } => {
                write!(
                    f,
                    "Update memory references in {} with {} entries",
                    path.display(),
                    references.len()
                )
            }
            SyncAction::WriteHookTranslation { path, tool, .. } => {
                write!(
                    f,
                    "Write hook translation for {:?} to {}",
                    tool,
                    path.display()
                )
            }
            SyncAction::WarnUnsupportedHooks { tool, reason } => {
                write!(f, "Warning: hooks unsupported for {:?}: {}", tool, reason)
            }
            SyncAction::WarnContentSize {
                tool,
                path,
                actual_size,
                limit,
                unit,
            } => {
                write!(
                    f,
                    "Warning: {} content ({} {}) exceeds limit ({} {}) for {}",
                    tool.display_name(),
                    actual_size,
                    unit,
                    limit,
                    unit,
                    path.display()
                )
            }
            SyncAction::CreateRuleFile {
                output, rule_name, ..
            } => {
                write!(
                    f,
                    "Create rule file for '{}': {}",
                    rule_name,
                    output.display()
                )
            }
            SyncAction::WriteMcpConfig { output, .. } => {
                write!(f, "Write MCP config: {}", output.display())
            }
            SyncAction::CopyCommandFile {
                output,
                command_name,
                ..
            } => {
                write!(
                    f,
                    "Copy command '{}': {}",
                    command_name,
                    output.display()
                )
            }
            SyncAction::WarnUnsupportedDimension {
                tool,
                dimension,
                reason,
            } => {
                write!(
                    f,
                    "Warning: {} sync unsupported for {}: {}",
                    dimension,
                    tool.display_name(),
                    reason
                )
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
    pub strategy: SyncStrategy,
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
        let claude: ToolKind = serde_json::from_str("\"claude-code\"").unwrap();
        assert!(matches!(claude, ToolKind::ClaudeCode));
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

    #[test]
    fn test_sync_strategy_default() {
        assert_eq!(SyncStrategy::default(), SyncStrategy::Symlink);
    }

    #[test]
    fn test_sync_strategy_serde_roundtrip() {
        let strategies = [SyncStrategy::Symlink, SyncStrategy::Copy, SyncStrategy::Generate];
        for s in strategies {
            let json = serde_json::to_string(&s).unwrap();
            let back: SyncStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(s, back);
        }
    }

    #[test]
    fn test_sync_strategy_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&SyncStrategy::Symlink).unwrap(), "\"symlink\"");
        assert_eq!(serde_json::to_string(&SyncStrategy::Copy).unwrap(), "\"copy\"");
        assert_eq!(serde_json::to_string(&SyncStrategy::Generate).unwrap(), "\"generate\"");
    }

    // --- New type tests (Phase 12, Plan 01) ---

    #[test]
    fn test_rule_metadata_construction() {
        let meta = RuleMetadata {
            description: Some("A test rule".into()),
            globs: vec!["*.rs".into()],
            always_apply: false,
        };
        assert_eq!(meta.description, Some("A test rule".into()));
        assert_eq!(meta.globs, vec!["*.rs".to_string()]);
        assert!(!meta.always_apply);
    }

    #[test]
    fn test_rule_metadata_defaults() {
        let meta: RuleMetadata = serde_json::from_str("{}").unwrap();
        assert_eq!(meta.description, None);
        assert!(meta.globs.is_empty());
        assert!(meta.always_apply); // default_true
    }

    #[test]
    fn test_rule_file_construction() {
        let rf = RuleFile {
            name: "my-rule".into(),
            metadata: RuleMetadata {
                description: None,
                globs: vec![],
                always_apply: true,
            },
            content: "rule content".into(),
            source_path: PathBuf::from("/src/rules/my-rule.md"),
        };
        assert_eq!(rf.name, "my-rule");
        assert_eq!(rf.content, "rule content");
        assert_eq!(rf.source_path, PathBuf::from("/src/rules/my-rule.md"));
    }

    #[test]
    fn test_mcp_server_construction() {
        let server = McpServer {
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-filesystem".into()],
            env: BTreeMap::from([("KEY".into(), "val".into())]),
        };
        assert_eq!(server.command, "npx");
        assert_eq!(server.args.len(), 2);
        assert_eq!(server.env.get("KEY"), Some(&"val".to_string()));
    }

    #[test]
    fn test_mcp_server_defaults() {
        let server: McpServer = serde_json::from_str(r#"{"command":"npx"}"#).unwrap();
        assert_eq!(server.command, "npx");
        assert!(server.args.is_empty());
        assert!(server.env.is_empty());
    }

    #[test]
    fn test_mcp_config_construction() {
        let config = McpConfig {
            servers: BTreeMap::from([("fs".into(), McpServer {
                command: "npx".into(),
                args: vec![],
                env: BTreeMap::new(),
            })]),
        };
        assert!(config.servers.contains_key("fs"));
    }

    #[test]
    fn test_mcp_config_defaults() {
        let config: McpConfig = serde_json::from_str("{}").unwrap();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn test_command_file_construction() {
        let cf = CommandFile {
            name: "build".into(),
            content: "cargo build".into(),
            source_path: PathBuf::from("/commands/build.sh"),
        };
        assert_eq!(cf.name, "build");
        assert_eq!(cf.content, "cargo build");
        assert_eq!(cf.source_path, PathBuf::from("/commands/build.sh"));
    }

    #[test]
    fn test_mcp_config_toml_roundtrip() {
        let config = McpConfig {
            servers: BTreeMap::from([("fs".into(), McpServer {
                command: "npx".into(),
                args: vec!["-y".into(), "server".into()],
                env: BTreeMap::from([("HOME".into(), "/home".into())]),
            })]),
        };
        let toml_str = toml::to_string(&config).unwrap();
        let back: McpConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.servers.len(), 1);
        let server = back.servers.get("fs").unwrap();
        assert_eq!(server.command, "npx");
        assert_eq!(server.args, vec!["-y", "server"]);
        assert_eq!(server.env.get("HOME"), Some(&"/home".to_string()));
    }

    #[test]
    fn test_mcp_server_toml_roundtrip() {
        let server = McpServer {
            command: "node".into(),
            args: vec!["index.js".into()],
            env: BTreeMap::new(),
        };
        let toml_str = toml::to_string(&server).unwrap();
        let back: McpServer = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.command, "node");
        assert_eq!(back.args, vec!["index.js"]);
    }

    #[test]
    fn test_rule_metadata_serde_json_roundtrip() {
        let meta = RuleMetadata {
            description: Some("test".into()),
            globs: vec!["*.ts".into()],
            always_apply: false,
        };
        let json = serde_json::to_string(&meta).unwrap();
        let back: RuleMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(back.description, Some("test".into()));
        assert_eq!(back.globs, vec!["*.ts".to_string()]);
        assert!(!back.always_apply);
    }

    #[test]
    fn test_sync_action_create_rule_file_display() {
        let action = SyncAction::CreateRuleFile {
            output: PathBuf::from(".cursor/rules/aisync-my-rule.mdc"),
            content: "rule content".into(),
            rule_name: "my-rule".into(),
        };
        let display = format!("{}", action);
        assert!(display.contains("my-rule"), "display should contain rule name");
        assert!(display.contains("aisync-my-rule.mdc"), "display should contain output path");
    }

    #[test]
    fn test_sync_action_write_mcp_config_display() {
        let action = SyncAction::WriteMcpConfig {
            output: PathBuf::from(".cursor/mcp.json"),
            content: "{}".into(),
        };
        let display = format!("{}", action);
        assert!(display.contains("MCP"));
        assert!(display.contains("mcp.json"));
    }

    #[test]
    fn test_sync_action_copy_command_file_display() {
        let action = SyncAction::CopyCommandFile {
            source: PathBuf::from("commands/build.sh"),
            output: PathBuf::from(".cursor/commands/build.sh"),
            command_name: "build".into(),
        };
        let display = format!("{}", action);
        assert!(display.contains("build"));
        assert!(display.contains(".cursor/commands/build.sh"));
    }

    #[test]
    fn test_sync_action_warn_unsupported_dimension_display() {
        let action = SyncAction::WarnUnsupportedDimension {
            tool: ToolKind::OpenCode,
            dimension: "commands".into(),
            reason: "no command format documented".into(),
        };
        let display = format!("{}", action);
        assert!(display.contains("OpenCode"), "should contain tool name");
        assert!(display.contains("commands"), "should contain dimension");
        assert!(display.contains("no command format documented"), "should contain reason");
    }
}
