use std::path::{Path, PathBuf};

use aisync_adapter::{AdapterError, DetectionResult, ToolAdapter};
use serde::Deserialize;

use crate::types::{
    Confidence, DriftState, HookTranslation, HooksConfig, SyncAction, SyncStrategy, ToolKind,
    ToolSyncStatus, content_hash,
};

// ---------------------------------------------------------------------------
// TOML schema structs
// ---------------------------------------------------------------------------

/// Top-level definition deserialized from a `.toml` adapter file.
#[derive(Debug, Clone, Deserialize)]
pub struct DeclarativeAdapterDef {
    pub name: String,
    pub display_name: String,
    #[serde(default)]
    pub detection: DetectionDef,
    pub sync: SyncDef,
    #[serde(default)]
    pub template: Option<TemplateDef>,
}

/// Detection configuration: which directories/files indicate the tool is present.
#[derive(Debug, Clone, Deserialize)]
pub struct DetectionDef {
    #[serde(default)]
    pub directories: Vec<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default = "default_true")]
    pub match_any: bool,
}

impl Default for DetectionDef {
    fn default() -> Self {
        Self {
            directories: vec![],
            files: vec![],
            match_any: true,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Sync configuration for the adapter.
#[derive(Debug, Clone, Deserialize)]
pub struct SyncDef {
    #[serde(default)]
    pub strategy: SyncStrategy,
    pub instruction_path: String,
    #[serde(default)]
    pub conditional_tags: Vec<String>,
    #[serde(default)]
    pub gitignore_entries: Vec<String>,
    #[serde(default)]
    pub watch_paths: Vec<String>,
}

/// Template for Generate-strategy adapters.
#[derive(Debug, Clone, Deserialize)]
pub struct TemplateDef {
    pub content: String,
    #[serde(default)]
    pub frontmatter_strip: Option<String>,
}

// ---------------------------------------------------------------------------
// Builtin name collision guard
// ---------------------------------------------------------------------------

/// Builtin tool kind names that TOML adapters cannot shadow.
const BUILTIN_NAMES: &[&str] = &["claude-code", "cursor", "opencode", "windsurf", "codex"];

// ---------------------------------------------------------------------------
// DeclarativeAdapter
// ---------------------------------------------------------------------------

/// A tool adapter driven entirely by a parsed TOML definition.
///
/// Constructed once at startup; fields that require `&'static str` lifetimes
/// (for trait compatibility) use `Box::leak` -- acceptable because TOML
/// adapters live for the program's entire lifetime.
#[derive(Debug)]
pub struct DeclarativeAdapter {
    def: DeclarativeAdapterDef,
    /// Pre-computed `&'static str` slice for `conditional_tags() -> &[&str]`.
    cached_tags: &'static [&'static str],
    /// Pre-computed `Vec<&'static str>` for `watch_paths()`.
    cached_watch_paths: Vec<&'static str>,
    /// Owned copy for `native_instruction_path() -> &str`.
    instruction_path: String,
    /// Owned copy for `display_name() -> &str`.
    display_name_owned: String,
}

// Safety: all fields are either owned or 'static references.
unsafe impl Send for DeclarativeAdapter {}
unsafe impl Sync for DeclarativeAdapter {}

impl DeclarativeAdapter {
    /// Create a new declarative adapter from a parsed definition.
    ///
    /// Returns an error if `def.name` matches a builtin tool kind name.
    pub fn new(def: DeclarativeAdapterDef) -> Result<Self, AdapterError> {
        if BUILTIN_NAMES.contains(&def.name.as_str()) {
            return Err(AdapterError::Other(format!(
                "TOML adapter name '{}' collides with builtin tool kind",
                def.name
            )));
        }

        // Leak conditional tags for &'static str lifetime compatibility.
        let leaked_tags: Vec<&'static str> = def
            .sync
            .conditional_tags
            .iter()
            .map(|s| {
                let leaked: &'static str = Box::leak(s.clone().into_boxed_str());
                leaked
            })
            .collect();
        let cached_tags: &'static [&'static str] = Box::leak(leaked_tags.into_boxed_slice());

        // Leak watch_paths (or default to instruction_path).
        let cached_watch_paths = if def.sync.watch_paths.is_empty() {
            let leaked: &'static str =
                Box::leak(def.sync.instruction_path.clone().into_boxed_str());
            vec![leaked]
        } else {
            def.sync
                .watch_paths
                .iter()
                .map(|s| {
                    let leaked: &'static str = Box::leak(s.clone().into_boxed_str());
                    leaked
                })
                .collect()
        };

        let instruction_path = def.sync.instruction_path.clone();
        let display_name_owned = def.display_name.clone();

        Ok(Self {
            def,
            cached_tags,
            cached_watch_paths,
            instruction_path,
            display_name_owned,
        })
    }
}

impl ToolAdapter for DeclarativeAdapter {
    fn name(&self) -> ToolKind {
        ToolKind::Custom(self.def.name.clone())
    }

    fn display_name(&self) -> &str {
        &self.display_name_owned
    }

    fn native_instruction_path(&self) -> &str {
        &self.instruction_path
    }

    fn conditional_tags(&self) -> &[&str] {
        self.cached_tags
    }

    fn gitignore_entries(&self) -> Vec<String> {
        self.def.sync.gitignore_entries.clone()
    }

    fn watch_paths(&self) -> Vec<&str> {
        self.cached_watch_paths.clone()
    }

    fn default_sync_strategy(&self) -> SyncStrategy {
        self.def.sync.strategy
    }

    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AdapterError> {
        let mut markers_found = Vec::new();

        for dir in &self.def.detection.directories {
            let p = project_root.join(dir);
            if p.is_dir() {
                markers_found.push(p);
            }
        }

        for file in &self.def.detection.files {
            let p = project_root.join(file);
            if p.exists() {
                markers_found.push(p);
            }
        }

        let total_markers = self.def.detection.directories.len() + self.def.detection.files.len();
        let detected = if total_markers == 0 {
            false
        } else if self.def.detection.match_any {
            !markers_found.is_empty()
        } else {
            markers_found.len() == total_markers
        };

        Ok(DetectionResult {
            tool: ToolKind::Custom(self.def.name.clone()),
            detected,
            confidence: Confidence::Medium,
            markers_found,
            version_hint: None,
        })
    }

    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AdapterError> {
        let path = project_root.join(&self.instruction_path);
        if !path.exists() {
            return Ok(None);
        }

        let raw = std::fs::read_to_string(&path).map_err(AdapterError::Io)?;

        // For Generate strategy with frontmatter_strip, strip the frontmatter.
        if self.def.sync.strategy == SyncStrategy::Generate {
            if let Some(ref template) = self.def.template {
                if let Some(ref delimiter) = template.frontmatter_strip {
                    return Ok(Some(strip_frontmatter(&raw, delimiter)));
                }
            }
        }

        Ok(Some(raw))
    }

    fn plan_sync(
        &self,
        project_root: &Path,
        canonical_content: &str,
        strategy: SyncStrategy,
    ) -> Result<Vec<SyncAction>, AdapterError> {
        match strategy {
            SyncStrategy::Generate => self.plan_sync_generate(project_root, canonical_content),
            SyncStrategy::Symlink => self.plan_sync_symlink(project_root),
            SyncStrategy::Copy => self.plan_sync_copy(project_root, canonical_content),
        }
    }

    fn sync_status(
        &self,
        project_root: &Path,
        canonical_hash: &str,
        strategy: SyncStrategy,
    ) -> Result<ToolSyncStatus, AdapterError> {
        let path = project_root.join(&self.instruction_path);

        if !path.exists() {
            return Ok(ToolSyncStatus {
                tool: self.name(),
                strategy,
                drift: DriftState::Missing,
                details: None,
            });
        }

        let raw = std::fs::read_to_string(&path).map_err(AdapterError::Io)?;

        // For Generate with frontmatter, strip before hashing.
        let body = if strategy == SyncStrategy::Generate {
            if let Some(ref template) = self.def.template {
                if let Some(ref delimiter) = template.frontmatter_strip {
                    strip_frontmatter(&raw, delimiter)
                } else {
                    raw.clone()
                }
            } else {
                raw.clone()
            }
        } else {
            raw.clone()
        };

        let body_hash = content_hash(body.as_bytes());
        if body_hash == canonical_hash {
            Ok(ToolSyncStatus {
                tool: self.name(),
                strategy,
                drift: DriftState::InSync,
                details: None,
            })
        } else {
            Ok(ToolSyncStatus {
                tool: self.name(),
                strategy,
                drift: DriftState::Drifted {
                    reason: "content hash mismatch".to_string(),
                },
                details: Some(format!(
                    "body hash: {body_hash}, expected: {canonical_hash}"
                )),
            })
        }
    }

    fn plan_memory_sync(
        &self,
        _project_root: &Path,
        _memory_files: &[PathBuf],
    ) -> Result<Vec<SyncAction>, AdapterError> {
        Ok(vec![])
    }

    fn translate_hooks(&self, _hooks: &HooksConfig) -> Result<HookTranslation, AdapterError> {
        Ok(HookTranslation::Unsupported {
            tool: self.name(),
            reason: format!(
                "{} does not support hooks (TOML adapter)",
                self.display_name_owned
            ),
        })
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

impl DeclarativeAdapter {
    fn plan_sync_generate(
        &self,
        project_root: &Path,
        canonical_content: &str,
    ) -> Result<Vec<SyncAction>, AdapterError> {
        let template = self
            .def
            .template
            .as_ref()
            .map(|t| t.content.as_str())
            .unwrap_or("{{content}}");

        let output_content = template.replace("{{content}}", canonical_content);
        let output_path = project_root.join(&self.instruction_path);

        let mut actions = Vec::new();

        // Ensure parent directory exists.
        if let Some(parent) = output_path.parent() {
            if parent != project_root && !parent.is_dir() {
                actions.push(SyncAction::CreateDirectory {
                    path: parent.to_path_buf(),
                });
            }
        }

        // Idempotent: skip if content unchanged.
        if output_path.exists() {
            let existing = std::fs::read_to_string(&output_path).map_err(AdapterError::Io)?;
            if existing == output_content {
                return Ok(vec![]);
            }
        }

        actions.push(SyncAction::CreateFile {
            path: output_path,
            content: output_content,
        });

        Ok(actions)
    }

    fn plan_sync_symlink(
        &self,
        project_root: &Path,
    ) -> Result<Vec<SyncAction>, AdapterError> {
        let link_path = project_root.join(&self.instruction_path);
        let target_rel = Path::new(".ai/instructions.md");

        // Idempotent: skip if correct symlink already exists.
        if let Ok(meta) = link_path.symlink_metadata() {
            if meta.file_type().is_symlink() {
                if let Ok(current_target) = std::fs::read_link(&link_path) {
                    if current_target == target_rel {
                        return Ok(vec![]);
                    }
                }
            }
        }

        Ok(vec![SyncAction::CreateSymlink {
            link: link_path,
            target: target_rel.to_path_buf(),
        }])
    }

    fn plan_sync_copy(
        &self,
        project_root: &Path,
        canonical_content: &str,
    ) -> Result<Vec<SyncAction>, AdapterError> {
        let output_path = project_root.join(&self.instruction_path);

        // Idempotent: skip if content unchanged.
        if output_path.exists() {
            let existing = std::fs::read_to_string(&output_path).map_err(AdapterError::Io)?;
            if existing == canonical_content {
                return Ok(vec![]);
            }
        }

        Ok(vec![SyncAction::CreateFile {
            path: output_path,
            content: canonical_content.to_string(),
        }])
    }
}

/// Strip frontmatter delimited by the given delimiter string.
///
/// Matches the pattern used by the Windsurf adapter: content between the
/// first occurrence of `delimiter` at the start and the next occurrence
/// is removed.
fn strip_frontmatter(raw: &str, delimiter: &str) -> String {
    if let Some(after_open) = raw.strip_prefix(delimiter) {
        if let Some(end_idx) = after_open.find(delimiter) {
            let after_frontmatter = &after_open[end_idx + delimiter.len()..];
            return after_frontmatter.trim_start_matches('\n').to_string();
        }
    }
    raw.to_string()
}

// ---------------------------------------------------------------------------
// Public loader
// ---------------------------------------------------------------------------

/// Scan `.ai/adapters/` for TOML adapter definition files.
///
/// Returns successfully parsed adapters; malformed files and builtin name
/// collisions are logged to stderr and skipped.
pub fn discover_toml_adapters(project_root: &Path) -> Vec<DeclarativeAdapter> {
    let adapters_dir = project_root.join(".ai").join("adapters");
    if !adapters_dir.is_dir() {
        return vec![];
    }

    let mut adapters = Vec::new();
    let entries = match std::fs::read_dir(&adapters_dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "toml") {
            match load_toml_adapter(&path) {
                Ok(adapter) => adapters.push(adapter),
                Err(e) => eprintln!("Warning: skipping adapter {}: {e}", path.display()),
            }
        }
    }
    adapters
}

/// Parse a TOML file into a `DeclarativeAdapter`.
pub fn load_toml_adapter(path: &Path) -> Result<DeclarativeAdapter, AdapterError> {
    let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
    let def: DeclarativeAdapterDef =
        toml::from_str(&content).map_err(|e| AdapterError::Other(format!(
            "failed to parse {}: {e}",
            path.display()
        )))?;
    DeclarativeAdapter::new(def)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper: build a minimal valid definition.
    fn minimal_def() -> DeclarativeAdapterDef {
        DeclarativeAdapterDef {
            name: "aider".to_string(),
            display_name: "Aider".to_string(),
            detection: DetectionDef {
                directories: vec![".aider".to_string()],
                files: vec![".aider.conf.yml".to_string()],
                match_any: true,
            },
            sync: SyncDef {
                strategy: SyncStrategy::Generate,
                instruction_path: ".aider/rules/project.md".to_string(),
                conditional_tags: vec!["aider-only".to_string()],
                gitignore_entries: vec![".aider/rules/".to_string()],
                watch_paths: vec![],
            },
            template: Some(TemplateDef {
                content: "---\nrule: always\n---\n\n{{content}}".to_string(),
                frontmatter_strip: Some("---".to_string()),
            }),
        }
    }

    // --- TOML deserialization ---

    #[test]
    fn test_deserialize_full_toml() {
        let toml_str = r#"
name = "aider"
display_name = "Aider"

[detection]
directories = [".aider"]
files = [".aider.conf.yml"]
match_any = true

[sync]
strategy = "generate"
instruction_path = ".aider/rules/project.md"
conditional_tags = ["aider-only"]
gitignore_entries = [".aider/rules/"]

[template]
content = "---\nrule: always\n---\n\n{{content}}"
frontmatter_strip = "---"
"#;
        let def: DeclarativeAdapterDef = toml::from_str(toml_str).unwrap();
        assert_eq!(def.name, "aider");
        assert_eq!(def.display_name, "Aider");
        assert!(def.detection.match_any);
        assert_eq!(def.detection.directories, vec![".aider"]);
        assert_eq!(def.sync.strategy, SyncStrategy::Generate);
        assert_eq!(def.sync.instruction_path, ".aider/rules/project.md");
        assert_eq!(def.sync.conditional_tags, vec!["aider-only"]);
        assert!(def.template.is_some());
        assert_eq!(def.template.unwrap().frontmatter_strip, Some("---".to_string()));
    }

    #[test]
    fn test_deserialize_minimal_toml() {
        let toml_str = r#"
name = "my-tool"
display_name = "My Tool"

[sync]
instruction_path = "MYTOOL.md"
"#;
        let def: DeclarativeAdapterDef = toml::from_str(toml_str).unwrap();
        assert_eq!(def.name, "my-tool");
        assert_eq!(def.sync.strategy, SyncStrategy::Symlink); // default
        assert!(def.detection.match_any); // default true
        assert!(def.detection.directories.is_empty());
        assert!(def.template.is_none());
    }

    // --- Builtin name collision ---

    #[test]
    fn test_rejects_builtin_names() {
        for name in BUILTIN_NAMES {
            let mut def = minimal_def();
            def.name = name.to_string();
            let result = DeclarativeAdapter::new(def);
            assert!(result.is_err(), "should reject builtin name '{}'", name);
            let err = result.unwrap_err().to_string();
            assert!(err.contains("collides with builtin"), "error message should mention collision: {}", err);
        }
    }

    #[test]
    fn test_accepts_non_builtin_name() {
        let def = minimal_def();
        let adapter = DeclarativeAdapter::new(def);
        assert!(adapter.is_ok());
    }

    // --- ToolAdapter method tests ---

    #[test]
    fn test_name_returns_custom() {
        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        assert_eq!(adapter.name(), ToolKind::Custom("aider".to_string()));
    }

    #[test]
    fn test_display_name() {
        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        assert_eq!(adapter.display_name(), "Aider");
    }

    #[test]
    fn test_native_instruction_path() {
        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        assert_eq!(adapter.native_instruction_path(), ".aider/rules/project.md");
    }

    #[test]
    fn test_conditional_tags() {
        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        assert_eq!(adapter.conditional_tags(), &["aider-only"]);
    }

    #[test]
    fn test_conditional_tags_empty() {
        let mut def = minimal_def();
        def.sync.conditional_tags = vec![];
        let adapter = DeclarativeAdapter::new(def).unwrap();
        assert!(adapter.conditional_tags().is_empty());
    }

    #[test]
    fn test_gitignore_entries() {
        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        assert_eq!(adapter.gitignore_entries(), vec![".aider/rules/"]);
    }

    #[test]
    fn test_default_sync_strategy() {
        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        assert_eq!(adapter.default_sync_strategy(), SyncStrategy::Generate);
    }

    #[test]
    fn test_watch_paths_defaults_to_instruction_path() {
        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        assert_eq!(adapter.watch_paths(), vec![".aider/rules/project.md"]);
    }

    #[test]
    fn test_watch_paths_custom() {
        let mut def = minimal_def();
        def.sync.watch_paths = vec!["path/a".to_string(), "path/b".to_string()];
        let adapter = DeclarativeAdapter::new(def).unwrap();
        assert_eq!(adapter.watch_paths(), vec!["path/a", "path/b"]);
    }

    // --- detect() ---

    #[test]
    fn test_detect_match_any_dir() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".aider")).unwrap();

        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        let result = adapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::Medium);
    }

    #[test]
    fn test_detect_match_any_file() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".aider.conf.yml"), "").unwrap();

        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        let result = adapter.detect(dir.path()).unwrap();
        assert!(result.detected);
    }

    #[test]
    fn test_detect_match_any_none_found() {
        let dir = TempDir::new().unwrap();

        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        let result = adapter.detect(dir.path()).unwrap();
        assert!(!result.detected);
    }

    #[test]
    fn test_detect_match_all_all_present() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".aider")).unwrap();
        std::fs::write(dir.path().join(".aider.conf.yml"), "").unwrap();

        let mut def = minimal_def();
        def.detection.match_any = false;
        let adapter = DeclarativeAdapter::new(def).unwrap();
        let result = adapter.detect(dir.path()).unwrap();
        assert!(result.detected);
    }

    #[test]
    fn test_detect_match_all_partial() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".aider")).unwrap();
        // .aider.conf.yml missing

        let mut def = minimal_def();
        def.detection.match_any = false;
        let adapter = DeclarativeAdapter::new(def).unwrap();
        let result = adapter.detect(dir.path()).unwrap();
        assert!(!result.detected, "match_all should require ALL markers");
    }

    #[test]
    fn test_detect_empty_markers_returns_not_detected() {
        let dir = TempDir::new().unwrap();

        let mut def = minimal_def();
        def.detection.directories = vec![];
        def.detection.files = vec![];
        let adapter = DeclarativeAdapter::new(def).unwrap();
        let result = adapter.detect(dir.path()).unwrap();
        assert!(!result.detected);
    }

    // --- plan_sync() Generate ---

    #[test]
    fn test_plan_sync_generate_creates_file() {
        let dir = TempDir::new().unwrap();

        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        let actions = adapter
            .plan_sync(dir.path(), "# My instructions", SyncStrategy::Generate)
            .unwrap();

        let has_create = actions
            .iter()
            .any(|a| matches!(a, SyncAction::CreateFile { .. }));
        assert!(has_create, "expected CreateFile action");

        if let Some(SyncAction::CreateFile { content, .. }) = actions.iter().find(|a| matches!(a, SyncAction::CreateFile { .. })) {
            assert!(content.contains("# My instructions"));
            assert!(content.contains("rule: always"));
        }
    }

    #[test]
    fn test_plan_sync_generate_creates_directory() {
        let dir = TempDir::new().unwrap();

        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        let actions = adapter
            .plan_sync(dir.path(), "content", SyncStrategy::Generate)
            .unwrap();

        let has_mkdir = actions
            .iter()
            .any(|a| matches!(a, SyncAction::CreateDirectory { .. }));
        assert!(has_mkdir, "expected CreateDirectory action");
    }

    #[test]
    fn test_plan_sync_generate_idempotent() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".aider").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        let canonical = "# Instructions";
        let template_content = "---\nrule: always\n---\n\n# Instructions";
        std::fs::write(rules_dir.join("project.md"), template_content).unwrap();

        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        let actions = adapter
            .plan_sync(dir.path(), canonical, SyncStrategy::Generate)
            .unwrap();
        assert!(actions.is_empty(), "expected no actions for unchanged content");
    }

    // --- plan_sync() Symlink ---

    #[test]
    fn test_plan_sync_symlink_creates_symlink() {
        let dir = TempDir::new().unwrap();

        let mut def = minimal_def();
        def.sync.strategy = SyncStrategy::Symlink;
        def.sync.instruction_path = "AIDER.md".to_string();
        let adapter = DeclarativeAdapter::new(def).unwrap();
        let actions = adapter
            .plan_sync(dir.path(), "content", SyncStrategy::Symlink)
            .unwrap();

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::CreateSymlink { link, target } => {
                assert_eq!(link, &dir.path().join("AIDER.md"));
                assert_eq!(target, Path::new(".ai/instructions.md"));
            }
            other => panic!("expected CreateSymlink, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_plan_sync_symlink_idempotent() {
        let dir = TempDir::new().unwrap();
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("instructions.md"), "content").unwrap();

        std::os::unix::fs::symlink(
            Path::new(".ai/instructions.md"),
            dir.path().join("AIDER.md"),
        )
        .unwrap();

        let mut def = minimal_def();
        def.sync.strategy = SyncStrategy::Symlink;
        def.sync.instruction_path = "AIDER.md".to_string();
        let adapter = DeclarativeAdapter::new(def).unwrap();
        let actions = adapter
            .plan_sync(dir.path(), "content", SyncStrategy::Symlink)
            .unwrap();
        assert!(actions.is_empty(), "expected no actions for correct symlink");
    }

    // --- plan_sync() Copy ---

    #[test]
    fn test_plan_sync_copy_creates_file() {
        let dir = TempDir::new().unwrap();

        let mut def = minimal_def();
        def.sync.strategy = SyncStrategy::Copy;
        def.sync.instruction_path = "TOOL.md".to_string();
        let adapter = DeclarativeAdapter::new(def).unwrap();
        let actions = adapter
            .plan_sync(dir.path(), "canonical content", SyncStrategy::Copy)
            .unwrap();

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::CreateFile { content, .. } => {
                assert_eq!(content, "canonical content");
            }
            other => panic!("expected CreateFile, got {other:?}"),
        }
    }

    // --- read_instructions() ---

    #[test]
    fn test_read_instructions_generate_strips_frontmatter() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".aider").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        let content = "---\nrule: always\n---\n\n# Instructions";
        std::fs::write(rules_dir.join("project.md"), content).unwrap();

        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        let result = adapter.read_instructions(dir.path()).unwrap();
        assert_eq!(result, Some("# Instructions".to_string()));
    }

    #[test]
    fn test_read_instructions_returns_none_when_missing() {
        let dir = TempDir::new().unwrap();

        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        let result = adapter.read_instructions(dir.path()).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_instructions_symlink_reads_directly() {
        let dir = TempDir::new().unwrap();

        let mut def = minimal_def();
        def.sync.strategy = SyncStrategy::Symlink;
        def.sync.instruction_path = "TOOL.md".to_string();
        def.template = None;
        let adapter = DeclarativeAdapter::new(def).unwrap();

        std::fs::write(dir.path().join("TOOL.md"), "raw content").unwrap();
        let result = adapter.read_instructions(dir.path()).unwrap();
        assert_eq!(result, Some("raw content".to_string()));
    }

    // --- sync_status() ---

    #[test]
    fn test_sync_status_missing() {
        let dir = TempDir::new().unwrap();

        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        let status = adapter
            .sync_status(dir.path(), "abc123", SyncStrategy::Generate)
            .unwrap();
        assert_eq!(status.drift, DriftState::Missing);
    }

    #[test]
    fn test_sync_status_in_sync() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".aider").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        let canonical = "# Instructions";
        let full_content = "---\nrule: always\n---\n\n# Instructions";
        std::fs::write(rules_dir.join("project.md"), full_content).unwrap();

        let canonical_hash = content_hash(canonical.as_bytes());
        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        let status = adapter
            .sync_status(dir.path(), &canonical_hash, SyncStrategy::Generate)
            .unwrap();
        assert_eq!(status.drift, DriftState::InSync);
    }

    #[test]
    fn test_sync_status_drifted() {
        let dir = TempDir::new().unwrap();
        let rules_dir = dir.path().join(".aider").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        let full_content = "---\nrule: always\n---\n\nold content";
        std::fs::write(rules_dir.join("project.md"), full_content).unwrap();

        let wrong_hash = content_hash(b"different content");
        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        let status = adapter
            .sync_status(dir.path(), &wrong_hash, SyncStrategy::Generate)
            .unwrap();
        assert!(matches!(status.drift, DriftState::Drifted { .. }));
    }

    // --- translate_hooks() ---

    #[test]
    fn test_translate_hooks_unsupported() {
        use std::collections::BTreeMap;

        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        let config = HooksConfig {
            events: BTreeMap::new(),
        };
        let result = adapter.translate_hooks(&config).unwrap();
        match result {
            HookTranslation::Unsupported { tool, reason } => {
                assert_eq!(tool, ToolKind::Custom("aider".to_string()));
                assert!(reason.contains("TOML adapter"));
            }
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    // --- plan_memory_sync() ---

    #[test]
    fn test_plan_memory_sync_returns_empty() {
        let dir = TempDir::new().unwrap();
        let adapter = DeclarativeAdapter::new(minimal_def()).unwrap();
        let actions = adapter.plan_memory_sync(dir.path(), &[]).unwrap();
        assert!(actions.is_empty());
    }

    // --- load_toml_adapter() ---

    #[test]
    fn test_load_toml_adapter_valid() {
        let dir = TempDir::new().unwrap();
        let toml_path = dir.path().join("aider.toml");
        let toml_str = r#"
name = "aider"
display_name = "Aider"

[detection]
directories = [".aider"]

[sync]
strategy = "generate"
instruction_path = ".aider/rules/project.md"

[template]
content = "{{content}}"
"#;
        std::fs::write(&toml_path, toml_str).unwrap();

        let adapter = load_toml_adapter(&toml_path).unwrap();
        assert_eq!(adapter.name(), ToolKind::Custom("aider".to_string()));
        assert_eq!(adapter.display_name(), "Aider");
    }

    #[test]
    fn test_load_toml_adapter_malformed() {
        let dir = TempDir::new().unwrap();
        let toml_path = dir.path().join("bad.toml");
        std::fs::write(&toml_path, "this is not valid toml [[[").unwrap();

        let result = load_toml_adapter(&toml_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_toml_adapter_builtin_collision() {
        let dir = TempDir::new().unwrap();
        let toml_path = dir.path().join("cursor.toml");
        let toml_str = r#"
name = "cursor"
display_name = "Cursor Override"

[sync]
instruction_path = ".cursor/rules/project.mdc"
"#;
        std::fs::write(&toml_path, toml_str).unwrap();

        let result = load_toml_adapter(&toml_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("collides"));
    }

    #[test]
    fn test_load_toml_adapter_missing_file() {
        let result = load_toml_adapter(Path::new("/nonexistent/adapter.toml"));
        assert!(result.is_err());
    }

    // --- strip_frontmatter helper ---

    #[test]
    fn test_strip_frontmatter_basic() {
        let raw = "---\nkey: value\n---\n\n# Body";
        assert_eq!(strip_frontmatter(raw, "---"), "# Body");
    }

    #[test]
    fn test_strip_frontmatter_no_frontmatter() {
        let raw = "# Just content";
        assert_eq!(strip_frontmatter(raw, "---"), "# Just content");
    }

    #[test]
    fn test_strip_frontmatter_custom_delimiter() {
        let raw = "+++\ntitle = 'test'\n+++\n\n# Body";
        assert_eq!(strip_frontmatter(raw, "+++"), "# Body");
    }

    // --- discover_toml_adapters() ---

    #[test]
    fn test_discover_no_adapters_dir() {
        let dir = TempDir::new().unwrap();
        let adapters = discover_toml_adapters(dir.path());
        assert!(adapters.is_empty());
    }

    #[test]
    fn test_discover_empty_adapters_dir() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".ai/adapters")).unwrap();
        let adapters = discover_toml_adapters(dir.path());
        assert!(adapters.is_empty());
    }

    #[test]
    fn test_discover_valid_toml_files() {
        let dir = TempDir::new().unwrap();
        let adapters_dir = dir.path().join(".ai/adapters");
        std::fs::create_dir_all(&adapters_dir).unwrap();

        let toml1 = r#"
name = "aider"
display_name = "Aider"

[detection]
directories = [".aider"]

[sync]
strategy = "generate"
instruction_path = ".aider/rules/project.md"

[template]
content = "{{content}}"
"#;
        std::fs::write(adapters_dir.join("aider.toml"), toml1).unwrap();

        let toml2 = r#"
name = "continue"
display_name = "Continue"

[sync]
instruction_path = ".continue/instructions.md"
"#;
        std::fs::write(adapters_dir.join("continue.toml"), toml2).unwrap();

        let adapters = discover_toml_adapters(dir.path());
        assert_eq!(adapters.len(), 2);

        let names: Vec<String> = adapters.iter().map(|a| a.display_name().to_string()).collect();
        assert!(names.contains(&"Aider".to_string()));
        assert!(names.contains(&"Continue".to_string()));
    }

    #[test]
    fn test_discover_skips_malformed_toml() {
        let dir = TempDir::new().unwrap();
        let adapters_dir = dir.path().join(".ai/adapters");
        std::fs::create_dir_all(&adapters_dir).unwrap();

        // Valid file
        let valid = r#"
name = "aider"
display_name = "Aider"

[sync]
instruction_path = ".aider/rules/project.md"
"#;
        std::fs::write(adapters_dir.join("aider.toml"), valid).unwrap();

        // Malformed file
        std::fs::write(adapters_dir.join("bad.toml"), "this is not valid [[[").unwrap();

        let adapters = discover_toml_adapters(dir.path());
        assert_eq!(adapters.len(), 1);
        assert_eq!(adapters[0].display_name(), "Aider");
    }

    #[test]
    fn test_discover_skips_builtin_name_collisions() {
        let dir = TempDir::new().unwrap();
        let adapters_dir = dir.path().join(".ai/adapters");
        std::fs::create_dir_all(&adapters_dir).unwrap();

        // File with builtin name
        let cursor_toml = r#"
name = "cursor"
display_name = "Cursor Override"

[sync]
instruction_path = ".cursor/rules/project.mdc"
"#;
        std::fs::write(adapters_dir.join("cursor.toml"), cursor_toml).unwrap();

        let adapters = discover_toml_adapters(dir.path());
        assert!(adapters.is_empty());
    }

    #[test]
    fn test_discover_skips_non_toml_files() {
        let dir = TempDir::new().unwrap();
        let adapters_dir = dir.path().join(".ai/adapters");
        std::fs::create_dir_all(&adapters_dir).unwrap();

        // Non-toml files
        std::fs::write(adapters_dir.join("README.md"), "# Adapters").unwrap();
        std::fs::write(adapters_dir.join("notes.txt"), "some notes").unwrap();

        let adapters = discover_toml_adapters(dir.path());
        assert!(adapters.is_empty());
    }
}
