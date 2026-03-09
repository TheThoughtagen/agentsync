# Phase 4: Watch Mode and Bidirectional Sync - Research

**Researched:** 2026-03-06
**Domain:** File watching, bidirectional sync, diff comparison, conditional content processing
**Confidence:** HIGH

## Summary

Phase 4 adds five capabilities to aisync: a file-watching daemon (`aisync watch`), reverse sync from tool-native files back to canonical `.ai/` directory, a diff viewer (`aisync diff`), a CI check command (`aisync check`), and conditional sections in instructions.md. The existing codebase has a well-structured `SyncEngine` with `plan()` and `execute()` methods, per-adapter `read_instructions()` for reading tool-native content, and `status()` for drift detection. All five requirements build naturally on this foundation.

The primary technical challenges are: (1) preventing infinite sync loops when watch mode detects its own changes, (2) handling bidirectional conflict where both canonical and tool-native files changed, and (3) parsing/stripping conditional sections per-tool. The `notify` crate (v8.2.0, MSRV 1.85 -- matching this project) with `notify-debouncer-mini` is the standard for file watching. The `similar` crate (v2.7.0) provides unified diff output for `aisync diff`.

**Primary recommendation:** Use `notify-debouncer-mini` for watch mode with a 500ms debounce window. Implement reverse sync by comparing tool-native content hash against canonical hash -- if tool file is newer and different, copy content back to `.ai/instructions.md` then forward-sync. Use a "sync lock" (in-memory flag or PID-based lockfile) to suppress re-triggering during aisync's own writes.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CLI-05 | `aisync watch` daemon that auto-syncs on file changes | notify + notify-debouncer-mini crate, watch loop architecture |
| CLI-06 | `aisync diff` to compare canonical vs tool-native files | similar crate TextDiff, per-adapter read_instructions() already exists |
| CLI-07 | `aisync check` for CI validation (exit non-zero on drift) | SyncEngine::status() already returns drift state, just needs CLI wiring |
| INST-08 | Bidirectional sync: external edits to tool-native files reverse-sync to .ai/ | Reverse sync engine, loop prevention, conflict detection |
| INST-09 | Conditional sections (`<!-- aisync:tool-only -->`) per tool | Content filter/preprocessor before plan_sync |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| notify | 8.2.0 | Cross-platform filesystem watching | De facto standard, MSRV 1.85 matches project, used by cargo-watch/rust-analyzer |
| notify-debouncer-mini | 0.7.0 | Debounce rapid FS events into single callbacks | Official companion to notify, prevents event storms |
| similar | 2.7.0 | Text diffing with unified diff output | Dependency-free, by mitsuhiko (insta author), supports line/word/char diffs |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| ctrlc | 3.4 | Graceful SIGINT/SIGTERM handling for watch daemon | Watch mode needs clean shutdown |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| notify-debouncer-mini | Custom debounce with notify raw events | More control but more code; mini is simpler and sufficient |
| similar | diffy / flickzeug | similar is more popular, dependency-free, better maintained |
| ctrlc | signal-hook | signal-hook is more powerful but ctrlc is simpler for just Ctrl+C |

**Installation (add to workspace Cargo.toml):**
```toml
[workspace.dependencies]
notify = "8.2"
notify-debouncer-mini = "0.7"
similar = "2.7"
ctrlc = "3.4"
```

## Architecture Patterns

### Recommended Module Structure
```
crates/aisync-core/src/
  watch.rs           # WatchEngine: file watcher + sync loop
  diff.rs            # DiffEngine: canonical vs tool-native comparison
  conditional.rs     # ConditionalProcessor: parse/strip tool-only sections
  sync.rs            # (existing) add reverse_sync() method

crates/aisync/src/commands/
  watch.rs           # CLI command: aisync watch
  diff.rs            # CLI command: aisync diff
  check.rs           # CLI command: aisync check
```

### Pattern 1: Watch Loop with Sync Lock
**What:** A file watcher that monitors `.ai/` and tool-native files, debounces events, then runs sync while suppressing self-triggered events.
**When to use:** CLI-05, INST-08

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};

pub struct WatchEngine {
    syncing: Arc<AtomicBool>,  // sync lock to prevent loops
}

impl WatchEngine {
    pub fn watch(config: &AisyncConfig, project_root: &Path) -> Result<(), AisyncError> {
        let syncing = Arc::new(AtomicBool::new(false));
        let syncing_clone = syncing.clone();

        let (tx, rx) = std::sync::mpsc::channel();

        let mut debouncer = new_debouncer(
            Duration::from_millis(500),
            move |res| {
                if let Ok(events) = res {
                    let _ = tx.send(events);
                }
            },
        ).map_err(|e| /* ... */)?;

        // Watch .ai/ directory (canonical source)
        debouncer.watcher().watch(
            &project_root.join(".ai"),
            notify::RecursiveMode::Recursive,
        )?;

        // Watch tool-native files for reverse sync
        for tool_path in Self::tool_watch_paths(config, project_root) {
            if tool_path.exists() {
                debouncer.watcher().watch(
                    &tool_path,
                    notify::RecursiveMode::NonRecursive,
                )?;
            }
        }

        // Event loop
        for events in rx {
            if syncing_clone.load(Ordering::SeqCst) {
                continue;  // Skip events triggered by our own writes
            }

            syncing_clone.store(true, Ordering::SeqCst);

            let is_canonical = events.iter().any(|e|
                e.path.starts_with(project_root.join(".ai"))
            );
            let is_tool_native = events.iter().any(|e|
                !e.path.starts_with(project_root.join(".ai"))
            );

            if is_tool_native && !is_canonical {
                // Reverse sync: tool file changed externally
                Self::reverse_sync(config, project_root, &events)?;
            }
            // Always forward sync after any change
            let plan = SyncEngine::plan(config, project_root)?;
            SyncEngine::execute(&plan, project_root)?;

            syncing_clone.store(false, Ordering::SeqCst);
        }

        Ok(())
    }
}
```

### Pattern 2: Reverse Sync Detection
**What:** Detect when a tool-native file (CLAUDE.md, AGENTS.md, project.mdc) was edited externally and copy its content back to `.ai/instructions.md`.
**When to use:** INST-08

Key logic:
1. On watch event for tool-native file, check if it's a symlink to `.ai/instructions.md` -- if so, the canonical file was already updated (symlink means same file), no reverse sync needed.
2. For regular files (copy strategy) or generated files (Cursor .mdc), read the tool-native content via `adapter.read_instructions()`, compare hash to canonical hash.
3. If different, write the tool-native content to `.ai/instructions.md`, then forward-sync to other tools.
4. For Cursor (.mdc), the adapter already strips frontmatter in `read_instructions()`.

**Important:** When sync strategy is Symlink, CLAUDE.md and AGENTS.md are symlinks to `.ai/instructions.md`. Editing either one edits the canonical file directly -- no reverse sync needed. Reverse sync is only relevant for:
- Tools using Copy strategy
- Cursor (always Generate strategy, so .mdc is a separate file)
- Users who replaced symlinks with regular files

### Pattern 3: Conditional Sections
**What:** Parse `<!-- aisync:claude-only -->...<!-- /aisync:claude-only -->` blocks and include/exclude them per tool.
**When to use:** INST-09

```rust
pub struct ConditionalProcessor;

impl ConditionalProcessor {
    /// Process conditional sections for a specific tool.
    /// Keeps content in matching tool sections, removes non-matching sections.
    pub fn process(content: &str, tool: ToolKind) -> String {
        let tool_tags = Self::tool_tag_names(tool);
        let mut result = String::new();
        let mut skip_depth = 0;

        for line in content.lines() {
            if let Some(tag) = Self::parse_open_tag(line) {
                if tool_tags.contains(&tag.as_str()) {
                    // This section IS for this tool -- include content
                    continue; // Skip the marker line itself
                } else {
                    // This section is NOT for this tool -- skip content
                    skip_depth += 1;
                    continue;
                }
            }
            if let Some(tag) = Self::parse_close_tag(line) {
                if !tool_tags.contains(&tag.as_str()) && skip_depth > 0 {
                    skip_depth -= 1;
                }
                continue; // Skip marker lines
            }
            if skip_depth == 0 {
                result.push_str(line);
                result.push('\n');
            }
        }
        result
    }

    fn tool_tag_names(tool: ToolKind) -> Vec<&'static str> {
        match tool {
            ToolKind::ClaudeCode => vec!["claude-only", "claude-code-only"],
            ToolKind::Cursor => vec!["cursor-only"],
            ToolKind::OpenCode => vec!["opencode-only"],
        }
    }
}
```

Marker format: `<!-- aisync:claude-only -->` / `<!-- /aisync:claude-only -->`

### Pattern 4: Diff Engine
**What:** Compare canonical `.ai/instructions.md` against each tool's native file and show unified diff.
**When to use:** CLI-06

```rust
use similar::TextDiff;

pub struct DiffEngine;

impl DiffEngine {
    pub fn diff_all(config: &AisyncConfig, project_root: &Path) -> Vec<ToolDiff> {
        let canonical = std::fs::read_to_string(
            project_root.join(".ai/instructions.md")
        ).unwrap_or_default();

        let mut diffs = Vec::new();
        for (tool_kind, adapter, _) in SyncEngine::enabled_tools(config) {
            if let Ok(Some(tool_content)) = adapter.read_instructions(project_root) {
                let diff = TextDiff::from_lines(&canonical, &tool_content);
                let unified = diff.unified_diff()
                    .context_radius(3)
                    .header(".ai/instructions.md", &tool_file_name(tool_kind))
                    .to_string();
                diffs.push(ToolDiff {
                    tool: tool_kind,
                    has_changes: diff.ratio() < 1.0,
                    unified_diff: unified,
                });
            }
        }
        diffs
    }
}
```

### Pattern 5: Check Command (CI Mode)
**What:** Non-interactive sync status check that exits non-zero on drift. Essentially `aisync status --json` with exit code logic but cleaner CI output.
**When to use:** CLI-07

This is straightforward -- reuse `SyncEngine::status()` and return exit code 1 if `!status.all_in_sync()`. The existing `status` command with `--json` almost does this already (it exits 1 on drift), but `check` should be purpose-built for CI with:
- Machine-readable output (JSON by default or simple text)
- Clear exit codes: 0 = all synced, 1 = drift detected, 2 = error
- No color codes in output (CI friendly)

### Anti-Patterns to Avoid
- **Polling instead of inotify/FSEvents:** Never use `loop { sleep; check_files }`. The `notify` crate uses OS-native APIs.
- **Watching without debounce:** Raw FS events fire multiple times per save. Always debounce.
- **Recursive watch on project root:** Only watch specific paths (`.ai/`, tool-native files). Watching `.` recursively catches unrelated changes and wastes resources.
- **Sync lock via filesystem lock:** Use in-memory `AtomicBool`. Filesystem locks add complexity and failure modes (stale locks).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| File watching | Custom polling loop | notify + notify-debouncer-mini | Cross-platform (inotify, FSEvents, kqueue), handles edge cases |
| Text diffing | Line-by-line comparison | similar::TextDiff | Unified diff format, configurable context, handles Unicode |
| Debouncing | Custom timer/hash dedup | notify-debouncer-mini | Handles event dedup, timing, edge cases per platform |
| Signal handling | Raw libc signal handling | ctrlc crate | Cross-platform, safe Rust API |

## Common Pitfalls

### Pitfall 1: Infinite Sync Loop
**What goes wrong:** Watch detects change to CLAUDE.md, syncs, which changes .mdc, watch detects .mdc change, syncs again, forever.
**Why it happens:** File watcher fires on ALL writes, including those made by aisync itself.
**How to avoid:** Use an `AtomicBool` sync lock. Set it before writing, clear it after. Skip all events while the lock is held. The debounce window (500ms) helps too -- events from our writes arrive within the debounce window and get collapsed.
**Warning signs:** CPU spins to 100%, logs show continuous sync cycles.

### Pitfall 2: Symlink Confusion in Watch Mode
**What goes wrong:** Watching a symlink (CLAUDE.md -> .ai/instructions.md) may not fire events when the target is edited, depending on OS and notify backend.
**Why it happens:** Some backends watch the symlink itself, not the target. macOS FSEvents watches the target. Linux inotify may not.
**How to avoid:** Watch the `.ai/` directory directly (the canonical source), not the symlinks. For reverse sync, watch only non-symlink tool files.
**Warning signs:** Edits to .ai/instructions.md don't trigger sync on Linux.

### Pitfall 3: Reverse Sync with Symlinks is a No-Op
**What goes wrong:** Developer implements reverse sync logic for CLAUDE.md but it's a symlink -- editing CLAUDE.md already edits .ai/instructions.md.
**Why it happens:** Forgetting that symlink strategy means the files are the same file.
**How to avoid:** In reverse sync logic, first check if the tool file is a symlink. If it points to .ai/instructions.md, skip reverse sync -- the canonical file was already updated.

### Pitfall 4: Debounce Too Short or Too Long
**What goes wrong:** At <100ms, multiple events still fire separately. At >2s, users perceive lag.
**Why it happens:** Different editors save files differently (vim writes to temp then renames, VS Code writes in-place).
**How to avoid:** Use 500ms as default debounce. This is the sweet spot used by most file watchers (cargo-watch, watchexec).

### Pitfall 5: Conditional Section Nesting
**What goes wrong:** User nests `<!-- aisync:claude-only -->` inside `<!-- aisync:cursor-only -->` and gets unexpected behavior.
**Why it happens:** Naive line-by-line parser doesn't track nesting depth correctly.
**How to avoid:** Track skip_depth as a counter. Document that nesting is not supported (or handle it properly with depth tracking). Recommend flat conditional sections only.

### Pitfall 6: Race Condition in Reverse Sync
**What goes wrong:** Two tool files change simultaneously, both try to write to .ai/instructions.md.
**Why it happens:** Unlikely in practice (one developer editing two tools at once) but possible.
**How to avoid:** Process events serially. Use a "last writer wins" strategy with timestamp comparison. Log a warning when conflicting changes are detected.

## Code Examples

### Watch Command CLI Integration
```rust
// crates/aisync/src/commands/watch.rs
pub fn run_watch(verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = AisyncConfig::from_file(Path::new("aisync.toml"))?;
    let project_root = Path::new(".");

    println!("Watching for changes... (Ctrl+C to stop)");

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })?;

    WatchEngine::watch(&config, project_root, running, verbose)?;

    println!("\nStopped watching.");
    Ok(())
}
```

### Diff Command Output
```rust
// crates/aisync/src/commands/diff.rs
pub fn run_diff(verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = AisyncConfig::from_file(Path::new("aisync.toml"))?;
    let project_root = Path::new(".");

    let diffs = DiffEngine::diff_all(&config, project_root)?;

    let mut any_diff = false;
    for tool_diff in &diffs {
        if tool_diff.has_changes {
            any_diff = true;
            println!("--- {} ---", tool_display_name(tool_diff.tool));
            println!("{}", tool_diff.unified_diff);
        } else if verbose {
            println!("{}: in sync", tool_display_name(tool_diff.tool));
        }
    }

    if !any_diff {
        println!("All tools in sync with .ai/instructions.md");
    }

    Ok(())
}
```

### Check Command (CI)
```rust
// crates/aisync/src/commands/check.rs
pub fn run_check(verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = AisyncConfig::from_file(Path::new("aisync.toml"))?;
    let project_root = Path::new(".");

    let status = SyncEngine::status(&config, project_root)?;

    if status.all_in_sync() {
        println!("OK: all tools in sync");
        Ok(())
    } else {
        let drifted: Vec<_> = status.tools.iter()
            .filter(|t| !matches!(t.drift, DriftState::InSync | DriftState::NotConfigured))
            .collect();
        for t in &drifted {
            eprintln!("DRIFT: {:?} - {:?}", t.tool, t.drift);
        }
        std::process::exit(1);
    }
}
```

### Conditional Section Processing
```rust
// Test for conditional processing
#[test]
fn test_conditional_claude_only() {
    let content = "# Common\n\
        <!-- aisync:claude-only -->\n\
        Claude-specific content\n\
        <!-- /aisync:claude-only -->\n\
        \n\
        <!-- aisync:cursor-only -->\n\
        Cursor-specific content\n\
        <!-- /aisync:cursor-only -->\n\
        \n\
        # More common";

    let claude_result = ConditionalProcessor::process(content, ToolKind::ClaudeCode);
    assert!(claude_result.contains("Claude-specific content"));
    assert!(!claude_result.contains("Cursor-specific content"));
    assert!(claude_result.contains("# Common"));
    assert!(claude_result.contains("# More common"));

    let cursor_result = ConditionalProcessor::process(content, ToolKind::Cursor);
    assert!(!cursor_result.contains("Claude-specific content"));
    assert!(cursor_result.contains("Cursor-specific content"));
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| notify v5 with `watcher::Watcher` | notify v8 with `recommended_watcher()` | v6.0 (2023) | Simplified API, better cross-platform |
| Manual debounce with timers | notify-debouncer-mini | v0.3+ (2023) | Official companion, handles edge cases |
| diff crate | similar crate | 2021+ | Better API, actively maintained, used by insta |

## Open Questions

1. **Conflict resolution UI for bidirectional sync**
   - What we know: Success criterion says "reverse-syncs to .ai/instructions.md" suggesting auto-merge
   - What's unclear: What happens when BOTH canonical and tool-native change between syncs?
   - Recommendation: For v1, use "last writer wins" with a warning. ADV-01 (interactive TUI conflict resolution) is explicitly deferred to v2. Log the conflict and let the user resolve manually.

2. **Watch mode on Windows**
   - What we know: notify supports Windows via ReadDirectoryChanges. Symlinks may not work.
   - What's unclear: Whether Windows users will hit issues with symlink watching.
   - Recommendation: Watch mode should work on Windows since we watch .ai/ directory (not symlinks). Test on Windows is Phase 5 (DIST-05).

3. **Memory and hook file watching**
   - What we know: Success criteria focus on instructions sync.
   - What's unclear: Should watch also trigger on .ai/memory/ or .ai/hooks.toml changes?
   - Recommendation: Yes, watch should cover .ai/ recursively which includes memory and hooks. This falls out naturally from watching `.ai/` recursively.

## Sources

### Primary (HIGH confidence)
- [notify 8.2.0 docs](https://docs.rs/notify/latest/notify/) - API, RecommendedWatcher, event types
- [notify-debouncer-mini 0.7.0 docs](https://docs.rs/notify-debouncer-mini/latest/notify_debouncer_mini/) - Debouncer API, DebouncedEvent
- [similar 2.7.0 TextDiff docs](https://docs.rs/similar/latest/similar/struct.TextDiff.html) - Diff API, unified_diff()
- [notify GitHub](https://github.com/notify-rs/notify) - MSRV 1.85, project status

### Secondary (MEDIUM confidence)
- [Crate downloads / usage data](https://crates.io/crates/notify) - 62.7M downloads, widely adopted

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - notify is the only serious choice for Rust file watching, similar is well-established
- Architecture: HIGH - builds directly on existing SyncEngine patterns, adapter trait already has read_instructions()
- Pitfalls: HIGH - sync loops and symlink behavior are well-documented problems in file watcher implementations
- Conditional sections: MEDIUM - straightforward parsing, but edge cases in nesting and whitespace handling need testing

**Research date:** 2026-03-06
**Valid until:** 2026-04-06 (stable ecosystem, unlikely to change)
