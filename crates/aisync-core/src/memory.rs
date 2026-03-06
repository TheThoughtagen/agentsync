/// Memory engine for managing .ai/memory/ topic files and importing from Claude.

use std::path::{Path, PathBuf};

use crate::error::{AisyncError, MemoryError};

/// Result of importing Claude memory files.
#[derive(Debug, Clone)]
pub struct ImportResult {
    /// Files that were successfully imported.
    pub imported: Vec<String>,
    /// Files that already exist in .ai/memory/ (conflicts).
    pub conflicts: Vec<String>,
    /// The source path that was imported from.
    pub source_path: PathBuf,
}

/// Engine for memory file operations.
pub struct MemoryEngine;

impl MemoryEngine {
    /// List all .md files in .ai/memory/, sorted alphabetically.
    pub fn list(project_root: &Path) -> Result<Vec<PathBuf>, AisyncError> {
        let memory_dir = project_root.join(".ai/memory");
        if !memory_dir.exists() {
            return Ok(vec![]);
        }

        let mut files = Vec::new();
        let entries = std::fs::read_dir(&memory_dir)
            .map_err(MemoryError::ReadFailed)?;

        for entry in entries {
            let entry = entry.map_err(MemoryError::ReadFailed)?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "md" {
                        files.push(path);
                    }
                }
            }
        }

        files.sort();
        Ok(files)
    }

    /// Create a new memory file .ai/memory/<topic>.md with "# <Topic>" header.
    /// Sanitizes topic name: lowercase, hyphens for spaces, strip non-alphanumeric except hyphens.
    pub fn add(project_root: &Path, topic: &str) -> Result<PathBuf, AisyncError> {
        let memory_dir = project_root.join(".ai/memory");
        std::fs::create_dir_all(&memory_dir)
            .map_err(MemoryError::WriteFailed)?;

        let sanitized = Self::sanitize_filename(topic);
        let filename = format!("{}.md", sanitized);
        let file_path = memory_dir.join(&filename);

        if file_path.exists() {
            return Err(MemoryError::AlreadyExists {
                path: file_path.display().to_string(),
            }.into());
        }

        let title = Self::title_case(topic);
        let content = format!("# {}\n", title);
        std::fs::write(&file_path, content)
            .map_err(MemoryError::WriteFailed)?;

        Ok(file_path)
    }

    /// Compute Claude Code project key: absolute path with '/' replaced by '-'.
    /// Example: /Users/pmannion/project -> -Users-pmannion-project
    pub fn claude_project_key(project_root: &Path) -> Result<String, AisyncError> {
        let canonical = project_root.canonicalize()
            .map_err(MemoryError::PathResolution)?;
        let path_str = canonical.to_string_lossy();
        Ok(path_str.replace('/', "-"))
    }

    /// Get the Claude memory directory path: ~/.claude/projects/<key>/memory/
    pub fn claude_memory_path(project_root: &Path) -> Result<PathBuf, AisyncError> {
        let key = Self::claude_project_key(project_root)?;
        let home = dirs::home_dir().ok_or_else(|| {
            MemoryError::PathResolution(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "could not determine home directory",
            ))
        })?;
        Ok(home.join(".claude/projects").join(key).join("memory"))
    }

    /// Import memory files from Claude's auto-memory into .ai/memory/.
    /// Returns list of imported filenames. Does NOT prompt -- that is CLI layer's job.
    /// If a file already exists in .ai/memory/, includes it in a conflicts Vec for CLI to handle.
    pub fn import_claude(project_root: &Path) -> Result<ImportResult, AisyncError> {
        let claude_path = Self::claude_memory_path(project_root)?;

        if !claude_path.exists() {
            return Err(MemoryError::ClaudeMemoryNotFound {
                path: claude_path.display().to_string(),
            }.into());
        }

        let memory_dir = project_root.join(".ai/memory");
        std::fs::create_dir_all(&memory_dir)
            .map_err(MemoryError::WriteFailed)?;

        let mut imported = Vec::new();
        let mut conflicts = Vec::new();

        let entries = std::fs::read_dir(&claude_path)
            .map_err(MemoryError::ReadFailed)?;

        for entry in entries {
            let entry = entry.map_err(MemoryError::ReadFailed)?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(ext) = path.extension() {
                if ext != "md" {
                    continue;
                }
            } else {
                continue;
            }

            let filename = entry.file_name().to_string_lossy().to_string();
            let dest = memory_dir.join(&filename);

            if dest.exists() {
                conflicts.push(filename);
            } else {
                std::fs::copy(&path, &dest)
                    .map_err(MemoryError::WriteFailed)?;
                imported.push(filename);
            }
        }

        imported.sort();
        conflicts.sort();

        Ok(ImportResult {
            imported,
            conflicts,
            source_path: claude_path,
        })
    }

    /// Sanitize a topic name into a valid filename.
    /// Lowercase, hyphens for spaces, strip non-alphanumeric except hyphens.
    fn sanitize_filename(topic: &str) -> String {
        let mut result = String::new();
        for ch in topic.chars() {
            if ch.is_alphanumeric() {
                result.push(ch.to_ascii_lowercase());
            } else if ch == ' ' || ch == '_' {
                if !result.ends_with('-') {
                    result.push('-');
                }
            } else {
                // Skip non-alphanumeric characters, but if we had content before
                // and the next char is alphanumeric, we might want a separator
                // Only add hyphen if result is non-empty and doesn't already end with one
            }
        }
        // Trim trailing hyphens
        result.trim_end_matches('-').to_string()
    }

    /// Convert a topic string to title case.
    fn title_case(s: &str) -> String {
        s.split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        let upper: String = first.to_uppercase().collect();
                        let rest: String = chars.collect::<String>().to_lowercase();
                        format!("{}{}", upper, rest)
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // --- list tests ---

    #[test]
    fn test_list_returns_sorted_md_files() {
        let dir = TempDir::new().unwrap();
        let memory_dir = dir.path().join(".ai/memory");
        fs::create_dir_all(&memory_dir).unwrap();
        fs::write(memory_dir.join("zebra.md"), "# Zebra").unwrap();
        fs::write(memory_dir.join("alpha.md"), "# Alpha").unwrap();
        fs::write(memory_dir.join("middle.md"), "# Middle").unwrap();
        // Non-.md file should be excluded
        fs::write(memory_dir.join("notes.txt"), "not markdown").unwrap();

        let files = MemoryEngine::list(dir.path()).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files[0].ends_with("alpha.md"));
        assert!(files[1].ends_with("middle.md"));
        assert!(files[2].ends_with("zebra.md"));
    }

    #[test]
    fn test_list_returns_empty_when_dir_missing() {
        let dir = TempDir::new().unwrap();
        let files = MemoryEngine::list(dir.path()).unwrap();
        assert!(files.is_empty());
    }

    // --- add tests ---

    #[test]
    fn test_add_creates_topic_file_with_header() {
        let dir = TempDir::new().unwrap();
        let path = MemoryEngine::add(dir.path(), "deployment notes").unwrap();

        assert!(path.exists());
        assert!(path.ends_with("deployment-notes.md"));
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("# Deployment Notes"));
    }

    #[test]
    fn test_add_sanitizes_filename() {
        let dir = TempDir::new().unwrap();
        let path = MemoryEngine::add(dir.path(), "My Cool Topic! (v2)").unwrap();

        let filename = path.file_name().unwrap().to_str().unwrap();
        assert_eq!(filename, "my-cool-topic-v2.md");
    }

    #[test]
    fn test_add_errors_if_file_exists() {
        let dir = TempDir::new().unwrap();
        let memory_dir = dir.path().join(".ai/memory");
        fs::create_dir_all(&memory_dir).unwrap();
        fs::write(memory_dir.join("existing.md"), "# Existing").unwrap();

        let result = MemoryEngine::add(dir.path(), "existing");
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("already exists"));
    }

    #[test]
    fn test_add_creates_memory_directory() {
        let dir = TempDir::new().unwrap();
        // .ai/memory/ does not exist yet
        assert!(!dir.path().join(".ai/memory").exists());

        MemoryEngine::add(dir.path(), "new topic").unwrap();
        assert!(dir.path().join(".ai/memory").exists());
    }

    // --- claude_project_key tests ---

    #[test]
    fn test_claude_project_key_replaces_slashes() {
        let dir = TempDir::new().unwrap();
        let canonical = dir.path().canonicalize().unwrap();
        let key = MemoryEngine::claude_project_key(&canonical).unwrap();

        // Should start with '-' (replacing leading '/')
        assert!(key.starts_with('-'));
        // Should not contain any '/'
        assert!(!key.contains('/'));
        // Should contain the path components separated by '-'
        assert!(key.len() > 1);
    }

    // --- claude_memory_path tests ---

    #[test]
    fn test_claude_memory_path_format() {
        let dir = TempDir::new().unwrap();
        let canonical = dir.path().canonicalize().unwrap();
        let path = MemoryEngine::claude_memory_path(&canonical).unwrap();

        let path_str = path.to_string_lossy();
        assert!(path_str.contains(".claude/projects/"));
        assert!(path_str.ends_with("/memory"));
    }

    // --- import_claude tests ---

    #[test]
    fn test_import_claude_copies_md_files() {
        let dir = TempDir::new().unwrap();
        let project_root = dir.path().join("project");
        fs::create_dir_all(&project_root).unwrap();

        // Create a fake Claude memory directory
        let claude_memory = MemoryEngine::claude_memory_path(
            &project_root.canonicalize().unwrap(),
        ).unwrap();
        fs::create_dir_all(&claude_memory).unwrap();
        fs::write(claude_memory.join("topic-a.md"), "# Topic A content").unwrap();
        fs::write(claude_memory.join("topic-b.md"), "# Topic B content").unwrap();

        let result = MemoryEngine::import_claude(
            &project_root.canonicalize().unwrap(),
        ).unwrap();

        assert_eq!(result.imported.len(), 2);
        assert!(result.conflicts.is_empty());

        // Verify files were actually copied
        let ai_memory = project_root.join(".ai/memory");
        assert!(ai_memory.join("topic-a.md").exists());
        assert!(ai_memory.join("topic-b.md").exists());
    }

    #[test]
    fn test_import_claude_reports_conflicts() {
        let dir = TempDir::new().unwrap();
        let project_root = dir.path().join("project");
        fs::create_dir_all(&project_root).unwrap();

        // Create existing memory file
        let ai_memory = project_root.join(".ai/memory");
        fs::create_dir_all(&ai_memory).unwrap();
        fs::write(ai_memory.join("existing.md"), "# Existing local").unwrap();

        // Create Claude memory with overlapping file
        let claude_memory = MemoryEngine::claude_memory_path(
            &project_root.canonicalize().unwrap(),
        ).unwrap();
        fs::create_dir_all(&claude_memory).unwrap();
        fs::write(claude_memory.join("existing.md"), "# Existing remote").unwrap();
        fs::write(claude_memory.join("new-topic.md"), "# New topic").unwrap();

        let result = MemoryEngine::import_claude(
            &project_root.canonicalize().unwrap(),
        ).unwrap();

        assert_eq!(result.imported.len(), 1);
        assert!(result.imported.contains(&"new-topic.md".to_string()));
        assert_eq!(result.conflicts.len(), 1);
        assert!(result.conflicts.contains(&"existing.md".to_string()));

        // Existing file should NOT be overwritten
        let content = fs::read_to_string(ai_memory.join("existing.md")).unwrap();
        assert_eq!(content, "# Existing local");
    }

    #[test]
    fn test_import_claude_errors_when_path_missing() {
        let dir = TempDir::new().unwrap();
        let project_root = dir.path().join("nonexistent-project");
        fs::create_dir_all(&project_root).unwrap();

        // Don't create Claude memory dir -- should error
        let result = MemoryEngine::import_claude(
            &project_root.canonicalize().unwrap(),
        );
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("claude memory path not found") || err.contains("ClaudeMemoryNotFound"));
    }
}
