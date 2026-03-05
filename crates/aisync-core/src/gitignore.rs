/// Managed .gitignore section markers and utilities.

pub const MARKER_START: &str = "# aisync-managed";
pub const MARKER_END: &str = "# /aisync-managed";

/// Updates the managed section in a .gitignore file.
///
/// - If the file doesn't exist, creates it with the managed section.
/// - If MARKER_START exists, replaces everything between markers (inclusive) with new section.
/// - If MARKER_START exists without MARKER_END, replaces from MARKER_START to end of file.
/// - If no markers exist, appends the managed section.
pub fn update_managed_section(
    gitignore_path: &std::path::Path,
    entries: &[&str],
) -> Result<(), std::io::Error> {
    // TODO: implement
    let _ = (gitignore_path, entries);
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_append_to_empty_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".gitignore");
        fs::write(&path, "").unwrap();

        update_managed_section(&path, &[".ai/", "CLAUDE.md"]).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains(MARKER_START));
        assert!(content.contains(MARKER_END));
        assert!(content.contains(".ai/"));
        assert!(content.contains("CLAUDE.md"));
    }

    #[test]
    fn test_append_to_nonexistent_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".gitignore");

        update_managed_section(&path, &[".ai/"]).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains(MARKER_START));
        assert!(content.contains(".ai/"));
    }

    #[test]
    fn test_replace_existing_section() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".gitignore");
        let existing = format!(
            "node_modules/\n{}\nold-entry\n{}\n.env\n",
            MARKER_START, MARKER_END
        );
        fs::write(&path, &existing).unwrap();

        update_managed_section(&path, &["new-entry"]).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("node_modules/"));
        assert!(content.contains("new-entry"));
        assert!(!content.contains("old-entry"));
        assert!(content.contains(".env"));
    }

    #[test]
    fn test_handles_missing_end_marker() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".gitignore");
        let existing = format!("node_modules/\n{}\nold-entry\n", MARKER_START);
        fs::write(&path, &existing).unwrap();

        update_managed_section(&path, &["new-entry"]).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("node_modules/"));
        assert!(content.contains("new-entry"));
        assert!(!content.contains("old-entry"));
        assert!(content.contains(MARKER_END));
    }

    #[test]
    fn test_preserves_content_outside_section() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".gitignore");
        let existing = format!(
            "# Custom ignores\nnode_modules/\n\n{}\nold\n{}\n\n# More custom\n.env\n",
            MARKER_START, MARKER_END
        );
        fs::write(&path, &existing).unwrap();

        update_managed_section(&path, &["replaced"]).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Custom ignores"));
        assert!(content.contains("node_modules/"));
        assert!(content.contains("# More custom"));
        assert!(content.contains(".env"));
        assert!(content.contains("replaced"));
        assert!(!content.contains("old"));
    }

    #[test]
    fn test_appends_with_newline_separator() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".gitignore");
        fs::write(&path, "node_modules/").unwrap(); // no trailing newline

        update_managed_section(&path, &[".ai/"]).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        // Should have a newline between existing content and managed section
        assert!(content.starts_with("node_modules/\n"));
        assert!(content.contains(MARKER_START));
    }
}
