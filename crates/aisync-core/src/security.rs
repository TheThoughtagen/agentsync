use std::sync::LazyLock;

use regex::Regex;

use crate::types::McpConfig;

/// A warning about a potential secret found in MCP server environment variables.
#[derive(Debug, Clone)]
pub struct SecurityWarning {
    pub server_name: String,
    pub env_key: String,
    pub pattern_name: String,
}

/// Named regex pattern for detecting secrets.
struct SecretPattern {
    name: &'static str,
    regex: Regex,
}

static SECRET_PATTERNS: LazyLock<Vec<SecretPattern>> = LazyLock::new(|| {
    vec![
        SecretPattern {
            name: "AWS Access Key",
            regex: Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
        },
        SecretPattern {
            name: "GitHub Token",
            regex: Regex::new(r"gh[ps]_[A-Za-z0-9_]{36,}").unwrap(),
        },
        SecretPattern {
            name: "GitHub Fine-grained Token",
            regex: Regex::new(r"github_pat_[A-Za-z0-9_]{22,}").unwrap(),
        },
        SecretPattern {
            name: "Slack Token",
            regex: Regex::new(r"xox[bpors]-[0-9A-Za-z-]+").unwrap(),
        },
        SecretPattern {
            name: "Anthropic API Key",
            regex: Regex::new(r"sk-ant-api\d+-[A-Za-z0-9_-]+").unwrap(),
        },
        SecretPattern {
            name: "OpenAI API Key",
            regex: Regex::new(r"sk-[A-Za-z0-9]{48,}").unwrap(),
        },
    ]
});

/// Scans MCP configurations for hardcoded secrets in environment variables.
pub struct SecurityScanner;

impl SecurityScanner {
    /// Returns true if the value matches a known secret pattern and is NOT
    /// already a `${...}` variable reference.
    pub fn looks_like_secret(value: &str) -> bool {
        // Skip ${VAR} references -- already sanitized
        if value.starts_with("${") && value.ends_with('}') {
            return false;
        }

        SECRET_PATTERNS.iter().any(|p| p.regex.is_match(value))
    }

    /// Scan all env values across all MCP servers, returning warnings for
    /// values that look like hardcoded secrets.
    pub fn scan_mcp_config(config: &McpConfig) -> Vec<SecurityWarning> {
        let mut warnings = Vec::new();
        for (server_name, server) in &config.servers {
            for (env_key, env_value) in &server.env {
                for pattern in SECRET_PATTERNS.iter() {
                    if pattern.regex.is_match(env_value)
                        && !(env_value.starts_with("${") && env_value.ends_with('}'))
                    {
                        warnings.push(SecurityWarning {
                            server_name: server_name.clone(),
                            env_key: env_key.clone(),
                            pattern_name: pattern.name.to_string(),
                        });
                        break; // one warning per env key
                    }
                }
            }
        }
        warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use crate::types::McpServer;

    // --- looks_like_secret tests ---

    #[test]
    fn test_looks_like_secret_aws_key() {
        assert!(SecurityScanner::looks_like_secret("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn test_looks_like_secret_github_token() {
        let token = format!("ghp_{}", "a".repeat(36));
        assert!(SecurityScanner::looks_like_secret(&token));
    }

    #[test]
    fn test_looks_like_secret_github_fine_grained() {
        let token = format!("github_pat_{}", "a".repeat(22));
        assert!(SecurityScanner::looks_like_secret(&token));
    }

    #[test]
    fn test_looks_like_secret_slack_token() {
        assert!(SecurityScanner::looks_like_secret("xoxb-123-456-abcDEF"));
    }

    #[test]
    fn test_looks_like_secret_anthropic_key() {
        assert!(SecurityScanner::looks_like_secret(
            "sk-ant-api03-abcdefghijklmnop"
        ));
    }

    #[test]
    fn test_looks_like_secret_openai_key() {
        let key = format!("sk-{}", "a".repeat(48));
        assert!(SecurityScanner::looks_like_secret(&key));
    }

    #[test]
    fn test_looks_like_secret_var_reference_not_secret() {
        assert!(!SecurityScanner::looks_like_secret("${MY_API_KEY}"));
    }

    #[test]
    fn test_looks_like_secret_normal_value_not_secret() {
        assert!(!SecurityScanner::looks_like_secret("/usr/local/bin"));
        assert!(!SecurityScanner::looks_like_secret("production"));
        assert!(!SecurityScanner::looks_like_secret(""));
    }

    // --- scan_mcp_config tests ---

    #[test]
    fn test_scan_detects_aws_key() {
        let config = McpConfig {
            servers: BTreeMap::from([(
                "my-server".to_string(),
                McpServer {
                    command: "npx".to_string(),
                    args: vec![],
                    env: BTreeMap::from([(
                        "AWS_ACCESS_KEY_ID".to_string(),
                        "AKIAIOSFODNN7EXAMPLE".to_string(),
                    )]),
                },
            )]),
        };
        let warnings = SecurityScanner::scan_mcp_config(&config);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].server_name, "my-server");
        assert_eq!(warnings[0].env_key, "AWS_ACCESS_KEY_ID");
        assert_eq!(warnings[0].pattern_name, "AWS Access Key");
    }

    #[test]
    fn test_scan_detects_github_token() {
        let token = format!("ghp_{}", "x".repeat(36));
        let config = McpConfig {
            servers: BTreeMap::from([(
                "gh-server".to_string(),
                McpServer {
                    command: "node".to_string(),
                    args: vec![],
                    env: BTreeMap::from([("GITHUB_TOKEN".to_string(), token)]),
                },
            )]),
        };
        let warnings = SecurityScanner::scan_mcp_config(&config);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].pattern_name, "GitHub Token");
    }

    #[test]
    fn test_scan_detects_slack_token() {
        let config = McpConfig {
            servers: BTreeMap::from([(
                "slack".to_string(),
                McpServer {
                    command: "node".to_string(),
                    args: vec![],
                    env: BTreeMap::from([(
                        "SLACK_TOKEN".to_string(),
                        "xoxb-123-456-abcdef".to_string(),
                    )]),
                },
            )]),
        };
        let warnings = SecurityScanner::scan_mcp_config(&config);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].pattern_name, "Slack Token");
    }

    #[test]
    fn test_scan_returns_empty_for_clean_config() {
        let config = McpConfig {
            servers: BTreeMap::from([(
                "fs".to_string(),
                McpServer {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "server".to_string()],
                    env: BTreeMap::from([("HOME".to_string(), "/home/user".to_string())]),
                },
            )]),
        };
        let warnings = SecurityScanner::scan_mcp_config(&config);
        assert!(
            warnings.is_empty(),
            "clean config should produce no warnings"
        );
    }

    #[test]
    fn test_scan_skips_var_references() {
        let config = McpConfig {
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
        let warnings = SecurityScanner::scan_mcp_config(&config);
        assert!(warnings.is_empty(), "var references should not trigger warnings");
    }

    #[test]
    fn test_scan_detects_anthropic_key() {
        let config = McpConfig {
            servers: BTreeMap::from([(
                "anthropic".to_string(),
                McpServer {
                    command: "node".to_string(),
                    args: vec![],
                    env: BTreeMap::from([(
                        "ANTHROPIC_API_KEY".to_string(),
                        "sk-ant-api03-abcdefghijklmnop".to_string(),
                    )]),
                },
            )]),
        };
        let warnings = SecurityScanner::scan_mcp_config(&config);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].pattern_name, "Anthropic API Key");
    }
}
