pub mod adapter;
pub mod adapters;
pub mod config;
pub mod detection;
pub mod error;
pub mod gitignore;
pub mod hooks;
pub mod init;
pub mod managed_section;
pub mod memory;
pub mod sync;
pub mod types;

pub use adapter::{
    AnyAdapter, ClaudeCodeAdapter, CursorAdapter, DetectionResult, OpenCodeAdapter, ToolAdapter,
};
pub use config::{AisyncConfig, DefaultsConfig, SyncStrategy, ToolConfig, ToolsConfig};
pub use detection::DetectionEngine;
pub use error::{
    AdapterError, AisyncError, ConfigError, DetectionError, HookError, InitError, MemoryError,
    SyncError,
};
pub use gitignore::update_managed_section;
pub use init::{ImportChoice, ImportSource, InitEngine, InitOptions};
pub use sync::SyncEngine;
pub use memory::{ImportResult, MemoryEngine};
pub use hooks::{HookEngine, HookSummary, VALID_EVENTS};
pub use types::{
    content_hash, Confidence, DriftState, HookGroup, HookHandler, HookStatusReport,
    HookTranslation, HooksConfig, MemoryStatusReport, StatusReport, SyncAction, SyncReport,
    ToolHookStatus, ToolKind, ToolMemoryStatus, ToolSyncResult, ToolSyncStatus,
};
