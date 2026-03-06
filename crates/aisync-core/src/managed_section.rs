/// Generalized managed section update with configurable markers.
///
/// This module provides the core algorithm for managing delimited sections
/// within text files. The gitignore module delegates to this for its
/// specific marker format.
use std::path::Path;

/// Updates a managed section in a file using custom start/end markers.
///
/// - If the file doesn't exist, creates it with the managed section.
/// - If marker_start exists, replaces everything between markers (inclusive) with new section.
/// - If marker_start exists without marker_end, replaces from marker_start to end of file.
/// - If no markers exist, appends the managed section.
pub fn update_managed_section(
    file_path: &Path,
    entries: &[&str],
    marker_start: &str,
    marker_end: &str,
) -> Result<(), std::io::Error> {
    use std::fs;

    let existing = fs::read_to_string(file_path).unwrap_or_default();

    let managed_section = format!("{}\n{}\n{}", marker_start, entries.join("\n"), marker_end,);

    let new_content = if let Some(start_idx) = existing.find(marker_start) {
        let before = &existing[..start_idx];
        let after_marker_start = &existing[start_idx..];

        let after = if let Some(end_offset) = after_marker_start.find(marker_end) {
            let end_idx = start_idx + end_offset + marker_end.len();
            // Skip any trailing newline after marker_end
            let end_idx = if existing[end_idx..].starts_with('\n') {
                end_idx + 1
            } else {
                end_idx
            };
            &existing[end_idx..]
        } else {
            // No end marker: replace from start marker to end of file
            ""
        };

        format!("{}{}\n{}", before, managed_section, after)
    } else {
        // No existing managed section: append
        if existing.is_empty() {
            format!("{}\n", managed_section)
        } else if existing.ends_with('\n') {
            format!("{}{}\n", existing, managed_section)
        } else {
            format!("{}\n{}\n", existing, managed_section)
        }
    };

    fs::write(file_path, new_content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const TEST_START: &str = "<!-- managed-start -->";
    const TEST_END: &str = "<!-- managed-end -->";

    #[test]
    fn test_custom_markers_create_new_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.md");

        update_managed_section(&path, &["ref1", "ref2"], TEST_START, TEST_END).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains(TEST_START));
        assert!(content.contains(TEST_END));
        assert!(content.contains("ref1"));
        assert!(content.contains("ref2"));
    }

    #[test]
    fn test_custom_markers_replace_existing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.md");
        let existing = format!(
            "# Header\n{}\nold-ref\n{}\n# Footer\n",
            TEST_START, TEST_END
        );
        fs::write(&path, &existing).unwrap();

        update_managed_section(&path, &["new-ref"], TEST_START, TEST_END).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Header"));
        assert!(content.contains("new-ref"));
        assert!(!content.contains("old-ref"));
        assert!(content.contains("# Footer"));
    }

    #[test]
    fn test_custom_markers_append_to_existing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "existing content\n").unwrap();

        update_managed_section(&path, &["entry"], TEST_START, TEST_END).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("existing content\n"));
        assert!(content.contains(TEST_START));
        assert!(content.contains("entry"));
    }
}
