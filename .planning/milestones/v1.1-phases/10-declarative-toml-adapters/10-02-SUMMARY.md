---
phase: 10-declarative-toml-adapters
plan: 02
subsystem: adapters
tags: [toml, plugin-sdk, declarative-adapters, discovery, sync-engine, detection]

requires:
  - phase: 10-01
    provides: "DeclarativeAdapter, load_toml_adapter, TOML schema"
provides:
  - "discover_toml_adapters() function for scanning .ai/adapters/*.toml"
  - "SyncEngine integration with TOML adapters via Plugin variant"
  - "DetectionEngine integration with TOML adapters (non-fatal errors)"
  - "Strategy fallback to adapter.default_sync_strategy() for unconfigured tools"
affects: [phase-11-plugin-registry]

tech-stack:
  added: []
  patterns: ["discover-and-wrap pattern for TOML adapter auto-loading", "non-fatal error handling for user-provided adapters"]

key-files:
  created: []
  modified:
    - crates/aisync-core/src/declarative.rs
    - crates/aisync-core/src/sync.rs
    - crates/aisync-core/src/detection.rs
    - crates/aisync-core/src/lib.rs
    - crates/aisync-core/src/diff.rs
    - crates/aisync-core/src/watch.rs

key-decisions:
  - "Strategy fallback uses adapter.default_sync_strategy() instead of config.defaults when no tool_config exists"
  - "TOML adapter detection errors are non-fatal (eprintln warning), unlike builtin adapter errors which return Err"

patterns-established:
  - "discover_toml_adapters() + AnyAdapter::Plugin(Arc::new(...)) wrapping pattern for runtime adapter integration"
  - "Graceful skip for malformed/colliding TOML files in discovery"

requirements-completed: [SDK-05]

duration: 4min
completed: 2026-03-09
---

# Phase 10 Plan 02: TOML Adapter Discovery & Integration Summary

**discover_toml_adapters() auto-loads .ai/adapters/*.toml into SyncEngine and DetectionEngine via Plugin(Arc) wrapping with graceful malformed-file handling**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-09T03:38:30Z
- **Completed:** 2026-03-09T03:42:37Z
- **Tasks:** 1 (TDD: RED + GREEN)
- **Files modified:** 6

## Accomplishments
- discover_toml_adapters() scans .ai/adapters/ and returns parsed DeclarativeAdapters, skipping malformed files
- SyncEngine::enabled_tools() extended with project_root param to include TOML adapters alongside builtins
- DetectionEngine::scan() extended to detect TOML-defined tools with non-fatal error handling
- Strategy fallback corrected: unconfigured adapters use their own default_sync_strategy() instead of global config default
- 11 new tests covering discovery, sync integration, detection integration, and config filtering

## Task Commits

Each task was committed atomically:

1. **Task 1 (RED): Failing tests** - `6caf454` (test)
2. **Task 1 (GREEN): Implementation** - `59443b8` (feat)

_TDD task with RED/GREEN commits._

## Files Created/Modified
- `crates/aisync-core/src/declarative.rs` - Added discover_toml_adapters() and 6 discovery tests
- `crates/aisync-core/src/sync.rs` - Extended enabled_tools() with project_root, fixed strategy fallback, added 5 integration tests
- `crates/aisync-core/src/detection.rs` - Extended scan() with TOML adapter detection, added 2 tests
- `crates/aisync-core/src/lib.rs` - Re-exported discover_toml_adapters
- `crates/aisync-core/src/diff.rs` - Updated enabled_tools() call site
- `crates/aisync-core/src/watch.rs` - Updated enabled_tools() call site

## Decisions Made
- Strategy fallback uses adapter.default_sync_strategy() instead of config.defaults.sync_strategy when no tool_config exists -- this ensures TOML adapters that specify "generate" strategy actually use it without requiring explicit config entries
- TOML adapter detection errors are non-fatal (eprintln warning) unlike builtin adapter errors which return Err -- user-provided adapters should fail gracefully

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed strategy fallback for unconfigured adapters**
- **Found during:** Task 1 (GREEN phase, test_plan_includes_toml_adapter_generate failing)
- **Issue:** When no tool_config exists for a TOML adapter, plan_all_internal used config.defaults.sync_strategy (Symlink) instead of the adapter's own default_sync_strategy()
- **Fix:** Changed .unwrap_or(config.defaults.sync_strategy) to .unwrap_or_else(|| adapter.default_sync_strategy()) in both plan_all_internal() and status()
- **Files modified:** crates/aisync-core/src/sync.rs
- **Verification:** test_plan_includes_toml_adapter_generate passes; all 297 aisync-core tests pass
- **Committed in:** 59443b8 (Task 1 GREEN commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Essential fix for correct TOML adapter behavior. No scope creep.

## Issues Encountered
- Lifetime annotation needed on enabled_tools() return type after adding second &Path parameter -- Rust compiler required explicit `<'a>` to disambiguate which reference the returned &ToolConfig borrows from

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Phase 10 complete: DeclarativeAdapter (plan 01) + auto-discovery & integration (plan 02) fully operational
- Users can drop .toml files in .ai/adapters/ and they are auto-discovered during sync and status
- Ready for Phase 11: Plugin registry and inventory integration

---
*Phase: 10-declarative-toml-adapters*
*Completed: 2026-03-09*
