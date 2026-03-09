use thiserror::Error;

/// Top-level error type for aisync operations.
#[derive(Debug, Error)]
pub enum AisyncError {
    #[error("configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("detection error: {0}")]
    Detection(#[from] DetectionError),

    #[error("adapter error for {tool}: {source}")]
    Adapter {
        tool: String,
        source: aisync_adapter::AdapterError,
    },

    #[error("sync error: {0}")]
    Sync(#[from] SyncError),

    #[error("init error: {0}")]
    Init(#[from] InitError),

    #[error("memory error: {0}")]
    Memory(#[from] MemoryError),

    #[error("hook error: {0}")]
    Hook(#[from] HookError),

    #[error("watch error: {0}")]
    Watch(#[from] WatchError),
}

/// Errors related to configuration parsing and validation.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    ReadFile(#[from] std::io::Error),

    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("unsupported schema version {version}, expected {expected}")]
    UnsupportedVersion { version: u32, expected: u32 },
}

/// Errors related to tool detection.
#[derive(Debug, Error)]
pub enum DetectionError {
    #[error("scan failed for path {path}: {source}")]
    ScanFailed {
        path: String,
        source: std::io::Error,
    },
}

/// Re-export AdapterError from aisync-adapter for backward compatibility.
pub use aisync_adapter::AdapterError;

/// Errors related to sync operations.
#[derive(Debug, Error)]
pub enum SyncError {
    #[error("sync failed for {tool}: {reason}")]
    ToolSyncFailed { tool: String, reason: String },

    #[error("symlink creation failed: {0}")]
    SymlinkFailed(#[source] std::io::Error),

    #[error("file write failed: {0}")]
    WriteFailed(#[source] std::io::Error),

    #[error("canonical file not found: {path}")]
    CanonicalMissing { path: String },

    #[error("gitignore update failed: {0}")]
    GitignoreFailed(#[source] std::io::Error),
}

/// Errors related to project initialization.
#[derive(Debug, Error)]
pub enum InitError {
    #[error("scaffold failed: {0}")]
    ScaffoldFailed(#[source] std::io::Error),

    #[error("import failed: {0}")]
    ImportFailed(String),

    #[error("already initialized, use --force to re-initialize")]
    AlreadyInitialized,
}

/// Errors related to memory operations.
#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("memory directory not found: {path}")]
    DirectoryNotFound { path: String },

    #[error("memory file already exists: {path}")]
    AlreadyExists { path: String },

    #[error("failed to read memory: {0}")]
    ReadFailed(#[source] std::io::Error),

    #[error("failed to write memory: {0}")]
    WriteFailed(#[source] std::io::Error),

    #[error("path resolution failed: {0}")]
    PathResolution(#[source] std::io::Error),

    #[error("claude memory path not found: {path}")]
    ClaudeMemoryNotFound { path: String },
}

/// Errors related to watch mode operations.
#[derive(Debug, Error)]
pub enum WatchError {
    #[error("watch failed: {0}")]
    WatchFailed(String),
    #[error("reverse sync failed: {0}")]
    ReverseSyncFailed(String),
}

/// Errors related to hook operations.
#[derive(Debug, Error)]
pub enum HookError {
    #[error("hooks file not found: {path}")]
    FileNotFound { path: String },

    #[error("failed to parse hooks: {0}")]
    ParseFailed(#[from] toml::de::Error),

    #[error("failed to write hooks: {0}")]
    WriteFailed(#[source] std::io::Error),

    #[error("invalid event name: {name}")]
    InvalidEvent { name: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let config_err: ConfigError = io_err.into();
        assert!(matches!(config_err, ConfigError::ReadFile(_)));
    }

    #[test]
    fn test_config_error_unsupported_version() {
        let err = ConfigError::UnsupportedVersion {
            version: 2,
            expected: 1,
        };
        let msg = format!("{err}");
        assert!(msg.contains("unsupported schema version 2"));
        assert!(msg.contains("expected 1"));
    }

    #[test]
    fn test_aisync_error_from_config_error() {
        let config_err = ConfigError::UnsupportedVersion {
            version: 99,
            expected: 1,
        };
        let aisync_err: AisyncError = config_err.into();
        assert!(matches!(aisync_err, AisyncError::Config(_)));
    }

    #[test]
    fn test_aisync_error_from_detection_error() {
        let det_err = DetectionError::ScanFailed {
            path: "/tmp".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        };
        let aisync_err: AisyncError = det_err.into();
        assert!(matches!(aisync_err, AisyncError::Detection(_)));
    }

    #[test]
    fn test_detection_error_display() {
        let err = DetectionError::ScanFailed {
            path: "/some/path".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        };
        let msg = format!("{err}");
        assert!(msg.contains("/some/path"));
    }

    #[test]
    fn test_adapter_error_display() {
        let err = AdapterError::DetectionFailed("no markers".to_string());
        let msg = format!("{err}");
        assert!(msg.contains("no markers"));
    }

    #[test]
    fn test_all_errors_implement_error_trait() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<AisyncError>();
        assert_error::<ConfigError>();
        assert_error::<DetectionError>();
        assert_error::<AdapterError>();
    }
}
