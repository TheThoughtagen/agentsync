use std::path::Path;

use crate::error::{AisyncError, SyncError};
use crate::types::{RuleFile, RuleMetadata};

/// Engine for loading and parsing canonical rule files from `.ai/rules/`.
pub struct RuleEngine;

impl RuleEngine {
    /// Load all canonical rule files from `.ai/rules/*.md`.
    ///
    /// Returns an empty Vec if the directory doesn't exist or contains no `.md` files.
    /// Results are sorted by name for deterministic ordering.
    pub fn load(project_root: &Path) -> Result<Vec<RuleFile>, AisyncError> {
        let rules_dir = project_root.join(".ai/rules");
        if !rules_dir.is_dir() {
            return Ok(vec![]);
        }

        let mut rules = Vec::new();
        let entries = std::fs::read_dir(&rules_dir)
            .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                let raw = std::fs::read_to_string(&path)
                    .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                let (metadata, content) = Self::parse_frontmatter(&raw)?;
                let name = path.file_stem().unwrap().to_string_lossy().to_string();
                rules.push(RuleFile {
                    name,
                    metadata,
                    content,
                    source_path: path,
                });
            }
        }

        rules.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(rules)
    }

    /// Parse YAML frontmatter from a rule file.
    ///
    /// Returns (metadata, body_content). If no frontmatter is present,
    /// returns default metadata (always_apply=true) with the entire file as content.
    fn parse_frontmatter(raw: &str) -> Result<(RuleMetadata, String), AisyncError> {
        // Opening --- must be the very first characters in the file
        if let Some(after_open) = raw.strip_prefix("---\n") {
            // Handle empty frontmatter (closing --- immediately follows opening)
            if let Some(rest) = after_open.strip_prefix("---") {
                let body = rest.trim_start_matches('\n').to_string();
                let metadata = Self::parse_yaml_metadata("")?;
                return Ok((metadata, body));
            }
            // Find closing --- at start of a line (not inside YAML values)
            if let Some(end_idx) = after_open.find("\n---") {
                let yaml_str = &after_open[..end_idx];
                let metadata = Self::parse_yaml_metadata(yaml_str)?;
                // Body is everything after the closing ---
                let after_close = &after_open[end_idx + 4..]; // skip "\n---"
                let body = after_close.trim_start_matches('\n').to_string();
                return Ok((metadata, body));
            }
        }

        // No frontmatter -- treat entire file as content with default metadata
        Ok((
            RuleMetadata {
                description: None,
                globs: vec![],
                always_apply: true,
            },
            raw.to_string(),
        ))
    }

    /// Hand-parse YAML metadata fields from a frontmatter string.
    fn parse_yaml_metadata(yaml_str: &str) -> Result<RuleMetadata, AisyncError> {
        let mut description = None;
        let mut globs = Vec::new();
        let mut always_apply = true; // default

        for line in yaml_str.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("description:") {
                let val = val.trim();
                // Strip surrounding quotes if present
                let val = val
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .unwrap_or(val);
                if !val.is_empty() {
                    description = Some(val.to_string());
                }
            } else if let Some(val) = line.strip_prefix("globs:") {
                let val = val.trim();
                if val.starts_with('[') {
                    // Array syntax: [*.rs, *.toml]
                    let inner = val.trim_start_matches('[').trim_end_matches(']');
                    globs = inner
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                } else {
                    // Comma-separated string (possibly quoted): "*.rs, *.toml"
                    let val = val
                        .strip_prefix('"')
                        .and_then(|v| v.strip_suffix('"'))
                        .unwrap_or(val);
                    globs = val
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            } else if let Some(val) = line.strip_prefix("always_apply:") {
                always_apply = val.trim().parse().unwrap_or(true);
            }
        }

        Ok(RuleMetadata {
            description,
            globs,
            always_apply,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- RuleEngine::load() tests ---

    #[test]
    fn test_load_returns_empty_when_no_rules_dir() {
        let dir = TempDir::new().unwrap();
        let rules = RuleEngine::load(dir.path()).unwrap();
        assert!(rules.is_empty(), "should return empty vec when .ai/rules/ doesn't exist");
    }

    #[test]
    fn test_load_returns_empty_when_no_md_files() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".ai/rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::write(rules_dir.join("readme.txt"), "not a rule").unwrap();

        let rules = RuleEngine::load(dir.path()).unwrap();
        assert!(rules.is_empty(), "should return empty vec when no .md files");
    }

    #[test]
    fn test_load_reads_md_files_sorted_by_name() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".ai/rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        std::fs::write(
            rules_dir.join("zz-last.md"),
            "---\ndescription: Last rule\n---\nLast content",
        )
        .unwrap();
        std::fs::write(
            rules_dir.join("aa-first.md"),
            "---\ndescription: First rule\n---\nFirst content",
        )
        .unwrap();

        let rules = RuleEngine::load(dir.path()).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].name, "aa-first");
        assert_eq!(rules[1].name, "zz-last");
    }

    #[test]
    fn test_load_parses_frontmatter_correctly() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".ai/rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        std::fs::write(
            rules_dir.join("my-rule.md"),
            "---\ndescription: A test rule\nglobs: [*.rs, *.toml]\nalways_apply: false\n---\n# Rule Body\n\nSome content here.",
        )
        .unwrap();

        let rules = RuleEngine::load(dir.path()).unwrap();
        assert_eq!(rules.len(), 1);
        let rule = &rules[0];
        assert_eq!(rule.name, "my-rule");
        assert_eq!(rule.metadata.description, Some("A test rule".to_string()));
        assert_eq!(rule.metadata.globs, vec!["*.rs".to_string(), "*.toml".to_string()]);
        assert!(!rule.metadata.always_apply);
        assert_eq!(rule.content, "# Rule Body\n\nSome content here.");
    }

    // --- parse_frontmatter() tests ---

    #[test]
    fn test_parse_frontmatter_extracts_yaml() {
        let raw = "---\ndescription: My rule\nglobs: [*.rs]\nalways_apply: true\n---\nBody text here";
        let (meta, body) = RuleEngine::parse_frontmatter(raw).unwrap();
        assert_eq!(meta.description, Some("My rule".to_string()));
        assert_eq!(meta.globs, vec!["*.rs".to_string()]);
        assert!(meta.always_apply);
        assert_eq!(body, "Body text here");
    }

    #[test]
    fn test_parse_frontmatter_handles_missing_frontmatter() {
        let raw = "Just some content\nwithout frontmatter";
        let (meta, body) = RuleEngine::parse_frontmatter(raw).unwrap();
        assert_eq!(meta.description, None);
        assert!(meta.globs.is_empty());
        assert!(meta.always_apply); // default
        assert_eq!(body, raw);
    }

    #[test]
    fn test_parse_frontmatter_handles_empty_frontmatter() {
        let raw = "---\n---\nBody only";
        let (meta, body) = RuleEngine::parse_frontmatter(raw).unwrap();
        assert_eq!(meta.description, None);
        assert!(meta.globs.is_empty());
        assert!(meta.always_apply);
        assert_eq!(body, "Body only");
    }

    #[test]
    fn test_parse_frontmatter_comma_separated_globs() {
        let raw = "---\nglobs: \"*.rs, *.toml\"\n---\nContent";
        let (meta, _body) = RuleEngine::parse_frontmatter(raw).unwrap();
        assert_eq!(meta.globs, vec!["*.rs".to_string(), "*.toml".to_string()]);
    }

    #[test]
    fn test_parse_frontmatter_array_globs() {
        let raw = "---\nglobs: [*.rs, *.toml]\n---\nContent";
        let (meta, _body) = RuleEngine::parse_frontmatter(raw).unwrap();
        assert_eq!(meta.globs, vec!["*.rs".to_string(), "*.toml".to_string()]);
    }

    #[test]
    fn test_parse_frontmatter_quoted_description() {
        let raw = "---\ndescription: \"A quoted description\"\n---\nContent";
        let (meta, _body) = RuleEngine::parse_frontmatter(raw).unwrap();
        assert_eq!(meta.description, Some("A quoted description".to_string()));
    }

    #[test]
    fn test_parse_frontmatter_unquoted_description() {
        let raw = "---\ndescription: An unquoted description\n---\nContent";
        let (meta, _body) = RuleEngine::parse_frontmatter(raw).unwrap();
        assert_eq!(meta.description, Some("An unquoted description".to_string()));
    }

    // --- parse_yaml_metadata() tests ---

    #[test]
    fn test_parse_yaml_metadata_all_fields() {
        let yaml = "description: Test\nglobs: [*.rs]\nalways_apply: false";
        let meta = RuleEngine::parse_yaml_metadata(yaml).unwrap();
        assert_eq!(meta.description, Some("Test".to_string()));
        assert_eq!(meta.globs, vec!["*.rs".to_string()]);
        assert!(!meta.always_apply);
    }

    #[test]
    fn test_parse_yaml_metadata_defaults() {
        let yaml = "";
        let meta = RuleEngine::parse_yaml_metadata(yaml).unwrap();
        assert_eq!(meta.description, None);
        assert!(meta.globs.is_empty());
        assert!(meta.always_apply); // default true
    }

    #[test]
    fn test_parse_yaml_metadata_only_description() {
        let yaml = "description: Just a description";
        let meta = RuleEngine::parse_yaml_metadata(yaml).unwrap();
        assert_eq!(meta.description, Some("Just a description".to_string()));
        assert!(meta.globs.is_empty());
        assert!(meta.always_apply);
    }
}
