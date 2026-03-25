use std::path::Path;

use crate::error::{AisyncError, ConfigError};
use crate::types::PluginsConfig;

/// Wrapper for the TOML file structure: `[plugins.<name>]` sections.
#[derive(Debug, serde::Deserialize)]
struct PluginsFile {
    #[serde(default)]
    plugins: PluginsConfig,
}

/// Engine for loading plugin configuration from `.ai/plugins.toml`.
pub struct PluginEngine;

impl PluginEngine {
    /// Load plugin configuration from `.ai/plugins.toml`.
    ///
    /// Returns an empty map if the file doesn't exist.
    /// Returns an error for invalid TOML.
    pub fn load(project_root: &Path) -> Result<PluginsConfig, AisyncError> {
        let config_path = project_root.join(".ai/plugins.toml");
        if !config_path.exists() {
            return Ok(PluginsConfig::new());
        }

        let raw = std::fs::read_to_string(&config_path)
            .map_err(|e| AisyncError::Config(ConfigError::ReadFile(e)))?;

        let file: PluginsFile =
            toml::from_str(&raw).map_err(|e| AisyncError::Config(ConfigError::Parse(e)))?;

        Ok(file.plugins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{PluginRef, PluginSource};
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_load_returns_empty_when_no_file() {
        let dir = TempDir::new().unwrap();
        let config = PluginEngine::load(dir.path()).unwrap();
        assert!(config.is_empty());
    }

    #[test]
    fn test_load_returns_empty_when_no_plugins_section() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("plugins.toml"), "# empty file\n").unwrap();

        let config = PluginEngine::load(dir.path()).unwrap();
        assert!(config.is_empty());
    }

    #[test]
    fn test_load_parses_github_source() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(
            ai_dir.join("plugins.toml"),
            r#"
[plugins.aisync]
source = "github:whiskeyhouse/agentsync"
description = "Sync AI tool configs"
"#,
        )
        .unwrap();

        let config = PluginEngine::load(dir.path()).unwrap();
        assert_eq!(config.len(), 1);
        let plugin = config.get("aisync").unwrap();
        assert_eq!(
            plugin.source,
            PluginSource::GitHub {
                owner: "whiskeyhouse".to_string(),
                repo: "agentsync".to_string(),
            }
        );
        assert_eq!(plugin.description, Some("Sync AI tool configs".to_string()));
    }

    #[test]
    fn test_load_parses_npm_source() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(
            ai_dir.join("plugins.toml"),
            r#"
[plugins.my-plugin]
source = "npm:@scope/my-plugin"
"#,
        )
        .unwrap();

        let config = PluginEngine::load(dir.path()).unwrap();
        assert_eq!(config.len(), 1);
        let plugin = config.get("my-plugin").unwrap();
        assert_eq!(
            plugin.source,
            PluginSource::Npm {
                package: "@scope/my-plugin".to_string(),
            }
        );
        assert_eq!(plugin.description, None);
    }

    #[test]
    fn test_load_parses_path_source() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(
            ai_dir.join("plugins.toml"),
            r#"
[plugins.local]
source = "path:./my-local-plugin"
description = "A local plugin"
"#,
        )
        .unwrap();

        let config = PluginEngine::load(dir.path()).unwrap();
        assert_eq!(config.len(), 1);
        let plugin = config.get("local").unwrap();
        assert_eq!(
            plugin.source,
            PluginSource::Path {
                path: PathBuf::from("./my-local-plugin"),
            }
        );
        assert_eq!(plugin.description, Some("A local plugin".to_string()));
    }

    #[test]
    fn test_load_parses_multiple_plugins() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(
            ai_dir.join("plugins.toml"),
            r#"
[plugins.alpha]
source = "github:org/alpha"

[plugins.beta]
source = "npm:beta-plugin"

[plugins.gamma]
source = "path:../gamma"
"#,
        )
        .unwrap();

        let config = PluginEngine::load(dir.path()).unwrap();
        assert_eq!(config.len(), 3);
        assert!(config.contains_key("alpha"));
        assert!(config.contains_key("beta"));
        assert!(config.contains_key("gamma"));
    }

    #[test]
    fn test_load_returns_error_for_invalid_toml() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("plugins.toml"), "this is not valid toml [[[").unwrap();

        let result = PluginEngine::load(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_returns_error_for_invalid_source_format() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(
            ai_dir.join("plugins.toml"),
            r#"
[plugins.bad]
source = "unknown:something"
"#,
        )
        .unwrap();

        let result = PluginEngine::load(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_source_display_roundtrip() {
        let sources = vec![
            PluginSource::GitHub {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
            },
            PluginSource::Npm {
                package: "@scope/pkg".to_string(),
            },
            PluginSource::Path {
                path: PathBuf::from("./local"),
            },
        ];

        for source in &sources {
            let serialized = toml::to_string(&PluginRef {
                source: source.clone(),
                description: None,
            })
            .unwrap();
            let back: PluginRef = toml::from_str(&serialized).unwrap();
            assert_eq!(&back.source, source);
        }
    }

    #[test]
    fn test_plugin_ref_roundtrip_with_description() {
        let plugin = PluginRef {
            source: PluginSource::GitHub {
                owner: "org".to_string(),
                repo: "repo".to_string(),
            },
            description: Some("A plugin".to_string()),
        };

        let toml_str = toml::to_string(&plugin).unwrap();
        let back: PluginRef = toml::from_str(&toml_str).unwrap();
        assert_eq!(back, plugin);
    }

    #[test]
    fn test_plugin_source_github_invalid_format() {
        // Missing repo part
        let result: Result<PluginSource, _> =
            toml::from_str::<PluginRef>("source = \"github:owner-only\"")
                .map(|r| r.source);
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_source_npm_empty_package() {
        let result: Result<PluginSource, _> =
            toml::from_str::<PluginRef>("source = \"npm:\"")
                .map(|r| r.source);
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_source_serde_json_roundtrip() {
        let plugin = PluginRef {
            source: PluginSource::GitHub {
                owner: "org".to_string(),
                repo: "repo".to_string(),
            },
            description: Some("desc".to_string()),
        };
        let json = serde_json::to_string(&plugin).unwrap();
        let back: PluginRef = serde_json::from_str(&json).unwrap();
        assert_eq!(back, plugin);
    }
}
