---
phase: 08-add-tool-command
plan: 01
subsystem: core
tags: [add-tool, detection, sync, toml, partial-sync]

# Dependency graph
requires:
  - phase: 07-windsurf-codex-adapters
    provides: "Windsurf and Codex adapters with detection, sync, and deduplication"
provides:
  - "AddToolEngine with discover_unconfigured() and add_tools() methods"
  - "SyncEngine::plan_for_tools() for partial sync of specified tools"
affects: [08-add-tool-command]

# Tech tracking
tech-stack:
  added: []
  patterns: ["plan_all_internal extraction pattern for plan reuse", "get_tool().is_none() check for unconfigured detection"]

key-files:
  created:
    - crates/aisync-core/src/add_tool.rs
  modified:
    - crates/aisync-core/src/lib.rs
    - crates/aisync-core/src/sync.rs

key-decisions:
  - "Reuse InitError for add_tool errors (ScaffoldFailed for IO, ImportFailed for serialization) -- follows init precedent"
  - "Omit sync_strategy from TOML when default is Symlink (keeps config clean)"
  - "plan_for_tools runs full plan then filters results (simplest correct approach preserving deduplication)"

patterns-established:
  - "plan_all_internal: shared plan logic extracted for reuse by plan() and plan_for_tools()"
  - "get_tool().is_none() for unconfigured check: avoids unconfigured-is-enabled semantic trap"

requirements-completed: [TOOL-01, TOOL-03, TOOL-04]

# Metrics
duration: 3min
completed: 2026-03-08
---

# Phase 08 Plan 01: Add Tool Core Engine Summary

**AddToolEngine with discover/add methods and SyncEngine::plan_for_tools() for partial sync with full deduplication**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-08T19:47:01Z
- **Completed:** 2026-03-08T19:49:35Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- AddToolEngine::discover_unconfigured correctly identifies detected-but-not-configured tools using get_tool().is_none()
- AddToolEngine::add_tools writes valid aisync.toml with per-tool sync_strategy (Generate for Windsurf, None/Symlink for Claude)
- SyncEngine::plan_for_tools filters results to requested tools while running full deduplication across all enabled tools
- 11 new tests (7 add_tool + 4 plan_for_tools), all 257 core tests pass, zero clippy warnings

## Task Commits

Each task was committed atomically:

1. **Task 1: AddToolEngine with discover_unconfigured and add_tools** - `cd26f35` (feat)
2. **Task 2: SyncEngine::plan_for_tools partial sync method** - `7bd36d2` (feat)

## Files Created/Modified
- `crates/aisync-core/src/add_tool.rs` - AddToolEngine with discover_unconfigured and add_tools methods
- `crates/aisync-core/src/lib.rs` - Module declaration and pub use export for AddToolEngine
- `crates/aisync-core/src/sync.rs` - Refactored plan() into plan_all_internal(), added plan_for_tools()

## Decisions Made
- Reused InitError variants for add_tool error mapping (follows init precedent of writing aisync.toml)
- Omit sync_strategy from ToolConfig when adapter default is Symlink (global default) for clean TOML
- Extracted plan_all_internal() rather than duplicating logic between plan() and plan_for_tools()

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Core AddToolEngine and partial sync ready for CLI integration in 08-02
- All existing tests pass with no regressions

---
*Phase: 08-add-tool-command*
*Completed: 2026-03-08*
