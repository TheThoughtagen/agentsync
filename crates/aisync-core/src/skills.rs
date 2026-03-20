use std::path::Path;

use crate::error::{AisyncError, SyncError};
use crate::types::SkillFile;

/// Engine for loading canonical skill files from `.ai/skills/*/SKILL.md`.
pub struct SkillEngine;

impl SkillEngine {
    /// Load all canonical skill files from `.ai/skills/*/SKILL.md`.
    ///
    /// Returns an empty Vec if the directory doesn't exist or contains no skill subdirectories.
    /// Skips subdirectories that do not contain a `SKILL.md` file.
    /// Results are sorted by name for deterministic ordering.
    pub fn load(project_root: &Path) -> Result<Vec<SkillFile>, AisyncError> {
        let skills_dir = project_root.join(".ai/skills");
        if !skills_dir.is_dir() {
            return Ok(vec![]);
        }

        let mut skills = Vec::new();
        let entries = std::fs::read_dir(&skills_dir)
            .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
            let path = entry.path();
            if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                if skill_md.is_file() {
                    let content = std::fs::read_to_string(&skill_md)
                        .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                    let name = path.file_name().unwrap().to_string_lossy().to_string();
                    skills.push(SkillFile {
                        name,
                        content,
                        source_path: skill_md,
                    });
                }
            }
        }

        skills.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(skills)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_returns_empty_when_no_skills_dir() {
        let dir = TempDir::new().unwrap();
        let skills = SkillEngine::load(dir.path()).unwrap();
        assert!(skills.is_empty(), "should return empty vec when .ai/skills/ doesn't exist");
    }

    #[test]
    fn test_load_returns_empty_when_no_skill_subdirs() {
        let dir = TempDir::new().unwrap();
        let skills_dir = dir.path().join(".ai/skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        let skills = SkillEngine::load(dir.path()).unwrap();
        assert!(skills.is_empty(), "should return empty vec when no skill subdirectories");
    }

    #[test]
    fn test_load_skips_dirs_without_skill_md() {
        let dir = TempDir::new().unwrap();
        let skills_dir = dir.path().join(".ai/skills");
        let skill_dir = skills_dir.join("no-skill-md");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("README.md"), "not a skill").unwrap();

        let skills = SkillEngine::load(dir.path()).unwrap();
        assert!(skills.is_empty(), "should skip directories without SKILL.md");
    }

    #[test]
    fn test_load_reads_skill_md_files_sorted_by_name() {
        let dir = TempDir::new().unwrap();
        let skills_dir = dir.path().join(".ai/skills");

        let zz_skill_dir = skills_dir.join("zz-deploy");
        std::fs::create_dir_all(&zz_skill_dir).unwrap();
        std::fs::write(zz_skill_dir.join("SKILL.md"), "Deploy skill").unwrap();

        let aa_skill_dir = skills_dir.join("aa-build");
        std::fs::create_dir_all(&aa_skill_dir).unwrap();
        std::fs::write(aa_skill_dir.join("SKILL.md"), "Build skill").unwrap();

        let skills = SkillEngine::load(dir.path()).unwrap();
        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0].name, "aa-build");
        assert_eq!(skills[1].name, "zz-deploy");
    }

    #[test]
    fn test_load_populates_fields_correctly() {
        let dir = TempDir::new().unwrap();
        let skills_dir = dir.path().join(".ai/skills");
        let skill_dir = skills_dir.join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# My Skill\nDoes things").unwrap();

        let skills = SkillEngine::load(dir.path()).unwrap();
        assert_eq!(skills.len(), 1);
        let skill = &skills[0];
        assert_eq!(skill.name, "my-skill");
        assert_eq!(skill.content, "# My Skill\nDoes things");
        assert!(skill.source_path.ends_with(".ai/skills/my-skill/SKILL.md"));
    }
}
