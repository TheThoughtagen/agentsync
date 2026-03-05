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
        source: AdapterError,
    },
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

/// Errors specific to individual tool adapters.
#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("detection failed: {0}")]
    DetectionFailed(String),
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
