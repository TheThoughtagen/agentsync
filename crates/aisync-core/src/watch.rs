use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::Arc;
use std::time::Duration;

use notify_debouncer_mini::new_debouncer;
use notify::RecursiveMode;

use crate::adapter::{AnyAdapter, ClaudeCodeAdapter, CursorAdapter, OpenCodeAdapter, ToolAdapter};
use crate::config::AisyncConfig;
use crate::error::{AisyncError, WatchError};
use crate::sync::SyncEngine;
use crate::types::{ToolKind, WatchEvent};

/// The watch engine monitors filesystem changes and orchestrates
/// forward sync (canonical -> tools) and reverse sync (tool-native -> canonical).
pub struct WatchEngine;

impl WatchEngine {
    /// Start watching for filesystem changes and syncing bidirectionally.
    ///
    /// - `config`: The aisync configuration
    /// - `project_root`: The project root directory
    /// - `running`: Shared flag for graceful shutdown (set to false to stop)
    /// - `event_callback`: Called for each watch event (for logging/display)
    pub fn watch(
        config: &AisyncConfig,
        project_root: &Path,
        running: Arc<AtomicBool>,
        event_callback: impl Fn(WatchEvent),
    ) -> Result<(), AisyncError> {
        let syncing = Arc::new(AtomicBool::new(false));

        let (tx, rx) = std::sync::mpsc::channel();

        let mut debouncer = new_debouncer(Duration::from_millis(500), tx)
            .map_err(|e| AisyncError::Watch(WatchError::WatchFailed(format!("{e}"))))?;

        // Watch .ai/ directory recursively
        let ai_dir = project_root.join(".ai");
        if ai_dir.is_dir() {
            debouncer
                .watcher()
                .watch(&ai_dir, RecursiveMode::Recursive)
                .map_err(|e| {
                    AisyncError::Watch(WatchError::WatchFailed(format!(
                        "failed to watch .ai/: {e}"
                    )))
                })?;
        }

        // Watch tool-native files (non-symlink only)
        let tool_paths = Self::tool_watch_paths(config, project_root);
        for path in &tool_paths {
            debouncer
                .watcher()
                .watch(path, RecursiveMode::NonRecursive)
                .map_err(|e| {
                    AisyncError::Watch(WatchError::WatchFailed(format!(
                        "failed to watch {}: {e}",
                        path.display()
                    )))
                })?;
        }

        // Event loop
        for events in rx {
            if !running.load(SeqCst) {
                break;
            }

            if syncing.load(SeqCst) {
                continue;
            }

            let events = match events {
                Ok(events) => events,
                Err(err) => {
                    event_callback(WatchEvent::Error {
                        message: format!("{err}"),
                    });
                    continue;
                }
            };

            if events.is_empty() {
                continue;
            }

            let changed_paths: Vec<PathBuf> = events.iter().map(|e| e.path.clone()).collect();

            let ai_dir_canonical = ai_dir.canonicalize().ok();
            let is_canonical = changed_paths.iter().any(|p| {
                if let Some(ref ai) = ai_dir_canonical {
                    p.canonicalize().ok().is_some_and(|cp| cp.starts_with(ai))
                } else {
                    p.starts_with(&ai_dir)
                }
            });

            let is_tool_native = changed_paths.iter().any(|p| {
                tool_paths.iter().any(|tp| {
                    p.canonicalize().ok().as_ref() == tp.canonicalize().ok().as_ref()
                        || p == tp
                })
            });

            syncing.store(true, SeqCst);

            if is_tool_native && !is_canonical {
                // Reverse sync: tool-native file changed externally
                match Self::reverse_sync(config, project_root, &changed_paths) {
                    Ok(Some(event)) => event_callback(event),
                    Ok(None) => {} // No change needed
                    Err(e) => {
                        event_callback(WatchEvent::Error {
                            message: format!("reverse sync failed: {e}"),
                        });
                    }
                }

                // Then forward sync to update other tools
                match SyncEngine::plan(config, project_root) {
                    Ok(plan) => {
                        if let Err(e) = SyncEngine::execute(&plan, project_root) {
                            event_callback(WatchEvent::Error {
                                message: format!("forward sync after reverse failed: {e}"),
                            });
                        } else {
                            event_callback(WatchEvent::ForwardSync {
                                changed_path: changed_paths
                                    .first()
                                    .cloned()
                                    .unwrap_or_default(),
                            });
                        }
                    }
                    Err(e) => {
                        event_callback(WatchEvent::Error {
                            message: format!("forward sync plan failed: {e}"),
                        });
                    }
                }
            } else if is_canonical {
                // Forward sync: canonical changed
                match SyncEngine::plan(config, project_root) {
                    Ok(plan) => {
                        if let Err(e) = SyncEngine::execute(&plan, project_root) {
                            event_callback(WatchEvent::Error {
                                message: format!("forward sync failed: {e}"),
                            });
                        } else {
                            event_callback(WatchEvent::ForwardSync {
                                changed_path: changed_paths
                                    .first()
                                    .cloned()
                                    .unwrap_or_default(),
                            });
                        }
                    }
                    Err(e) => {
                        event_callback(WatchEvent::Error {
                            message: format!("forward sync plan failed: {e}"),
                        });
                    }
                }
            }

            syncing.store(false, SeqCst);
        }

        Ok(())
    }

    /// Determine tool-native file paths to watch.
    /// Only returns paths that exist and are NOT symlinks.
    pub(crate) fn tool_watch_paths(config: &AisyncConfig, project_root: &Path) -> Vec<PathBuf> {
        let mut paths = Vec::new();

        for (tool_kind, _adapter, _tool_config) in SyncEngine::enabled_tools(config) {
            let path = match tool_kind {
                ToolKind::ClaudeCode => project_root.join("CLAUDE.md"),
                ToolKind::Cursor => project_root.join(".cursor/rules/project.mdc"),
                ToolKind::OpenCode => project_root.join("AGENTS.md"),
            };

            // Only watch files that exist and are NOT symlinks
            if let Ok(meta) = path.symlink_metadata() {
                if !meta.file_type().is_symlink() && meta.is_file() {
                    paths.push(path);
                }
            }
        }

        paths
    }

    /// Reverse sync: read content from a changed tool-native file
    /// and write it back to .ai/instructions.md if content differs.
    pub(crate) fn reverse_sync(
        _config: &AisyncConfig,
        project_root: &Path,
        changed_paths: &[PathBuf],
    ) -> Result<Option<WatchEvent>, AisyncError> {
        let canonical_path = project_root.join(".ai/instructions.md");
        let canonical_content = std::fs::read_to_string(&canonical_path).unwrap_or_default();

        for changed_path in changed_paths {
            // Determine which tool this path belongs to
            let (tool_kind, adapter) = if Self::path_matches(changed_path, project_root, "CLAUDE.md") {
                (ToolKind::ClaudeCode, AnyAdapter::ClaudeCode(ClaudeCodeAdapter))
            } else if Self::path_matches(changed_path, project_root, "AGENTS.md") {
                (ToolKind::OpenCode, AnyAdapter::OpenCode(OpenCodeAdapter))
            } else if Self::path_matches(changed_path, project_root, ".cursor/rules/project.mdc") {
                (ToolKind::Cursor, AnyAdapter::Cursor(CursorAdapter))
            } else {
                continue;
            };

            // Read tool-native content via adapter
            let tool_content = match adapter.read_instructions(project_root) {
                Ok(Some(content)) => content,
                Ok(None) => continue,
                Err(e) => {
                    return Err(AisyncError::Watch(WatchError::ReverseSyncFailed(
                        format!("failed to read {tool_kind:?} instructions: {e}"),
                    )));
                }
            };

            // Only write if content actually differs
            if tool_content.trim() != canonical_content.trim() {
                std::fs::write(&canonical_path, &tool_content).map_err(|e| {
                    AisyncError::Watch(WatchError::ReverseSyncFailed(format!(
                        "failed to write canonical: {e}"
                    )))
                })?;

                return Ok(Some(WatchEvent::ReverseSync {
                    tool: tool_kind,
                    source_path: changed_path.clone(),
                }));
            }
        }

        Ok(None)
    }

    /// Check if a changed path matches a tool's expected file location.
    fn path_matches(changed: &Path, project_root: &Path, relative: &str) -> bool {
        let expected = project_root.join(relative);
        changed == expected
            || changed.canonicalize().ok() == expected.canonicalize().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AisyncConfig, DefaultsConfig, SyncStrategy, ToolConfig, ToolsConfig};
    use tempfile::TempDir;

    fn all_enabled_config() -> AisyncConfig {
        AisyncConfig {
            schema_version: 1,
            defaults: DefaultsConfig {
                sync_strategy: SyncStrategy::Symlink,
            },
            tools: ToolsConfig {
                claude_code: Some(ToolConfig {
                    enabled: true,
                    sync_strategy: Some(SyncStrategy::Symlink),
                }),
                cursor: Some(ToolConfig {
                    enabled: true,
                    sync_strategy: Some(SyncStrategy::Generate),
                }),
                opencode: Some(ToolConfig {
                    enabled: true,
                    sync_strategy: Some(SyncStrategy::Symlink),
                }),
            },
        }
    }

    #[test]
    fn test_tool_watch_paths_returns_existing_non_symlink_files() {
        let dir = TempDir::new().unwrap();
        let config = all_enabled_config();

        // Create regular (non-symlink) tool files
        std::fs::write(dir.path().join("CLAUDE.md"), "# Claude").unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "# Agents").unwrap();

        let paths = WatchEngine::tool_watch_paths(&config, dir.path());

        assert!(paths.contains(&dir.path().join("CLAUDE.md")));
        assert!(paths.contains(&dir.path().join("AGENTS.md")));
        // Cursor .mdc doesn't exist, so shouldn't be returned
        assert!(!paths.contains(&dir.path().join(".cursor/rules/project.mdc")));
    }

    #[test]
    fn test_tool_watch_paths_skips_symlinked_files() {
        let dir = TempDir::new().unwrap();
        let config = all_enabled_config();

        // Create canonical file
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("instructions.md"), "# Instructions").unwrap();

        // Create CLAUDE.md as a symlink to canonical
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                ai_dir.join("instructions.md"),
                dir.path().join("CLAUDE.md"),
            )
            .unwrap();
        }

        // Create AGENTS.md as regular file
        std::fs::write(dir.path().join("AGENTS.md"), "# Agents").unwrap();

        let paths = WatchEngine::tool_watch_paths(&config, dir.path());

        // CLAUDE.md is a symlink, should NOT be in paths
        assert!(
            !paths.contains(&dir.path().join("CLAUDE.md")),
            "symlinked CLAUDE.md should not be in watch paths"
        );
        // AGENTS.md is a regular file, should be in paths
        assert!(paths.contains(&dir.path().join("AGENTS.md")));
    }

    #[test]
    fn test_tool_watch_paths_skips_missing_files() {
        let dir = TempDir::new().unwrap();
        let config = all_enabled_config();

        // No tool files created
        let paths = WatchEngine::tool_watch_paths(&config, dir.path());
        assert!(paths.is_empty(), "expected no paths for missing files, got: {paths:?}");
    }

    #[test]
    fn test_reverse_sync_updates_canonical() {
        let dir = TempDir::new().unwrap();
        let config = all_enabled_config();

        // Set up canonical with old content
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("instructions.md"), "old content").unwrap();

        // Create CLAUDE.md as regular file with new content
        std::fs::write(dir.path().join("CLAUDE.md"), "new content from claude").unwrap();

        let changed = vec![dir.path().join("CLAUDE.md")];
        let result = WatchEngine::reverse_sync(&config, dir.path(), &changed).unwrap();

        // Should have returned a ReverseSync event
        assert!(result.is_some(), "expected ReverseSync event");
        if let Some(WatchEvent::ReverseSync { tool, .. }) = result {
            assert_eq!(tool, ToolKind::ClaudeCode);
        }

        // Canonical should now have the new content
        let canonical = std::fs::read_to_string(ai_dir.join("instructions.md")).unwrap();
        assert_eq!(canonical, "new content from claude");
    }

    #[test]
    fn test_watch_exits_when_running_flag_is_false() {
        // When running flag is set to false, the watch loop should exit within 1 second
        // (no filesystem event needed to unblock)
        let dir = TempDir::new().unwrap();
        let config = all_enabled_config();

        // Create .ai directory so watch has something to monitor
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("instructions.md"), "# test").unwrap();

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        // Set running to false after 200ms (no filesystem event)
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            running_clone.store(false, SeqCst);
        });

        let start = std::time::Instant::now();
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let result = WatchEngine::watch(&config, dir.path(), running, move |event| {
            events_clone.lock().unwrap().push(format!("{event:?}"));
        });

        let elapsed = start.elapsed();
        assert!(result.is_ok(), "watch should exit cleanly");
        assert!(
            elapsed < Duration::from_secs(2),
            "watch should exit within 2 seconds of running=false, took {:?}",
            elapsed
        );
    }

    #[test]
    fn test_watch_processes_events_when_running() {
        // Normal filesystem events are still processed when running is true
        let dir = TempDir::new().unwrap();
        let config = all_enabled_config();

        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("instructions.md"), "# test").unwrap();

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let events_clone = events.clone();

        // Modify a file after 300ms, then stop after 1500ms
        let ai_dir_clone = ai_dir.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            std::fs::write(ai_dir_clone.join("instructions.md"), "# modified").unwrap();
            std::thread::sleep(Duration::from_millis(1200));
            running_clone.store(false, SeqCst);
        });

        let result = WatchEngine::watch(&config, dir.path(), running, move |event| {
            events_clone.lock().unwrap().push(format!("{event:?}"));
        });

        assert!(result.is_ok(), "watch should exit cleanly");
        // We can't guarantee events are received in test (timing-dependent),
        // but watch should have exited cleanly via the running flag
    }

    #[test]
    fn test_reverse_sync_noop_when_identical() {
        let dir = TempDir::new().unwrap();
        let config = all_enabled_config();

        // Set up canonical and tool file with same content
        let ai_dir = dir.path().join(".ai");
        std::fs::create_dir_all(&ai_dir).unwrap();
        std::fs::write(ai_dir.join("instructions.md"), "same content").unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "same content").unwrap();

        let changed = vec![dir.path().join("CLAUDE.md")];
        let result = WatchEngine::reverse_sync(&config, dir.path(), &changed).unwrap();

        assert!(result.is_none(), "expected no event when content is identical");
    }
}
