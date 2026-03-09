use std::path::Path;

use crate::error::{AisyncError, SyncError};
use crate::security::SecurityScanner;
use crate::types::McpConfig;

/// Engine for loading and processing MCP server configuration from `.ai/mcp.toml`.
pub struct McpEngine;

impl McpEngine {
    /// Load MCP configuration from `.ai/mcp.toml`.
    ///
    /// Returns an empty McpConfig if the file doesn't exist.
    /// Returns an error for invalid TOML.
    pub fn load(project_root: &Path) -> Result<McpConfig, AisyncError> {
        let config_path = project_root.join(".ai/mcp.toml");
        if !config_path.exists() {
            return Ok(McpConfig {
                servers: std::collections::BTreeMap::new(),
            });
        }

        let raw = std::fs::read_to_string(&config_path)
            .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;

        let config: McpConfig =
            toml::from_str(&raw).map_err(|e| AisyncError::Config(crate::error::ConfigError::Parse(e)))?;

        Ok(config)
    }

    /// Replace hardcoded secret values in MCP server env with `${KEY_NAME}` references.
    ///
    /// Values that are already `${...}` references are left untouched.
    pub fn sanitize_env(mcp: &mut McpConfig) {
        for server in mcp.servers.values_mut() {
            let keys: Vec<String> = server.env.keys().cloned().collect();
            for key in keys {
                if let Some(value) = server.env.get(&key) {
                    if SecurityScanner::looks_like_secret(value) {
                        server.env.insert(key.clone(), format!("${{{key}}}"));
                    }
                }
            }
        }
    }

    /// Generate `{"mcpServers": {...}}` JSON for the given MCP config.
    ///
    /// For each server:
    /// - Always includes "command"
    /// - Includes "args" only if non-empty
    /// - Includes "env" only if non-empty
    pub fn generate_mcp_json(mcp_config: &McpConfig) -> Result<String, AisyncError> {
        let mut servers = serde_json::Map::new();

        for (name, server) in &mcp_config.servers {
            let mut obj = serde_json::Map::new();
            obj.insert(
                "command".into(),
                serde_json::Value::String(server.command.clone()),
            );
            if !server.args.is_empty() {
                let args: Vec<serde_json::Value> = server
                    .args
                    .iter()
                    .map(|a| serde_json::Value::String(a.clone()))
                    .collect();
                obj.insert("args".into(), serde_json::Value::Array(args));
            }
            if !server.env.is_empty() {
                let env: serde_json::Map<String, serde_json::Value> = server
                    .env
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect();
                obj.insert("env".into(), serde_json::Value::Object(env));
            }
            servers.insert(name.clone(), serde_json::Value::Object(obj));
        }

        let root = serde_json::json!({ "mcpServers": servers });
        serde_json::to_string_pretty(&root).map_err(|e| {
            AisyncError::Sync(SyncError::WriteFailed(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("JSON serialization failed: {e}"),
            )))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::McpServer;
    use std::collections::BTreeMap;
    use tempfile::TempDir;

    // --- McpEngine::load tests ---

    #[test]
    fn test_load_returns_empty_when_no_file() {
        let dir = TempDir::new().unwrap();
        let config = McpEngine::load(dir.path()).unwrap();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn test_load_parses_valid_toml() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(
            ai_dir.join("mcp.toml"),
            r#"
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem"]

[servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_TOKEN = "${GITHUB_TOKEN}" }
"#,
        )
        .unwrap();

        let config = McpEngine::load(dir.path()).unwrap();
        assert_eq!(config.servers.len(), 2);
        assert!(config.servers.contains_key("filesystem"));
        assert!(config.servers.contains_key("github"));
        assert_eq!(config.servers["filesystem"].command, "npx");
        assert_eq!(config.servers["github"].env["GITHUB_TOKEN"], "${GITHUB_TOKEN}");
    }

    #[test]
    fn test_load_returns_error_for_invalid_toml() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("mcp.toml"), "this is not valid toml [[[").unwrap();

        let result = McpEngine::load(dir.path());
        assert!(result.is_err());
    }

    // --- McpEngine::sanitize_env tests ---

    #[test]
    fn test_sanitize_env_replaces_secrets() {
        let mut config = McpConfig {
            servers: BTreeMap::from([(
                "server".to_string(),
                McpServer {
                    command: "npx".to_string(),
                    args: vec![],
                    env: BTreeMap::from([(
                        "AWS_KEY".to_string(),
                        "AKIAIOSFODNN7EXAMPLE".to_string(),
                    )]),
                },
            )]),
        };

        McpEngine::sanitize_env(&mut config);
        assert_eq!(config.servers["server"].env["AWS_KEY"], "${AWS_KEY}");
    }

    #[test]
    fn test_sanitize_env_leaves_var_references_untouched() {
        let mut config = McpConfig {
            servers: BTreeMap::from([(
                "server".to_string(),
                McpServer {
                    command: "npx".to_string(),
                    args: vec![],
                    env: BTreeMap::from([(
                        "API_KEY".to_string(),
                        "${API_KEY}".to_string(),
                    )]),
                },
            )]),
        };

        McpEngine::sanitize_env(&mut config);
        assert_eq!(
            config.servers["server"].env["API_KEY"],
            "${API_KEY}",
            "var references should be left untouched"
        );
    }

    #[test]
    fn test_sanitize_env_leaves_normal_values() {
        let mut config = McpConfig {
            servers: BTreeMap::from([(
                "server".to_string(),
                McpServer {
                    command: "npx".to_string(),
                    args: vec![],
                    env: BTreeMap::from([(
                        "HOME".to_string(),
                        "/home/user".to_string(),
                    )]),
                },
            )]),
        };

        McpEngine::sanitize_env(&mut config);
        assert_eq!(config.servers["server"].env["HOME"], "/home/user");
    }

    // --- McpEngine::generate_mcp_json tests ---

    #[test]
    fn test_generate_mcp_json_full_server() {
        let config = McpConfig {
            servers: BTreeMap::from([(
                "fs".to_string(),
                McpServer {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "server-fs".to_string()],
                    env: BTreeMap::from([("HOME".to_string(), "/home".to_string())]),
                },
            )]),
        };

        let json = McpEngine::generate_mcp_json(&config).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed["mcpServers"]["fs"].is_object());
        assert_eq!(parsed["mcpServers"]["fs"]["command"], "npx");
        assert_eq!(parsed["mcpServers"]["fs"]["args"][0], "-y");
        assert_eq!(parsed["mcpServers"]["fs"]["args"][1], "server-fs");
        assert_eq!(parsed["mcpServers"]["fs"]["env"]["HOME"], "/home");
    }

    #[test]
    fn test_generate_mcp_json_omits_empty_args() {
        let config = McpConfig {
            servers: BTreeMap::from([(
                "simple".to_string(),
                McpServer {
                    command: "node".to_string(),
                    args: vec![],
                    env: BTreeMap::new(),
                },
            )]),
        };

        let json = McpEngine::generate_mcp_json(&config).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["mcpServers"]["simple"]["command"], "node");
        assert!(
            parsed["mcpServers"]["simple"].get("args").is_none(),
            "empty args should be omitted"
        );
        assert!(
            parsed["mcpServers"]["simple"].get("env").is_none(),
            "empty env should be omitted"
        );
    }

    #[test]
    fn test_generate_mcp_json_omits_empty_env() {
        let config = McpConfig {
            servers: BTreeMap::from([(
                "server".to_string(),
                McpServer {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string()],
                    env: BTreeMap::new(),
                },
            )]),
        };

        let json = McpEngine::generate_mcp_json(&config).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed["mcpServers"]["server"].get("args").is_some());
        assert!(
            parsed["mcpServers"]["server"].get("env").is_none(),
            "empty env should be omitted"
        );
    }

    // --- McpEngine::parse_mcp_json tests ---

    #[test]
    fn test_parse_mcp_json_valid_servers() {
        let dir = TempDir::new().unwrap();
        let json = r#"{
            "mcpServers": {
                "filesystem": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-filesystem"],
                    "env": { "HOME": "/home/user" }
                },
                "github": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-github"],
                    "env": { "GITHUB_TOKEN": "ghp_abc123" }
                }
            }
        }"#;
        let path = dir.path().join("mcp.json");
        std::fs::write(&path, json).unwrap();

        let config = McpEngine::parse_mcp_json(&path).unwrap();
        assert_eq!(config.servers.len(), 2);
        assert_eq!(config.servers["filesystem"].command, "npx");
        assert_eq!(config.servers["filesystem"].args, vec!["-y", "@modelcontextprotocol/server-filesystem"]);
        assert_eq!(config.servers["filesystem"].env["HOME"], "/home/user");
        assert_eq!(config.servers["github"].command, "npx");
    }

    #[test]
    fn test_parse_mcp_json_skips_http_servers() {
        let dir = TempDir::new().unwrap();
        let json = r#"{
            "mcpServers": {
                "local-tool": {
                    "command": "npx",
                    "args": ["-y", "my-tool"]
                },
                "remote-sse": {
                    "url": "https://example.com/sse"
                }
            }
        }"#;
        let path = dir.path().join("mcp.json");
        std::fs::write(&path, json).unwrap();

        let config = McpEngine::parse_mcp_json(&path).unwrap();
        assert_eq!(config.servers.len(), 1);
        assert!(config.servers.contains_key("local-tool"));
        assert!(!config.servers.contains_key("remote-sse"));
    }

    #[test]
    fn test_parse_mcp_json_missing_file_returns_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");

        let config = McpEngine::parse_mcp_json(&path).unwrap();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn test_parse_mcp_json_invalid_json_returns_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "not json at all").unwrap();

        let config = McpEngine::parse_mcp_json(&path).unwrap();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn test_parse_mcp_json_server_with_only_command() {
        let dir = TempDir::new().unwrap();
        let json = r#"{
            "mcpServers": {
                "simple": {
                    "command": "my-tool"
                }
            }
        }"#;
        let path = dir.path().join("mcp.json");
        std::fs::write(&path, json).unwrap();

        let config = McpEngine::parse_mcp_json(&path).unwrap();
        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers["simple"].command, "my-tool");
        assert!(config.servers["simple"].args.is_empty());
        assert!(config.servers["simple"].env.is_empty());
    }

    #[test]
    fn test_parse_mcp_json_no_mcp_servers_key() {
        let dir = TempDir::new().unwrap();
        let json = r#"{ "other": "data" }"#;
        let path = dir.path().join("mcp.json");
        std::fs::write(&path, json).unwrap();

        let config = McpEngine::parse_mcp_json(&path).unwrap();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn test_generate_mcp_json_multiple_servers() {
        let config = McpConfig {
            servers: BTreeMap::from([
                (
                    "alpha".to_string(),
                    McpServer {
                        command: "cmd-a".to_string(),
                        args: vec![],
                        env: BTreeMap::new(),
                    },
                ),
                (
                    "beta".to_string(),
                    McpServer {
                        command: "cmd-b".to_string(),
                        args: vec!["--flag".to_string()],
                        env: BTreeMap::new(),
                    },
                ),
            ]),
        };

        let json = McpEngine::generate_mcp_json(&config).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed["mcpServers"]["alpha"].is_object());
        assert!(parsed["mcpServers"]["beta"].is_object());
        assert_eq!(parsed["mcpServers"]["alpha"]["command"], "cmd-a");
        assert_eq!(parsed["mcpServers"]["beta"]["command"], "cmd-b");
    }
}
