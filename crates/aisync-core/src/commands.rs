use std::path::Path;

use crate::error::{AisyncError, SyncError};
use crate::types::CommandFile;

/// Engine for loading canonical command files from `.ai/commands/`.
pub struct CommandEngine;

impl CommandEngine {
    /// Load all canonical command files from `.ai/commands/*.md`.
    ///
    /// Returns an empty Vec if the directory doesn't exist or contains no `.md` files.
    /// Results are sorted by name for deterministic ordering.
    pub fn load(project_root: &Path) -> Result<Vec<CommandFile>, AisyncError> {
        let commands_dir = project_root.join(".ai/commands");
        if !commands_dir.is_dir() {
            return Ok(vec![]);
        }

        let mut commands = Vec::new();
        let entries = std::fs::read_dir(&commands_dir)
            .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                let content = std::fs::read_to_string(&path)
                    .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                let name = path.file_stem().unwrap().to_string_lossy().to_string();
                commands.push(CommandFile {
                    name,
                    content,
                    source_path: path,
                });
            }
        }

        commands.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(commands)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_returns_empty_when_no_commands_dir() {
        let dir = TempDir::new().unwrap();
        let commands = CommandEngine::load(dir.path()).unwrap();
        assert!(commands.is_empty(), "should return empty vec when .ai/commands/ doesn't exist");
    }

    #[test]
    fn test_load_returns_empty_when_no_md_files() {
        let dir = TempDir::new().unwrap();
        let commands_dir = dir.path().join(".ai/commands");
        std::fs::create_dir_all(&commands_dir).unwrap();
        std::fs::write(commands_dir.join("readme.txt"), "not a command").unwrap();

        let commands = CommandEngine::load(dir.path()).unwrap();
        assert!(commands.is_empty(), "should return empty vec when no .md files");
    }

    #[test]
    fn test_load_reads_md_files_sorted_by_name() {
        let dir = TempDir::new().unwrap();
        let commands_dir = dir.path().join(".ai/commands");
        std::fs::create_dir_all(&commands_dir).unwrap();

        std::fs::write(commands_dir.join("zz-deploy.md"), "Deploy the app").unwrap();
        std::fs::write(commands_dir.join("aa-build.md"), "Build the project").unwrap();

        let commands = CommandEngine::load(dir.path()).unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].name, "aa-build");
        assert_eq!(commands[1].name, "zz-deploy");
    }

    #[test]
    fn test_load_populates_fields_correctly() {
        let dir = TempDir::new().unwrap();
        let commands_dir = dir.path().join(".ai/commands");
        std::fs::create_dir_all(&commands_dir).unwrap();

        std::fs::write(commands_dir.join("test-cmd.md"), "Run all tests\n\ncargo test").unwrap();

        let commands = CommandEngine::load(dir.path()).unwrap();
        assert_eq!(commands.len(), 1);
        let cmd = &commands[0];
        assert_eq!(cmd.name, "test-cmd");
        assert_eq!(cmd.content, "Run all tests\n\ncargo test");
        assert!(cmd.source_path.ends_with(".ai/commands/test-cmd.md"));
    }
}
