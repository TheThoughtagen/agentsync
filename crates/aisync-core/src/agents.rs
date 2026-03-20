use std::path::Path;

use crate::error::{AisyncError, SyncError};
use crate::types::AgentFile;

/// Engine for loading canonical agent files from `.ai/agents/*.md`.
pub struct AgentEngine;

impl AgentEngine {
    /// Load all canonical agent files from `.ai/agents/*.md`.
    ///
    /// Returns an empty Vec if the directory doesn't exist or contains no `.md` files.
    /// Non-.md files are ignored.
    /// Results are sorted by name for deterministic ordering.
    pub fn load(project_root: &Path) -> Result<Vec<AgentFile>, AisyncError> {
        let agents_dir = project_root.join(".ai/agents");
        if !agents_dir.is_dir() {
            return Ok(vec![]);
        }

        let mut agents = Vec::new();
        let entries = std::fs::read_dir(&agents_dir)
            .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
                let content = std::fs::read_to_string(&path)
                    .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                let name = path.file_stem().unwrap().to_string_lossy().to_string();
                agents.push(AgentFile {
                    name,
                    content,
                    source_path: path,
                });
            }
        }

        agents.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(agents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_returns_empty_when_no_agents_dir() {
        let dir = TempDir::new().unwrap();
        let agents = AgentEngine::load(dir.path()).unwrap();
        assert!(agents.is_empty(), "should return empty vec when .ai/agents/ doesn't exist");
    }

    #[test]
    fn test_load_returns_empty_when_no_md_files() {
        let dir = TempDir::new().unwrap();
        let agents_dir = dir.path().join(".ai/agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(agents_dir.join("README.txt"), "not an agent").unwrap();

        let agents = AgentEngine::load(dir.path()).unwrap();
        assert!(agents.is_empty(), "should return empty vec when no .md files");
    }

    #[test]
    fn test_load_ignores_non_md_files() {
        let dir = TempDir::new().unwrap();
        let agents_dir = dir.path().join(".ai/agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(agents_dir.join("notes.txt"), "just a note").unwrap();
        std::fs::write(agents_dir.join("config.json"), "{}").unwrap();
        std::fs::write(agents_dir.join("my-agent.md"), "# My Agent").unwrap();

        let agents = AgentEngine::load(dir.path()).unwrap();
        assert_eq!(agents.len(), 1, "should only load .md files");
        assert_eq!(agents[0].name, "my-agent");
    }

    #[test]
    fn test_load_reads_md_files_sorted_by_name() {
        let dir = TempDir::new().unwrap();
        let agents_dir = dir.path().join(".ai/agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(agents_dir.join("zz-frontend.md"), "Frontend agent").unwrap();
        std::fs::write(agents_dir.join("aa-backend.md"), "Backend agent").unwrap();

        let agents = AgentEngine::load(dir.path()).unwrap();
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].name, "aa-backend");
        assert_eq!(agents[1].name, "zz-frontend");
    }

    #[test]
    fn test_load_populates_fields_correctly() {
        let dir = TempDir::new().unwrap();
        let agents_dir = dir.path().join(".ai/agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(agents_dir.join("backend-expert.md"), "# Backend Expert\nHelps with APIs").unwrap();

        let agents = AgentEngine::load(dir.path()).unwrap();
        assert_eq!(agents.len(), 1);
        let agent = &agents[0];
        assert_eq!(agent.name, "backend-expert");
        assert_eq!(agent.content, "# Backend Expert\nHelps with APIs");
        assert!(agent.source_path.ends_with(".ai/agents/backend-expert.md"));
    }
}
