---
phase: 04-watch-mode-bidirectional-sync
plan: 02
subsystem: sync
tags: [watch, notify, debounce, reverse-sync, bidirectional, file-watcher]

requires:
  - phase: 04-watch-mode-bidirectional-sync
    provides: ConditionalProcessor, DiffEngine, WatchError, workspace notify deps
provides:
  - WatchEngine with debounced file watching and bidirectional sync
  - Reverse sync from tool-native files to canonical .ai/instructions.md
  - Sync lock (AtomicBool) for infinite loop prevention
  - WatchEvent enum for event-driven logging/display
affects: [04-03, watch-command, cli]

tech-stack:
  added: [notify 8.0 (in core), notify-debouncer-mini 0.7 (in core)]
  patterns: [debounced-file-watching, sync-lock-pattern, reverse-sync-via-adapter]

key-files:
  created:
    - crates/aisync-core/src/watch.rs
  modified:
    - crates/aisync-core/Cargo.toml
    - crates/aisync-core/src/lib.rs
    - crates/aisync-core/src/types.rs

key-decisions:
  - "WatchEngine lives in aisync-core with notify deps moved from CLI crate to core"
  - "Sync lock uses AtomicBool to prevent self-triggered watch events during sync writes"
  - "Reverse sync reads via ToolAdapter::read_instructions() for consistent content parsing"
  - "Tool watch paths filter to non-symlink files only (symlinks already edit canonical)"

patterns-established:
  - "WatchEngine as struct with associated functions, matching SyncEngine/MemoryEngine pattern"
  - "Debounced watching with 500ms duration via notify-debouncer-mini"
  - "Reverse sync: tool-native -> canonical, then forward sync to propagate to other tools"

requirements-completed: [CLI-05, INST-08]

duration: 3min
completed: 2026-03-06
---

# Phase 4 Plan 02: WatchEngine with Bidirectional Sync Summary

**WatchEngine with debounced file monitoring, reverse sync from tool-native files, and AtomicBool sync lock for loop prevention**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-06T13:56:14Z
- **Completed:** 2026-03-06T13:59:04Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- WatchEngine monitors .ai/ directory and tool-native files with debounced filesystem events
- Reverse sync detects external edits to non-symlink tool files and updates .ai/instructions.md
- Sync lock prevents infinite loops from self-triggered watch events
- 5 unit tests covering tool path filtering, reverse sync content update, and no-op detection

## Task Commits

Each task was committed atomically:

1. **Task 1: Create WatchEngine with forward sync loop and sync lock** - `5064385` (feat)
2. **Task 2: Add reverse sync tests and watch engine unit tests** - `0e2cb96` (test)

## Files Created/Modified
- `crates/aisync-core/src/watch.rs` - WatchEngine with watch loop, reverse sync, tool path detection, and 5 tests
- `crates/aisync-core/src/types.rs` - Added WatchEvent enum (ForwardSync, ReverseSync, Error)
- `crates/aisync-core/src/lib.rs` - Exported watch module, WatchEngine, and WatchEvent
- `crates/aisync-core/Cargo.toml` - Added notify and notify-debouncer-mini dependencies

## Decisions Made
- WatchEngine lives in aisync-core (not CLI) since it needs SyncEngine access; notify deps moved to core
- Sync lock uses AtomicBool checked at start of each event batch to skip self-triggered events
- Reverse sync reads tool content via adapter's read_instructions() for consistent parsing (e.g., Cursor strips frontmatter)
- tool_watch_paths only returns non-symlink files -- symlinked tool files already edit canonical directly

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed notify-debouncer-mini error type handling**
- **Found during:** Task 1 (initial build)
- **Issue:** Debouncer error callback returns single notify::Error, not Vec<notify::Error>
- **Fix:** Changed `for e in errs` loop to single `err` variable handling
- **Files modified:** crates/aisync-core/src/watch.rs
- **Verification:** cargo build --workspace succeeds
- **Committed in:** 5064385 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Minor API mismatch fix. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- WatchEngine ready for CLI `aisync watch` command integration (04-03)
- All 163 tests pass including 5 new watch engine tests
- Graceful shutdown via Arc<AtomicBool> ready for ctrlc signal handler

---
*Phase: 04-watch-mode-bidirectional-sync*
*Completed: 2026-03-06*
