---
phase: 04-watch-mode-bidirectional-sync
plan: 01
subsystem: sync
tags: [conditional-sections, diff, similar, notify, ctrlc]

requires:
  - phase: 03-memory-and-hooks
    provides: SyncEngine with memory and hooks, ToolAdapter trait
provides:
  - ConditionalProcessor for tool-specific instruction sections
  - DiffEngine for canonical vs tool-native unified diffs
  - WatchError type for watch mode error handling
  - Workspace deps notify, similar, ctrlc for future watch mode
affects: [04-02, 04-03, watch-mode, diff-command, check-command]

tech-stack:
  added: [similar 2.7, notify 8.0, notify-debouncer-mini 0.7, ctrlc 3.4]
  patterns: [conditional-section-processing, unified-diff-generation]

key-files:
  created:
    - crates/aisync-core/src/conditional.rs
    - crates/aisync-core/src/diff.rs
  modified:
    - Cargo.toml
    - crates/aisync-core/Cargo.toml
    - crates/aisync/Cargo.toml
    - crates/aisync-core/src/lib.rs
    - crates/aisync-core/src/types.rs
    - crates/aisync-core/src/error.rs
    - crates/aisync-core/src/sync.rs

key-decisions:
  - "ConditionalProcessor uses line-by-line parsing with skip_depth counter for nested tag handling"
  - "DiffEngine compares conditionally-processed canonical content (not raw) against tool-native content"
  - "SyncEngine::plan() applies ConditionalProcessor per-tool before adapter.plan_sync()"
  - "enabled_tools changed to pub(crate) for DiffEngine cross-module access"

patterns-established:
  - "Conditional section markers: <!-- aisync:TAG --> / <!-- /aisync:TAG --> with claude-only, cursor-only, opencode-only tags"
  - "DiffEngine uses similar crate TextDiff::from_lines with context_radius(3) for unified diffs"

requirements-completed: [INST-09, CLI-06]

duration: 3min
completed: 2026-03-06
---

# Phase 4 Plan 01: ConditionalProcessor and DiffEngine Foundation Summary

**ConditionalProcessor for tool-specific instruction sections and DiffEngine for unified diff comparison using similar crate**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-06T13:50:05Z
- **Completed:** 2026-03-06T13:53:24Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- ConditionalProcessor strips/includes tool-specific sections per ToolKind with 13 tests
- DiffEngine computes unified diffs between canonical and tool-native files with 3 tests
- SyncEngine::plan() now applies conditional processing before generating tool-native content
- Workspace deps (notify, similar, ctrlc) available for future watch mode and CLI commands

## Task Commits

Each task was committed atomically:

1. **Task 1: Add workspace deps, create ConditionalProcessor with tests** - `3c4ccaf` (feat)
2. **Task 2: Create DiffEngine and wire conditional processing into SyncEngine** - `60bbd29` (feat)

## Files Created/Modified
- `crates/aisync-core/src/conditional.rs` - ConditionalProcessor with line-by-line tag parser and 13 tests
- `crates/aisync-core/src/diff.rs` - DiffEngine with diff_all() and 3 tests
- `crates/aisync-core/src/types.rs` - Added ToolDiff struct
- `crates/aisync-core/src/error.rs` - Added WatchError enum and AisyncError::Watch variant
- `crates/aisync-core/src/sync.rs` - Wired ConditionalProcessor into plan(), added conditional test
- `crates/aisync-core/src/lib.rs` - Exported conditional, diff modules and new types
- `Cargo.toml` - Added notify, notify-debouncer-mini, similar, ctrlc workspace deps
- `crates/aisync-core/Cargo.toml` - Added similar dependency
- `crates/aisync/Cargo.toml` - Added notify, notify-debouncer-mini, ctrlc dependencies

## Decisions Made
- ConditionalProcessor uses line-by-line parsing with skip_depth counter -- simple, handles nesting by treating inner tags as text when inside a skipped section
- DiffEngine compares conditionally-processed canonical content against tool-native files, not raw canonical
- SyncEngine::plan() applies ConditionalProcessor per-tool before adapter.plan_sync() -- each tool gets filtered instructions
- enabled_tools changed from fn to pub(crate) fn for DiffEngine cross-module access

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- ConditionalProcessor and DiffEngine ready for CLI diff/check commands (04-02)
- Workspace deps ready for watch mode implementation (04-03)
- All 158 tests pass including 17 new tests

---
*Phase: 04-watch-mode-bidirectional-sync*
*Completed: 2026-03-06*
