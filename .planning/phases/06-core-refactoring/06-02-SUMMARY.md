---
phase: 06-core-refactoring
plan: 02
subsystem: adapter
tags: [trait-expansion, dispatch-macro, plugin-architecture, refactoring]

# Dependency graph
requires:
  - phase: 06-01
    provides: "ToolKind Custom(String) variant, Clone migration"
provides:
  - "Expanded ToolAdapter trait with 6 metadata methods"
  - "dispatch_adapter! macro for zero-boilerplate AnyAdapter dispatch"
  - "AnyAdapter::Plugin(Arc<dyn ToolAdapter>) for future SDK adapters"
  - "AnyAdapter::for_tool() and all_builtin() factory methods"
  - "ToolKind::display_name() convenience method"
  - "Zero hardcoded tool-to-metadata match arms outside adapter files"
affects: [06-core-refactoring, 07-adapter-expansion, 08-plugin-sdk]

# Tech tracking
tech-stack:
  added: []
  patterns: [dispatch-macro, trait-based-metadata, plugin-via-arc]

key-files:
  modified:
    - crates/aisync-core/src/adapter.rs
    - crates/aisync-core/src/adapters/claude_code.rs
    - crates/aisync-core/src/adapters/cursor.rs
    - crates/aisync-core/src/adapters/opencode.rs
    - crates/aisync-core/src/types.rs
    - crates/aisync-core/src/conditional.rs
    - crates/aisync-core/src/diff.rs
    - crates/aisync-core/src/watch.rs
    - crates/aisync-core/src/init.rs
    - crates/aisync-core/src/detection.rs
    - crates/aisync/src/commands/init.rs
    - crates/aisync/src/commands/status.rs
    - crates/aisync/src/commands/sync.rs
    - crates/aisync/src/commands/diff.rs
    - crates/aisync/src/commands/check.rs
    - crates/aisync/src/commands/hooks.rs

key-decisions:
  - "Plugin variant uses Arc<dyn ToolAdapter> for Clone+Send+Sync compatibility"
  - "ToolKind::display_name() added as bridging pattern for call sites without adapter access"
  - "conditional.rs uses adapter structs directly (not AnyAdapter::for_tool) to avoid lifetime issues"
  - "todo!() defaults replaced with safe returns (Ok(None), Ok(vec![]), NotConfigured)"

patterns-established:
  - "dispatch_adapter! macro: adding a new AnyAdapter variant requires one match arm per variant, not O(methods)"
  - "Adapter metadata via trait methods: display_name, native_instruction_path, conditional_tags, etc."
  - "Plugin(Arc<dyn ToolAdapter>) pattern for dynamic adapter registration"

requirements-completed: [REFAC-01, REFAC-03, REFAC-04]

# Metrics
duration: 8min
completed: 2026-03-08
---

# Phase 06 Plan 02: ToolAdapter Trait Expansion Summary

**dispatch_adapter! macro and Plugin(Arc) variant enabling one-file adapter addition with zero hardcoded metadata match arms**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-08T18:03:46Z
- **Completed:** 2026-03-08T18:11:50Z
- **Tasks:** 2
- **Files modified:** 16

## Accomplishments
- Expanded ToolAdapter trait with 6 new metadata methods (display_name, native_instruction_path, conditional_tags, gitignore_entries, watch_paths, default_sync_strategy)
- Created dispatch_adapter! macro handling all 13 trait methods through 4 variants
- Added Plugin(Arc<dyn ToolAdapter>) variant for future SDK adapters with Clone via Arc
- Eliminated all 6 duplicated tool_display_name() functions across CLI commands
- Replaced all hardcoded ToolKind-to-metadata match arms in conditional.rs, diff.rs, watch.rs, init.rs
- Added safe defaults replacing todo!() panics in trait default implementations
- Added AnyAdapter::for_tool() factory and renamed all() to all_builtin()

## Task Commits

Each task was committed atomically:

1. **Task 1: Expand ToolAdapter trait, add dispatch_adapter! macro, Plugin variant** - `87d5b37` (feat)
2. **Task 2: Implement new trait methods and eliminate hardcoded metadata** - `1ca918c` (feat)

## Files Created/Modified
- `crates/aisync-core/src/adapter.rs` - Expanded trait, dispatch macro, Plugin variant, for_tool/all_builtin
- `crates/aisync-core/src/adapters/claude_code.rs` - display_name, native_instruction_path, conditional_tags
- `crates/aisync-core/src/adapters/cursor.rs` - display_name, native_instruction_path, conditional_tags, default_sync_strategy
- `crates/aisync-core/src/adapters/opencode.rs` - display_name, native_instruction_path, conditional_tags
- `crates/aisync-core/src/types.rs` - ToolKind::display_name() convenience method
- `crates/aisync-core/src/conditional.rs` - Uses adapter.conditional_tags() instead of hardcoded match
- `crates/aisync-core/src/diff.rs` - Uses adapter.native_instruction_path() instead of tool_file_name()
- `crates/aisync-core/src/watch.rs` - Uses adapter.watch_paths() and adapter-based reverse sync matching
- `crates/aisync-core/src/init.rs` - Uses AnyAdapter::for_tool() and adapter.native_instruction_path()
- `crates/aisync-core/src/detection.rs` - Updated to use all_builtin()
- `crates/aisync/src/commands/*.rs` - All 6 tool_display_name() functions removed, using ToolKind::display_name()

## Decisions Made
- Plugin variant uses Arc<dyn ToolAdapter> (not Box) for automatic Clone+Send+Sync
- Added ToolKind::display_name() as bridging pattern for CLI call sites that only have a ToolKind
- conditional.rs constructs adapter structs directly to avoid lifetime issues with AnyAdapter::for_tool()
- Replaced todo!() with safe defaults: read_instructions->Ok(None), plan_sync->Ok(vec![]), sync_status->NotConfigured

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Lifetime issue with AnyAdapter::for_tool() in conditional.rs (returned &[&str] tied to local adapter's lifetime). Resolved by constructing zero-sized adapter structs directly instead of going through AnyAdapter.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Adapter trait fully expanded with all metadata methods
- dispatch_adapter! macro ready for new variants
- Plugin variant ready for Phase 08 SDK adapters
- Zero hardcoded metadata outside adapter files -- adding a new adapter is now single-file

---
*Phase: 06-core-refactoring*
*Completed: 2026-03-08*
