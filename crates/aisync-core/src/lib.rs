pub mod adapter;
pub mod adapters;
pub mod config;
pub mod detection;
pub mod error;
pub mod types;

pub use adapter::{
    AnyAdapter, ClaudeCodeAdapter, CursorAdapter, DetectionResult, OpenCodeAdapter, ToolAdapter,
};
pub use config::{AisyncConfig, DefaultsConfig, SyncStrategy, ToolConfig, ToolsConfig};
pub use detection::DetectionEngine;
pub use error::{AdapterError, AisyncError, ConfigError, DetectionError};
pub use types::{Confidence, ToolKind};
