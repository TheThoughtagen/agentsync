pub mod adapter;
pub mod adapters;
pub mod config;
pub mod detection;
pub mod error;
pub mod gitignore;
pub mod types;

pub use adapter::{
    AnyAdapter, ClaudeCodeAdapter, CursorAdapter, DetectionResult, OpenCodeAdapter, ToolAdapter,
};
pub use config::{AisyncConfig, DefaultsConfig, SyncStrategy, ToolConfig, ToolsConfig};
pub use detection::DetectionEngine;
pub use error::{AdapterError, AisyncError, ConfigError, DetectionError, InitError, SyncError};
pub use gitignore::update_managed_section;
pub use types::{
    Confidence, DriftState, StatusReport, SyncAction, SyncReport, ToolKind, ToolSyncResult,
    ToolSyncStatus,
};
