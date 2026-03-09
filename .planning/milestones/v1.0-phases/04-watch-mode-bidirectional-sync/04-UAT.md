---
status: diagnosed
phase: 04-watch-mode-bidirectional-sync
source: [04-01-SUMMARY.md, 04-02-SUMMARY.md, 04-03-SUMMARY.md]
started: 2026-03-06T14:10:00Z
updated: 2026-03-06T14:22:00Z
---

## Current Test

[testing complete]

## Tests

### 1. aisync diff — shows diffs or all-in-sync
expected: Run `cargo run -- diff`. If synced, shows "all in sync" message. If drift exists, shows colored unified diffs per tool.
result: pass

### 2. aisync check — CI-friendly exit codes
expected: Run `cargo run -- check`. Exit code 0 when all tools are in sync. Exit code 1 when drift is detected. Output is plain text (no color) for CI compatibility.
result: pass

### 3. aisync watch — starts and responds to file changes
expected: Run `cargo run -- watch`. Starts watching .ai/ and tool-native files. Shows event messages when files change. Ctrl+C gracefully stops the watcher.
result: issue
reported: "the ctrl+c didn't work, watcher goes into a loop..."
severity: blocker

### 4. Conditional sections filtered during sync
expected: Add a `<!-- aisync:cursor-only -->...<!-- /aisync:cursor-only -->` section to .ai/instructions.md. Run `cargo run -- sync`. That section appears in Cursor's rules but NOT in Claude's CLAUDE.md or OpenCode's instructions.
result: issue
reported: "fail, showed up in claude.md"
severity: major

### 5. Reverse sync — tool-native edit updates canonical
expected: Edit a non-symlinked tool-native file (e.g., .cursorrules or .opencode/instructions.md) directly. The watch engine (or a subsequent sync) detects the change and updates .ai/instructions.md with the new content.
result: issue
reported: "fail — edited tool-native file but canonical not updated, CLAUDE.md still shows old content"
severity: major

## Summary

total: 5
passed: 2
issues: 3
pending: 0
skipped: 0

## Gaps

- truth: "Ctrl+C gracefully stops the watch engine"
  status: failed
  reason: "User reported: the ctrl+c didn't work, watcher goes into a loop..."
  severity: blocker
  test: 3
  root_cause: "Watch loop uses blocking `for events in rx` (mpsc recv). The running flag check is inside the loop body, only reachable after a new filesystem event arrives. After Ctrl+C sets running=false, no events arrive so the flag is never checked."
  artifacts:
    - path: "crates/aisync-core/src/watch.rs"
      issue: "Line 67: blocking `for events in rx` iterator never yields after Ctrl+C"
    - path: "crates/aisync/src/commands/watch.rs"
      issue: "Lines 20-25: signal handler is correct, sets running=false"
  missing:
    - "Replace `for events in rx` with `recv_timeout(Duration::from_millis(500))` loop that checks running flag each iteration"
  debug_session: ".planning/debug/watch-ctrlc-infinite-loop.md"

- truth: "Conditional cursor-only section excluded from CLAUDE.md during sync"
  status: failed
  reason: "User reported: fail, showed up in claude.md"
  severity: major
  test: 4
  root_cause: "ClaudeCode adapter uses symlink strategy — CLAUDE.md symlinks to raw .ai/instructions.md. ConditionalProcessor output is passed to plan_sync() but the adapter ignores it (parameter named _canonical_content) and creates a symlink to the unprocessed source file."
  artifacts:
    - path: "crates/aisync-core/src/adapters/claude_code.rs"
      issue: "Line 59: _canonical_content ignored; lines 62-106 only create symlinks to raw .ai/instructions.md"
    - path: "crates/aisync-core/src/sync.rs"
      issue: "Lines 42-45: conditional processing correct but result discarded by symlink adapter"
  missing:
    - "When tool_content != canonical_content (conditionals applied), ClaudeCode adapter must fall back to generate/copy strategy — write processed file instead of symlinking"
  debug_session: ""

- truth: "Reverse sync updates canonical .ai/instructions.md from tool-native file edits"
  status: failed
  reason: "User reported: fail — edited tool-native file but canonical not updated, CLAUDE.md still shows old content"
  severity: major
  test: 5
  root_cause: "tool_watch_paths watches individual file paths with NonRecursive mode. Editors perform atomic saves (write temp + rename), creating new inodes that invalidate the file-level kqueue watch. After first save, watcher silently stops receiving events."
  artifacts:
    - path: "crates/aisync-core/src/watch.rs"
      issue: "Lines 54-57: watches file paths directly instead of parent directories"
    - path: "crates/aisync-core/src/watch.rs"
      issue: "Lines 177-196: tool_watch_paths returns exact file paths, not directory paths"
  missing:
    - "Watch parent directories of tool-native files instead of the files themselves, then filter events to only react to expected filenames"
    - "Keep separate list of expected tool file paths for is_tool_native matching"
  debug_session: ".planning/debug/reverse-sync-not-working.md"
