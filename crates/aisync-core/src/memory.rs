/// Memory engine for managing .ai/memory/ topic files and importing from Claude.

use std::path::{Path, PathBuf};

#[allow(unused_imports)]
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
    pub fn list(_project_root: &Path) -> Result<Vec<PathBuf>, AisyncError> {
        todo!("MemoryEngine::list not yet implemented")
    }

    /// Create a new memory file .ai/memory/<topic>.md with "# <Topic>" header.
    pub fn add(_project_root: &Path, _topic: &str) -> Result<PathBuf, AisyncError> {
        todo!("MemoryEngine::add not yet implemented")
    }

    /// Compute Claude Code project key from absolute path.
    pub fn claude_project_key(_project_root: &Path) -> Result<String, AisyncError> {
        todo!("MemoryEngine::claude_project_key not yet implemented")
    }

    /// Get the Claude memory directory path.
    pub fn claude_memory_path(_project_root: &Path) -> Result<PathBuf, AisyncError> {
        todo!("MemoryEngine::claude_memory_path not yet implemented")
    }

    /// Import memory files from Claude's auto-memory into .ai/memory/.
    pub fn import_claude(_project_root: &Path) -> Result<ImportResult, AisyncError> {
        todo!("MemoryEngine::import_claude not yet implemented")
    }
}
