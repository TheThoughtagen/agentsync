---
phase: 06-core-refactoring
plan: 01
subsystem: types
tags: [toolkind, serde, clone, enum, custom-variant]

# Dependency graph
requires: []
provides:
  - "ToolKind::Custom(String) variant for arbitrary tool names"
  - "Custom Serialize/Deserialize producing lowercase hyphenated strings"
  - "ToolKind no longer derives Copy -- all usage migrated to Clone"
  - "as_str() and Display impl for ToolKind"
affects: [06-core-refactoring, 07-adapter-expansion, 10-plugin-sdk]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "ToolKind passed by reference (&ToolKind) in display functions"
    - "tool_display_name returns String (not &'static str) to support Custom variant"
    - "Custom serde: known names normalize on deserialize (claude-code -> ClaudeCode, not Custom)"

key-files:
  created: []
  modified:
    - "crates/aisync-core/src/types.rs"
    - "crates/aisync-core/src/conditional.rs"
    - "crates/aisync-core/src/diff.rs"
    - "crates/aisync-core/src/init.rs"
    - "crates/aisync-core/src/sync.rs"
    - "crates/aisync-core/src/watch.rs"
    - "crates/aisync-core/src/detection.rs"
    - "crates/aisync/src/commands/check.rs"
    - "crates/aisync/src/commands/diff.rs"
    - "crates/aisync/src/commands/hooks.rs"
    - "crates/aisync/src/commands/init.rs"
    - "crates/aisync/src/commands/status.rs"
    - "crates/aisync/src/commands/sync.rs"

key-decisions:
  - "Custom(String) variant returns empty vec for conditional tag names (no tool-specific sections)"
  - "tool_display_name changed from returning &'static str to String to support Custom variant names"
  - "Custom tools use fallback adapter (ClaudeCode) in init -- real adapter support comes in later phases"

patterns-established:
  - "Pass ToolKind by reference (&ToolKind) or clone explicitly -- never rely on implicit Copy"
  - "Match on &tool_kind when value is needed after the match"
  - "All ToolKind match expressions must include Custom(_) arm"

requirements-completed: [REFAC-01]

# Metrics
duration: 7min
completed: 2026-03-08
---

# Phase 6 Plan 1: ToolKind Custom(String) Summary

**ToolKind extended with Custom(String) variant, custom serde producing lowercase strings, and Copy-to-Clone migration across 13 files**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-08T17:53:56Z
- **Completed:** 2026-03-08T18:01:12Z
- **Tasks:** 2
- **Files modified:** 13

## Accomplishments
- ToolKind now supports arbitrary tool names via Custom(String) variant
- Custom serde serializes as clean lowercase strings ("claude-code" not "ClaudeCode")
- Known names normalize on deserialize -- "claude-code" becomes ClaudeCode, not Custom("claude-code")
- Full Copy-to-Clone migration across entire workspace with zero test failures

## Task Commits

Each task was committed atomically:

1. **Task 1: Refactor ToolKind (TDD RED)** - `5e2820b` (test)
2. **Task 1: Refactor ToolKind (TDD GREEN)** - `1169a93` (feat)
3. **Task 2: Migrate Copy to Clone** - `3121386` (feat)

_Note: Task 1 used TDD flow with separate RED and GREEN commits_

## Files Created/Modified
- `crates/aisync-core/src/types.rs` - ToolKind enum with Custom(String), custom Serialize/Deserialize, as_str(), Display
- `crates/aisync-core/src/conditional.rs` - Added Custom(_) arm returning empty tag names
- `crates/aisync-core/src/diff.rs` - Changed tool_file_name to take &ToolKind, added Custom arm
- `crates/aisync-core/src/init.rs` - Changed adapter_for_tool to take &ToolKind, added .clone() calls
- `crates/aisync-core/src/sync.rs` - Added .clone() for ToolKind in plan/execute/status, added Custom arms
- `crates/aisync-core/src/watch.rs` - Added Custom arm for watch path resolution
- `crates/aisync-core/src/detection.rs` - Added .clone() in test code
- `crates/aisync/src/commands/*.rs` - All 6 CLI commands: tool_display_name takes &ToolKind, returns String

## Decisions Made
- Custom tools get empty conditional tag names (no tool-specific content sections yet)
- Custom tools use ClaudeCode adapter as fallback in init engine (temporary until adapter registry exists)
- tool_display_name changed to return String instead of &'static str to support dynamic Custom names

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed test code Copy assumptions**
- **Found during:** Task 2 (Clone migration)
- **Issue:** Three test files used `.tool` field access from behind shared references (`.iter().map(|r| r.tool)`)
- **Fix:** Added `.clone()` calls in test code in conditional.rs, detection.rs, and sync.rs tests
- **Files modified:** conditional.rs, detection.rs, sync.rs
- **Verification:** All 181 tests pass
- **Committed in:** 3121386 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary fix for test compilation. No scope creep.

## Issues Encountered
None - compiler errors guided all changes systematically.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- ToolKind foundation ready for Plan 02 (ToolAdapter trait expansion with registry pattern)
- ToolKind foundation ready for Plan 03 (BTreeMap-based config for dynamic tool support)
- All existing functionality preserved with zero regressions

---
*Phase: 06-core-refactoring*
*Completed: 2026-03-08*
