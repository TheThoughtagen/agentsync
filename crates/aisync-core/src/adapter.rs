use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::AisyncError;
use crate::types::{Confidence, DriftState, HookTranslation, HooksConfig, SyncAction, ToolKind};

/// Result of detecting a specific AI tool in a project directory.
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// Which tool was checked.
    pub tool: ToolKind,
    /// Whether the tool was detected.
    pub detected: bool,
    /// Confidence level of the detection.
    pub confidence: Confidence,
    /// Filesystem markers that were found (files/directories).
    pub markers_found: Vec<PathBuf>,
    /// Optional hint about the tool version or configuration format.
    pub version_hint: Option<String>,
}

/// Trait for AI tool adapters that can detect, read, sync, and check status.
///
/// All adapters must be Send + Sync to support the Plugin variant (Arc-wrapped).
pub trait ToolAdapter: Send + Sync {
    /// Returns which tool this adapter handles.
    fn name(&self) -> ToolKind;

    /// Human-readable display name (e.g., "Claude Code", "Cursor", "OpenCode").
    fn display_name(&self) -> &str;

    /// Relative path from project root to the tool's native instruction file.
    fn native_instruction_path(&self) -> &str;

    /// Conditional tag names that match this tool (e.g., ["claude-only", "claude-code-only"]).
    fn conditional_tags(&self) -> &[&str] {
        &[]
    }

    /// Entries to add to .gitignore when this tool is synced.
    fn gitignore_entries(&self) -> Vec<String> {
        vec![]
    }

    /// Relative paths to watch for reverse sync (defaults to native_instruction_path).
    fn watch_paths(&self) -> Vec<&str> {
        vec![self.native_instruction_path()]
    }

    /// Default sync strategy for this tool (overridable in config).
    fn default_sync_strategy(&self) -> crate::config::SyncStrategy {
        crate::config::SyncStrategy::Symlink
    }

    /// Detect whether this tool is configured in the given project directory.
    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError>;

    /// Read existing instructions from this tool's native format.
    /// Returns None if no instructions file exists.
    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AisyncError> {
        let _ = project_root;
        Ok(None)
    }

    /// Plan sync actions for this tool (does not execute).
    fn plan_sync(
        &self,
        project_root: &Path,
        canonical_content: &str,
        strategy: crate::config::SyncStrategy,
    ) -> Result<Vec<crate::types::SyncAction>, AisyncError> {
        let _ = (project_root, canonical_content, strategy);
        Ok(vec![])
    }

    /// Check sync status for this tool.
    fn sync_status(
        &self,
        project_root: &Path,
        canonical_hash: &str,
        strategy: crate::config::SyncStrategy,
    ) -> Result<crate::types::ToolSyncStatus, AisyncError> {
        let _ = (project_root, canonical_hash);
        Ok(crate::types::ToolSyncStatus {
            tool: self.name(),
            strategy,
            drift: DriftState::NotConfigured,
            details: None,
        })
    }

    /// Plan memory sync actions for this tool.
    fn plan_memory_sync(
        &self,
        project_root: &Path,
        memory_files: &[PathBuf],
    ) -> Result<Vec<SyncAction>, AisyncError> {
        let _ = (project_root, memory_files);
        Ok(vec![]) // Default: no memory sync
    }

    /// Translate hooks to this tool's native format.
    fn translate_hooks(&self, hooks: &HooksConfig) -> Result<HookTranslation, AisyncError> {
        let _ = hooks;
        Ok(HookTranslation::Unsupported {
            tool: self.name(),
            reason: "hooks not supported by this tool".into(),
        })
    }
}

/// Zero-sized adapter structs for compile-time dispatch.
#[derive(Debug, Clone)]
pub struct ClaudeCodeAdapter;

#[derive(Debug, Clone)]
pub struct CursorAdapter;

#[derive(Debug, Clone)]
pub struct OpenCodeAdapter;

#[derive(Debug, Clone)]
pub struct WindsurfAdapter;

#[derive(Debug, Clone)]
pub struct CodexAdapter;

/// Macro to dispatch a method call through all AnyAdapter variants.
///
/// Adding a new built-in variant requires adding one line per variant here.
/// The Plugin variant uses Arc<dyn ToolAdapter>, which auto-derefs for method calls.
macro_rules! dispatch_adapter {
    ($self:expr, $inner:ident => $body:expr) => {
        match $self {
            AnyAdapter::ClaudeCode($inner) => $body,
            AnyAdapter::Cursor($inner) => $body,
            AnyAdapter::OpenCode($inner) => $body,
            AnyAdapter::Windsurf($inner) => $body,
            AnyAdapter::Codex($inner) => $body,
            AnyAdapter::Plugin($inner) => $body,
        }
    };
}

/// Enum-based dispatch for all tool adapters.
///
/// Uses compile-time dispatch (enum) for built-in adapters and dynamic dispatch
/// via Arc<dyn ToolAdapter> for plugin adapters. The Plugin variant enables
/// future SDK adapters to plug in without modifying this enum.
pub enum AnyAdapter {
    ClaudeCode(ClaudeCodeAdapter),
    Cursor(CursorAdapter),
    OpenCode(OpenCodeAdapter),
    Windsurf(WindsurfAdapter),
    Codex(CodexAdapter),
    Plugin(Arc<dyn ToolAdapter>),
}

impl fmt::Debug for AnyAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnyAdapter::ClaudeCode(a) => f.debug_tuple("ClaudeCode").field(a).finish(),
            AnyAdapter::Cursor(a) => f.debug_tuple("Cursor").field(a).finish(),
            AnyAdapter::OpenCode(a) => f.debug_tuple("OpenCode").field(a).finish(),
            AnyAdapter::Windsurf(a) => f.debug_tuple("Windsurf").field(a).finish(),
            AnyAdapter::Codex(a) => f.debug_tuple("Codex").field(a).finish(),
            AnyAdapter::Plugin(_) => f.debug_tuple("Plugin").field(&"dyn ToolAdapter").finish(),
        }
    }
}

impl Clone for AnyAdapter {
    fn clone(&self) -> Self {
        match self {
            AnyAdapter::ClaudeCode(a) => AnyAdapter::ClaudeCode(a.clone()),
            AnyAdapter::Cursor(a) => AnyAdapter::Cursor(a.clone()),
            AnyAdapter::OpenCode(a) => AnyAdapter::OpenCode(a.clone()),
            AnyAdapter::Windsurf(a) => AnyAdapter::Windsurf(a.clone()),
            AnyAdapter::Codex(a) => AnyAdapter::Codex(a.clone()),
            AnyAdapter::Plugin(a) => AnyAdapter::Plugin(Arc::clone(a)),
        }
    }
}

impl AnyAdapter {
    /// Returns one instance of each built-in adapter variant.
    pub fn all_builtin() -> Vec<AnyAdapter> {
        vec![
            AnyAdapter::ClaudeCode(ClaudeCodeAdapter),
            AnyAdapter::Cursor(CursorAdapter),
            AnyAdapter::OpenCode(OpenCodeAdapter),
            AnyAdapter::Windsurf(WindsurfAdapter),
            AnyAdapter::Codex(CodexAdapter),
        ]
    }

    /// Factory method: returns the appropriate adapter for a built-in tool kind.
    /// Returns None for Custom tools (which require a Plugin adapter).
    pub fn for_tool(kind: &ToolKind) -> Option<AnyAdapter> {
        match kind {
            ToolKind::ClaudeCode => Some(AnyAdapter::ClaudeCode(ClaudeCodeAdapter)),
            ToolKind::Cursor => Some(AnyAdapter::Cursor(CursorAdapter)),
            ToolKind::OpenCode => Some(AnyAdapter::OpenCode(OpenCodeAdapter)),
            ToolKind::Windsurf => Some(AnyAdapter::Windsurf(WindsurfAdapter)),
            ToolKind::Codex => Some(AnyAdapter::Codex(CodexAdapter)),
            ToolKind::Custom(_) => None,
        }
    }
}

impl ToolAdapter for AnyAdapter {
    fn name(&self) -> ToolKind {
        dispatch_adapter!(self, a => a.name())
    }

    fn display_name(&self) -> &str {
        dispatch_adapter!(self, a => a.display_name())
    }

    fn native_instruction_path(&self) -> &str {
        dispatch_adapter!(self, a => a.native_instruction_path())
    }

    fn conditional_tags(&self) -> &[&str] {
        dispatch_adapter!(self, a => a.conditional_tags())
    }

    fn gitignore_entries(&self) -> Vec<String> {
        dispatch_adapter!(self, a => a.gitignore_entries())
    }

    fn watch_paths(&self) -> Vec<&str> {
        dispatch_adapter!(self, a => a.watch_paths())
    }

    fn default_sync_strategy(&self) -> crate::config::SyncStrategy {
        dispatch_adapter!(self, a => a.default_sync_strategy())
    }

    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError> {
        dispatch_adapter!(self, a => a.detect(project_root))
    }

    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AisyncError> {
        dispatch_adapter!(self, a => a.read_instructions(project_root))
    }

    fn plan_sync(
        &self,
        project_root: &Path,
        canonical_content: &str,
        strategy: crate::config::SyncStrategy,
    ) -> Result<Vec<crate::types::SyncAction>, AisyncError> {
        dispatch_adapter!(self, a => a.plan_sync(project_root, canonical_content, strategy))
    }

    fn sync_status(
        &self,
        project_root: &Path,
        canonical_hash: &str,
        strategy: crate::config::SyncStrategy,
    ) -> Result<crate::types::ToolSyncStatus, AisyncError> {
        dispatch_adapter!(self, a => a.sync_status(project_root, canonical_hash, strategy))
    }

    fn plan_memory_sync(
        &self,
        project_root: &Path,
        memory_files: &[PathBuf],
    ) -> Result<Vec<SyncAction>, AisyncError> {
        dispatch_adapter!(self, a => a.plan_memory_sync(project_root, memory_files))
    }

    fn translate_hooks(&self, hooks: &HooksConfig) -> Result<HookTranslation, AisyncError> {
        dispatch_adapter!(self, a => a.translate_hooks(hooks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_builtin_returns_five() {
        let adapters = AnyAdapter::all_builtin();
        assert_eq!(adapters.len(), 5);
    }

    #[test]
    fn test_for_tool_claude_code() {
        let adapter = AnyAdapter::for_tool(&ToolKind::ClaudeCode);
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().name(), ToolKind::ClaudeCode);
    }

    #[test]
    fn test_for_tool_cursor() {
        let adapter = AnyAdapter::for_tool(&ToolKind::Cursor);
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().name(), ToolKind::Cursor);
    }

    #[test]
    fn test_for_tool_opencode() {
        let adapter = AnyAdapter::for_tool(&ToolKind::OpenCode);
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().name(), ToolKind::OpenCode);
    }

    #[test]
    fn test_for_tool_windsurf() {
        let adapter = AnyAdapter::for_tool(&ToolKind::Windsurf);
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().name(), ToolKind::Windsurf);
    }

    #[test]
    fn test_for_tool_codex() {
        let adapter = AnyAdapter::for_tool(&ToolKind::Codex);
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().name(), ToolKind::Codex);
    }

    #[test]
    fn test_for_tool_custom_returns_none() {
        let adapter = AnyAdapter::for_tool(&ToolKind::Custom("x".into()));
        assert!(adapter.is_none());
    }

    #[test]
    fn test_plugin_variant_dispatch() {
        let plugin = AnyAdapter::Plugin(Arc::new(ClaudeCodeAdapter));
        assert_eq!(plugin.display_name(), "Claude Code");
        assert_eq!(plugin.name(), ToolKind::ClaudeCode);
        assert_eq!(plugin.native_instruction_path(), "CLAUDE.md");
    }

    #[test]
    fn test_plugin_clone_via_arc() {
        let plugin = AnyAdapter::Plugin(Arc::new(CursorAdapter));
        let cloned = plugin.clone();
        assert_eq!(cloned.name(), ToolKind::Cursor);
    }

    #[test]
    fn test_plugin_debug() {
        let plugin = AnyAdapter::Plugin(Arc::new(OpenCodeAdapter));
        let debug = format!("{:?}", plugin);
        assert!(debug.contains("Plugin"));
    }

    #[test]
    fn test_dispatch_display_name() {
        let claude = AnyAdapter::ClaudeCode(ClaudeCodeAdapter);
        assert_eq!(claude.display_name(), "Claude Code");
        let cursor = AnyAdapter::Cursor(CursorAdapter);
        assert_eq!(cursor.display_name(), "Cursor");
        let opencode = AnyAdapter::OpenCode(OpenCodeAdapter);
        assert_eq!(opencode.display_name(), "OpenCode");
    }

    #[test]
    fn test_dispatch_native_instruction_path() {
        let claude = AnyAdapter::ClaudeCode(ClaudeCodeAdapter);
        assert_eq!(claude.native_instruction_path(), "CLAUDE.md");
        let cursor = AnyAdapter::Cursor(CursorAdapter);
        assert_eq!(cursor.native_instruction_path(), ".cursor/rules/project.mdc");
        let opencode = AnyAdapter::OpenCode(OpenCodeAdapter);
        assert_eq!(opencode.native_instruction_path(), "AGENTS.md");
    }

    #[test]
    fn test_dispatch_conditional_tags() {
        let claude = AnyAdapter::ClaudeCode(ClaudeCodeAdapter);
        assert_eq!(claude.conditional_tags(), &["claude-only", "claude-code-only"]);
        let cursor = AnyAdapter::Cursor(CursorAdapter);
        assert_eq!(cursor.conditional_tags(), &["cursor-only"]);
        let opencode = AnyAdapter::OpenCode(OpenCodeAdapter);
        assert_eq!(opencode.conditional_tags(), &["opencode-only"]);
    }

    #[test]
    fn test_dispatch_default_sync_strategy() {
        use crate::config::SyncStrategy;
        let claude = AnyAdapter::ClaudeCode(ClaudeCodeAdapter);
        assert_eq!(claude.default_sync_strategy(), SyncStrategy::Symlink);
        let cursor = AnyAdapter::Cursor(CursorAdapter);
        assert_eq!(cursor.default_sync_strategy(), SyncStrategy::Generate);
    }

    #[test]
    fn test_safe_defaults_read_instructions() {
        // A plugin with no read_instructions override returns Ok(None)
        let plugin = AnyAdapter::Plugin(Arc::new(MinimalAdapter));
        let result = plugin.read_instructions(Path::new(".")).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_safe_defaults_plan_sync() {
        let plugin = AnyAdapter::Plugin(Arc::new(MinimalAdapter));
        let result = plugin
            .plan_sync(Path::new("."), "content", crate::config::SyncStrategy::Symlink)
            .unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_safe_defaults_sync_status() {
        let plugin = AnyAdapter::Plugin(Arc::new(MinimalAdapter));
        let result = plugin
            .sync_status(Path::new("."), "hash", crate::config::SyncStrategy::Symlink)
            .unwrap();
        assert_eq!(result.drift, DriftState::NotConfigured);
    }

    /// Minimal adapter for testing safe defaults.
    struct MinimalAdapter;

    impl ToolAdapter for MinimalAdapter {
        fn name(&self) -> ToolKind {
            ToolKind::Custom("minimal".into())
        }
        fn display_name(&self) -> &str {
            "Minimal"
        }
        fn native_instruction_path(&self) -> &str {
            "MINIMAL.md"
        }
        fn detect(&self, _project_root: &Path) -> Result<DetectionResult, AisyncError> {
            Ok(DetectionResult {
                tool: self.name(),
                detected: false,
                confidence: Confidence::Medium,
                markers_found: vec![],
                version_hint: None,
            })
        }
    }
}
