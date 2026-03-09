---
phase: 16-init-completeness
plan: 01
subsystem: core
tags: [config, sync, display, bug-fix]

# Dependency graph
requires: []
provides:
  - "Fixed is_enabled to require explicit tool listing (no ghost tools)"
  - "Present-tense SyncAction Display with dry-run 'Would:' prefix"
  - "Descriptive past-tense print_results for all common action types"
affects: [16-02, init, status, sync]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "is_some_and for opt-in tool enablement"
    - "Display trait for present-tense action descriptions, CLI layer for tense context"

key-files:
  created: []
  modified:
    - "crates/aisync-core/src/config.rs"
    - "crates/aisync-types/src/lib.rs"
    - "crates/aisync/src/commands/sync.rs"
    - "crates/aisync-core/src/sync.rs"
    - "crates/aisync-core/src/diff.rs"
    - "crates/aisync-core/src/watch.rs"

key-decisions:
  - "all_enabled_config test helpers updated to explicitly list all 5 builtins rather than relying on implicit enablement"
  - "Dry-run prefix is 'Would: ' (with colon) to clearly separate from action description"

patterns-established:
  - "Tools must be explicitly listed in aisync.toml to be enabled"
  - "SyncAction Display is present-tense; CLI layer adds context (Would: for dry-run, past-tense for results)"

requirements-completed: [INIT-02, INIT-04]

# Metrics
duration: 4min
completed: 2026-03-09
---

# Phase 16 Plan 01: Ghost Tool Filtering and Sync Action Messaging Summary

**Fixed is_enabled ghost tool bug (is_none_or to is_some_and) and removed "Would" prefix from SyncAction Display, moving tense handling to CLI layer**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-09T17:51:32Z
- **Completed:** 2026-03-09T17:55:35Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Fixed INIT-02: `is_enabled` now returns false for unconfigured tools, preventing ghost tools in status output
- Fixed INIT-04: SyncAction Display uses present tense; dry-run prepends "Would: " and real sync shows descriptive output
- Added explicit match arms in print_results for 7 additional action types (CreateFile, CreateDirectory, CreateRuleFile, WriteMcpConfig, CopyCommandFile, RemoveFile, UpdateGitignore)
- Updated all test helpers across 4 files (sync.rs, diff.rs, watch.rs, config.rs) to work with new explicit-enablement semantics

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix is_enabled ghost tool filtering (INIT-02)** - `5404d3e` (fix)
2. **Task 2: Fix SyncAction Display messaging (INIT-04)** - `a8b418d` (fix)

## Files Created/Modified
- `crates/aisync-core/src/config.rs` - Changed is_enabled from is_none_or to is_some_and
- `crates/aisync-types/src/lib.rs` - Removed "Would" prefix from SyncAction Display impl
- `crates/aisync/src/commands/sync.rs` - Added "Would: " dry-run prefix and explicit print_results match arms
- `crates/aisync-core/src/sync.rs` - Updated all_enabled_config and TOML adapter tests
- `crates/aisync-core/src/diff.rs` - Updated all_enabled_config to include all 5 builtins
- `crates/aisync-core/src/watch.rs` - Updated all_enabled_config to include all 5 builtins

## Decisions Made
- All test helpers updated to explicitly list all 5 builtin tools rather than relying on ghost-tool behavior
- Dry-run prefix uses "Would: " (with colon separator) for clear visual distinction
- TOML adapter tests now explicitly add "aider" to config since unconfigured tools are no longer auto-enabled

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed all_enabled_config in diff.rs and watch.rs**
- **Found during:** Task 1 (is_enabled fix)
- **Issue:** diff.rs and watch.rs had their own all_enabled_config helpers with only 3 tools, causing test failures after is_enabled semantics change
- **Fix:** Added windsurf and codex entries to both helpers
- **Files modified:** crates/aisync-core/src/diff.rs, crates/aisync-core/src/watch.rs
- **Verification:** All 420 aisync-core tests pass
- **Committed in:** 5404d3e (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Auto-fix necessary for test correctness. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Ghost tool filtering fixed, ready for init auto-sync in Plan 02
- Sync messaging now correctly differentiates dry-run vs real execution

---
*Phase: 16-init-completeness*
*Completed: 2026-03-09*
