//! ToolAdapter trait and supporting types for building aisync adapters.
//!
//! Community developers can depend on this crate (plus `aisync-types`) to
//! implement custom tool adapters without pulling in all of `aisync-core`.

pub use aisync_types;

use std::path::{Path, PathBuf};

use aisync_types::{
    AgentFile, CommandFile, Confidence, DriftState, HookTranslation, HooksConfig, McpConfig,
    RuleFile, SkillFile, SyncAction, SyncStrategy, ToolKind, ToolSyncStatus,
};

/// Errors specific to individual tool adapters.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("detection failed: {0}")]
    DetectionFailed(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

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
    fn default_sync_strategy(&self) -> SyncStrategy {
        SyncStrategy::Symlink
    }

    /// Detect whether this tool is configured in the given project directory.
    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AdapterError>;

    /// Read existing instructions from this tool's native format.
    /// Returns None if no instructions file exists.
    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AdapterError> {
        let _ = project_root;
        Ok(None)
    }

    /// Plan sync actions for this tool (does not execute).
    fn plan_sync(
        &self,
        project_root: &Path,
        canonical_content: &str,
        strategy: SyncStrategy,
    ) -> Result<Vec<SyncAction>, AdapterError> {
        let _ = (project_root, canonical_content, strategy);
        Ok(vec![])
    }

    /// Check sync status for this tool.
    fn sync_status(
        &self,
        project_root: &Path,
        canonical_hash: &str,
        strategy: SyncStrategy,
    ) -> Result<ToolSyncStatus, AdapterError> {
        let _ = (project_root, canonical_hash);
        Ok(ToolSyncStatus {
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
    ) -> Result<Vec<SyncAction>, AdapterError> {
        let _ = (project_root, memory_files);
        Ok(vec![]) // Default: no memory sync
    }

    /// Translate hooks to this tool's native format.
    fn translate_hooks(&self, hooks: &HooksConfig) -> Result<HookTranslation, AdapterError> {
        let _ = hooks;
        Ok(HookTranslation::Unsupported {
            tool: self.name(),
            reason: "hooks not supported by this tool".into(),
        })
    }

    /// Plan rule file sync actions for this tool.
    fn plan_rules_sync(
        &self,
        project_root: &Path,
        rules: &[RuleFile],
    ) -> Result<Vec<SyncAction>, AdapterError> {
        let _ = (project_root, rules);
        Ok(vec![])
    }

    /// Plan MCP config sync actions for this tool.
    fn plan_mcp_sync(
        &self,
        project_root: &Path,
        mcp_config: &McpConfig,
    ) -> Result<Vec<SyncAction>, AdapterError> {
        let _ = (project_root, mcp_config);
        Ok(vec![])
    }

    /// Plan command file sync actions for this tool.
    fn plan_commands_sync(
        &self,
        project_root: &Path,
        commands: &[CommandFile],
    ) -> Result<Vec<SyncAction>, AdapterError> {
        let _ = (project_root, commands);
        Ok(vec![])
    }

    /// Plan skill file sync actions for this tool.
    fn plan_skills_sync(
        &self,
        project_root: &Path,
        skills: &[SkillFile],
    ) -> Result<Vec<SyncAction>, AdapterError> {
        let _ = (project_root, skills);
        Ok(vec![])
    }

    /// Plan agent file sync actions for this tool.
    fn plan_agents_sync(
        &self,
        project_root: &Path,
        agents: &[AgentFile],
    ) -> Result<Vec<SyncAction>, AdapterError> {
        let _ = (project_root, agents);
        Ok(vec![])
    }
}

/// A registration entry for compile-time adapter discovery.
///
/// Community adapter crates submit instances via `inventory::submit!`.
/// The aisync binary iterates all submissions via `inventory::iter::<AdapterFactory>`.
///
/// # Example
///
/// ```rust,ignore
/// inventory::submit! {
///     AdapterFactory {
///         name: "my-tool",
///         create: || Box::new(MyToolAdapter),
///     }
/// }
/// ```
pub struct AdapterFactory {
    /// Identifier used for deduplication and logging.
    pub name: &'static str,
    /// Constructor function called once during adapter collection.
    pub create: fn() -> Box<dyn ToolAdapter>,
}

inventory::collect!(AdapterFactory);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection_result_fields() {
        let result = DetectionResult {
            tool: ToolKind::ClaudeCode,
            detected: true,
            confidence: Confidence::High,
            markers_found: vec![PathBuf::from("CLAUDE.md")],
            version_hint: Some("v1".to_string()),
        };
        assert_eq!(result.tool, ToolKind::ClaudeCode);
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
        assert_eq!(result.markers_found.len(), 1);
        assert_eq!(result.version_hint, Some("v1".to_string()));
    }

    /// Minimal test adapter for AdapterFactory tests.
    struct TestAdapter;

    impl ToolAdapter for TestAdapter {
        fn name(&self) -> ToolKind {
            ToolKind::Custom("test-factory".to_string())
        }
        fn display_name(&self) -> &str {
            "Test Factory Adapter"
        }
        fn native_instruction_path(&self) -> &str {
            ".test-factory/instructions.md"
        }
        fn detect(&self, _project_root: &Path) -> Result<DetectionResult, AdapterError> {
            Ok(DetectionResult {
                tool: self.name(),
                detected: false,
                confidence: Confidence::Medium,
                markers_found: vec![],
                version_hint: None,
            })
        }
    }

    #[test]
    fn test_adapter_factory_create() {
        let factory = AdapterFactory {
            name: "test-factory",
            create: || Box::new(TestAdapter),
        };
        assert_eq!(factory.name, "test-factory");
        let adapter = (factory.create)();
        assert_eq!(adapter.display_name(), "Test Factory Adapter");
        assert_eq!(adapter.name(), ToolKind::Custom("test-factory".to_string()));
    }

    #[test]
    fn test_adapter_error_detection_failed() {
        let err = AdapterError::DetectionFailed("no markers".to_string());
        assert!(format!("{err}").contains("no markers"));
    }

    #[test]
    fn test_adapter_error_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: AdapterError = io_err.into();
        assert!(matches!(err, AdapterError::Io(_)));
        assert!(format!("{err}").contains("file not found"));
    }

    #[test]
    fn test_adapter_error_other() {
        let err = AdapterError::Other("something went wrong".to_string());
        assert!(format!("{err}").contains("something went wrong"));
    }

    #[test]
    fn test_plan_skills_sync_default_returns_empty() {
        struct MinimalAdapter;
        impl ToolAdapter for MinimalAdapter {
            fn name(&self) -> ToolKind { ToolKind::Custom("minimal".to_string()) }
            fn display_name(&self) -> &str { "Minimal" }
            fn native_instruction_path(&self) -> &str { ".minimal/instructions.md" }
            fn detect(&self, _: &Path) -> Result<DetectionResult, AdapterError> {
                Ok(DetectionResult {
                    tool: self.name(),
                    detected: false,
                    confidence: Confidence::Medium,
                    markers_found: vec![],
                    version_hint: None,
                })
            }
        }
        let adapter = MinimalAdapter;
        let skills: Vec<aisync_types::SkillFile> = vec![];
        let result = adapter.plan_skills_sync(Path::new("/tmp"), &skills).unwrap();
        assert!(result.is_empty(), "default plan_skills_sync should return empty vec");
    }

    #[test]
    fn test_plan_agents_sync_default_returns_empty() {
        struct MinimalAdapter;
        impl ToolAdapter for MinimalAdapter {
            fn name(&self) -> ToolKind { ToolKind::Custom("minimal2".to_string()) }
            fn display_name(&self) -> &str { "Minimal2" }
            fn native_instruction_path(&self) -> &str { ".minimal2/instructions.md" }
            fn detect(&self, _: &Path) -> Result<DetectionResult, AdapterError> {
                Ok(DetectionResult {
                    tool: self.name(),
                    detected: false,
                    confidence: Confidence::Medium,
                    markers_found: vec![],
                    version_hint: None,
                })
            }
        }
        let adapter = MinimalAdapter;
        let agents: Vec<aisync_types::AgentFile> = vec![];
        let result = adapter.plan_agents_sync(Path::new("/tmp"), &agents).unwrap();
        assert!(result.is_empty(), "default plan_agents_sync should return empty vec");
    }
}
