---
phase: 06-core-refactoring
plan: 03
subsystem: config
tags: [btreemap, serde-flatten, toml, config, extensible-tools]

# Dependency graph
requires:
  - phase: 06-02
    provides: "AnyAdapter::all_builtin() and for_tool() dispatch methods"
provides:
  - "BTreeMap-based ToolsConfig accepting arbitrary [tools.X] TOML sections"
  - "Helper methods: get_tool(), configured_tools(), is_enabled(), set_tool()"
  - "Refactored enabled_tools() using AnyAdapter::all_builtin() loop"
affects: [07-new-adapters, 08-plugin-sdk, 11-plugin-registry]

# Tech tracking
tech-stack:
  added: []
  patterns: [serde-flatten-btreemap, helper-method-encapsulation]

key-files:
  created: []
  modified:
    - crates/aisync-core/src/config.rs
    - crates/aisync-core/src/sync.rs
    - crates/aisync-core/src/init.rs
    - crates/aisync-core/src/watch.rs
    - crates/aisync-core/src/diff.rs
    - crates/aisync/tests/integration/helpers.rs

key-decisions:
  - "BTreeMap field is private with public helper methods to prevent direct map access"
  - "Unconfigured-is-enabled semantics preserved via is_none_or in is_enabled()"

patterns-established:
  - "ToolsConfig access pattern: always use get_tool()/is_enabled()/set_tool() helpers, never direct field access"
  - "Tool name keys match ToolKind::as_str() output (e.g., 'claude-code', 'cursor', 'opencode')"

requirements-completed: [REFAC-02]

# Metrics
duration: 4min
completed: 2026-03-08
---

# Phase 6 Plan 3: ToolsConfig BTreeMap Migration Summary

**BTreeMap-based ToolsConfig with serde flatten enabling arbitrary tool names (windsurf, codex) without config.rs changes**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-08T18:14:40Z
- **Completed:** 2026-03-08T18:18:27Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Replaced hardcoded ToolsConfig fields (claude_code, cursor, opencode) with BTreeMap<String, ToolConfig>
- Added helper methods (get_tool, configured_tools, is_enabled, set_tool) encapsulating all map access
- Refactored enabled_tools() from 3 hardcoded per-tool blocks to single AnyAdapter::all_builtin() loop
- All 202 workspace tests pass including new arbitrary tool name and round-trip tests

## Task Commits

Each task was committed atomically:

1. **Task 1: Migrate ToolsConfig to BTreeMap with helper methods** - `1052387` (feat)
2. **Task 2: Refactor enabled_tools() and all ToolsConfig callers** - `806504a` (feat)

## Files Created/Modified
- `crates/aisync-core/src/config.rs` - BTreeMap<String, ToolConfig> with #[serde(flatten)], helper methods, 14 tests
- `crates/aisync-core/src/sync.rs` - enabled_tools() refactored to AnyAdapter::all_builtin() loop
- `crates/aisync-core/src/init.rs` - Uses set_tool() with ToolKind::as_str() keys
- `crates/aisync-core/src/watch.rs` - Test helper updated to use set_tool()
- `crates/aisync-core/src/diff.rs` - Test helper updated to use set_tool()
- `crates/aisync/tests/integration/helpers.rs` - Fixed TOML key from claude_code to claude-code

## Decisions Made
- Made BTreeMap field private to enforce helper method usage pattern across codebase
- Preserved unconfigured-is-enabled semantics: is_enabled() returns true for tools not in the map

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed incorrect TOML key in integration test helpers**
- **Found during:** Task 2 (caller migration)
- **Issue:** Integration tests used `[tools.claude_code]` (underscore) instead of `[tools.claude-code]` (hyphen). Previously silently ignored by old struct's serde rename, now would create wrong BTreeMap key.
- **Fix:** Changed to `[tools.claude-code]` matching ToolKind::ClaudeCode::as_str()
- **Files modified:** crates/aisync/tests/integration/helpers.rs
- **Verification:** All 14 integration tests pass
- **Committed in:** 806504a (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Essential correctness fix. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 6 (Core Refactoring) is complete with all 3 plans delivered
- ToolAdapter trait, dispatch macro, and extensible ToolsConfig form foundation for Phase 7 (New Adapters)
- Adding a new tool adapter now requires: one adapter file, one AnyAdapter variant, one all_builtin() entry, and zero config.rs changes

---
*Phase: 06-core-refactoring*
*Completed: 2026-03-08*
