use std::path::Path;

use crate::error::{AisyncError, HookError};
use crate::types::{HookGroup, HookHandler, HooksConfig};

/// Valid event names for hooks.
pub const VALID_EVENTS: &[&str] = &[
    "PreToolUse",
    "PostToolUse",
    "Notification",
    "Stop",
    "SubagentStop",
];

/// Summary of a single hook for display purposes.
#[derive(Debug, Clone)]
pub struct HookSummary {
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
    pub timeout: Option<u64>,
}

/// Engine for parsing, validating, and managing hooks.
pub struct HookEngine;

impl HookEngine {
    /// Parse .ai/hooks.toml into HooksConfig.
    pub fn parse(project_root: &Path) -> Result<HooksConfig, AisyncError> {
        let path = project_root.join(".ai/hooks.toml");
        if !path.exists() {
            return Err(HookError::FileNotFound {
                path: path.display().to_string(),
            }
            .into());
        }
        let content = std::fs::read_to_string(&path).map_err(|e| {
            HookError::WriteFailed(e)
        })?;
        let config: HooksConfig = toml::from_str(&content).map_err(HookError::ParseFailed)?;
        Ok(config)
    }

    /// Validate all event names in a HooksConfig against VALID_EVENTS.
    pub fn validate(config: &HooksConfig) -> Result<(), AisyncError> {
        for event_name in config.events.keys() {
            if !VALID_EVENTS.contains(&event_name.as_str()) {
                return Err(HookError::InvalidEvent {
                    name: event_name.clone(),
                }
                .into());
            }
        }
        Ok(())
    }

    /// Flatten hooks into a list of summaries for display.
    pub fn list_hooks(config: &HooksConfig) -> Vec<HookSummary> {
        let mut summaries = Vec::new();
        for (event, groups) in &config.events {
            for group in groups {
                for hook in &group.hooks {
                    summaries.push(HookSummary {
                        event: event.clone(),
                        matcher: group.matcher.clone(),
                        command: hook.command.clone(),
                        timeout: hook.timeout,
                    });
                }
            }
        }
        summaries
    }

    /// Add a new hook to .ai/hooks.toml. Creates the file if it doesn't exist.
    pub fn add_hook(
        project_root: &Path,
        event: &str,
        matcher: Option<&str>,
        command: &str,
        timeout: Option<u64>,
    ) -> Result<(), AisyncError> {
        let path = project_root.join(".ai/hooks.toml");

        let mut config = if path.exists() {
            let content = std::fs::read_to_string(&path).map_err(HookError::WriteFailed)?;
            toml::from_str::<HooksConfig>(&content).map_err(HookError::ParseFailed)?
        } else {
            HooksConfig {
                events: std::collections::BTreeMap::new(),
            }
        };

        let new_group = HookGroup {
            matcher: matcher.map(|s| s.to_string()),
            hooks: vec![HookHandler {
                hook_type: "command".to_string(),
                command: command.to_string(),
                timeout,
            }],
        };

        config
            .events
            .entry(event.to_string())
            .or_default()
            .push(new_group);

        let toml_str = Self::serialize(&config)?;
        std::fs::write(&path, toml_str).map_err(HookError::WriteFailed)?;
        Ok(())
    }

    /// Serialize HooksConfig back to TOML string.
    pub fn serialize(config: &HooksConfig) -> Result<String, AisyncError> {
        let toml_str = toml::to_string_pretty(config).map_err(|e| {
            HookError::WriteFailed(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        })?;
        Ok(toml_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tempfile::TempDir;

    fn sample_toml() -> &'static str {
        r#"[[PreToolUse]]
matcher = "Edit"

[[PreToolUse.hooks]]
type = "command"
command = "npm run lint"
timeout = 10000

[[PostToolUse]]

[[PostToolUse.hooks]]
type = "command"
command = "cargo fmt"
"#
    }

    fn sample_config() -> HooksConfig {
        let mut events = BTreeMap::new();
        events.insert(
            "PreToolUse".to_string(),
            vec![HookGroup {
                matcher: Some("Edit".to_string()),
                hooks: vec![HookHandler {
                    hook_type: "command".to_string(),
                    command: "npm run lint".to_string(),
                    timeout: Some(10000),
                }],
            }],
        );
        events.insert(
            "PostToolUse".to_string(),
            vec![HookGroup {
                matcher: None,
                hooks: vec![HookHandler {
                    hook_type: "command".to_string(),
                    command: "cargo fmt".to_string(),
                    timeout: None,
                }],
            }],
        );
        HooksConfig { events }
    }

    #[test]
    fn test_hooks_parse_valid_toml() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("hooks.toml"), sample_toml()).unwrap();

        let config = HookEngine::parse(dir.path()).unwrap();
        assert!(config.events.contains_key("PreToolUse"));
        assert!(config.events.contains_key("PostToolUse"));
        let pre = &config.events["PreToolUse"];
        assert_eq!(pre.len(), 1);
        assert_eq!(pre[0].matcher, Some("Edit".to_string()));
        assert_eq!(pre[0].hooks[0].command, "npm run lint");
        assert_eq!(pre[0].hooks[0].timeout, Some(10000));
    }

    #[test]
    fn test_hooks_parse_file_not_found() {
        let dir = TempDir::new().unwrap();
        let result = HookEngine::parse(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("not found"), "expected FileNotFound, got: {msg}");
    }

    #[test]
    fn test_hooks_parse_invalid_toml() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("hooks.toml"), "invalid { toml [[[").unwrap();

        let result = HookEngine::parse(dir.path());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("parse"), "expected ParseFailed, got: {msg}");
    }

    #[test]
    fn test_hooks_validate_valid_events() {
        let config = sample_config();
        assert!(HookEngine::validate(&config).is_ok());
    }

    #[test]
    fn test_hooks_validate_invalid_event() {
        let mut events = BTreeMap::new();
        events.insert(
            "InvalidEvent".to_string(),
            vec![HookGroup {
                matcher: None,
                hooks: vec![HookHandler {
                    hook_type: "command".to_string(),
                    command: "echo hi".to_string(),
                    timeout: None,
                }],
            }],
        );
        let config = HooksConfig { events };
        let result = HookEngine::validate(&config);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("InvalidEvent"), "expected InvalidEvent in error: {msg}");
    }

    #[test]
    fn test_hooks_list_hooks() {
        let config = sample_config();
        let summaries = HookEngine::list_hooks(&config);
        assert_eq!(summaries.len(), 2);
        // BTreeMap ordering: PostToolUse before PreToolUse
        assert_eq!(summaries[0].event, "PostToolUse");
        assert_eq!(summaries[0].command, "cargo fmt");
        assert_eq!(summaries[0].matcher, None);
        assert_eq!(summaries[1].event, "PreToolUse");
        assert_eq!(summaries[1].command, "npm run lint");
        assert_eq!(summaries[1].matcher, Some("Edit".to_string()));
        assert_eq!(summaries[1].timeout, Some(10000));
    }

    #[test]
    fn test_hooks_add_hook_creates_file() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();

        HookEngine::add_hook(
            dir.path(),
            "PreToolUse",
            Some("Edit"),
            "npm run lint",
            Some(5000),
        )
        .unwrap();

        // Should have created hooks.toml
        assert!(ai_dir.join("hooks.toml").exists());
        let config = HookEngine::parse(dir.path()).unwrap();
        assert!(config.events.contains_key("PreToolUse"));
    }

    #[test]
    fn test_hooks_add_hook_appends_to_existing() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("hooks.toml"), sample_toml()).unwrap();

        HookEngine::add_hook(
            dir.path(),
            "Stop",
            None,
            "echo done",
            None,
        )
        .unwrap();

        let config = HookEngine::parse(dir.path()).unwrap();
        assert!(config.events.contains_key("Stop"));
        // Original events still present
        assert!(config.events.contains_key("PreToolUse"));
        assert!(config.events.contains_key("PostToolUse"));
    }

    #[test]
    fn test_hooks_round_trip() {
        let config = sample_config();
        let toml_str = HookEngine::serialize(&config).unwrap();
        let reparsed: HooksConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(reparsed.events.len(), config.events.len());
        for (event, groups) in &config.events {
            let reparsed_groups = &reparsed.events[event];
            assert_eq!(groups.len(), reparsed_groups.len());
            for (g, rg) in groups.iter().zip(reparsed_groups.iter()) {
                assert_eq!(g.matcher, rg.matcher);
                assert_eq!(g.hooks.len(), rg.hooks.len());
                for (h, rh) in g.hooks.iter().zip(rg.hooks.iter()) {
                    assert_eq!(h.hook_type, rh.hook_type);
                    assert_eq!(h.command, rh.command);
                    assert_eq!(h.timeout, rh.timeout);
                }
            }
        }
    }
}
