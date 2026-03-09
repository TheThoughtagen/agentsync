pub mod adapter;
pub mod adapters;
pub mod add_tool;
pub mod conditional;
pub mod config;
pub mod declarative;
pub mod detection;
pub mod diff;
pub mod error;
pub mod gitignore;
pub mod hooks;
pub mod init;
pub mod managed_section;
pub mod memory;
pub mod sync;
pub mod types;
pub mod watch;

pub use adapter::{
    AnyAdapter, ClaudeCodeAdapter, CursorAdapter, DetectionResult, OpenCodeAdapter, ToolAdapter,
};
pub use declarative::{DeclarativeAdapter, DeclarativeAdapterDef, discover_toml_adapters};
pub use add_tool::AddToolEngine;
pub use conditional::ConditionalProcessor;
pub use config::{AisyncConfig, DefaultsConfig, SyncStrategy, ToolConfig, ToolsConfig};
pub use detection::DetectionEngine;
pub use diff::DiffEngine;
pub use error::{
    AdapterError, AisyncError, ConfigError, DetectionError, HookError, InitError, MemoryError,
    SyncError, WatchError,
};
pub use gitignore::update_managed_section;
pub use hooks::{HookEngine, HookSummary, VALID_EVENTS};
pub use init::{ImportChoice, ImportSource, InitEngine, InitOptions};
pub use memory::{ImportResult, MemoryEngine};
pub use sync::SyncEngine;
pub use types::{
    Confidence, DriftState, HookGroup, HookHandler, HookStatusReport, HookTranslation, HooksConfig,
    MemoryStatusReport, StatusReport, SyncAction, SyncReport, ToolDiff, ToolHookStatus, ToolKind,
    ToolMemoryStatus, ToolSyncResult, ToolSyncStatus, WatchEvent, content_hash,
};
pub use watch::WatchEngine;
