---
phase: 07-windsurf-codex-adapters
plan: 01
subsystem: adapters
tags: [windsurf, codex, tool-adapter, enum-dispatch, detection]

# Dependency graph
requires:
  - phase: 06-core-refactoring
    provides: ToolAdapter trait with safe defaults, BTreeMap ToolsConfig, Plugin variant
provides:
  - ToolKind::Windsurf and ToolKind::Codex named enum variants
  - WindsurfAdapter with Generate strategy and YAML frontmatter
  - CodexAdapter with Symlink strategy mirroring OpenCode
  - Detection for .windsurf/rules/, .windsurfrules legacy, .codex/ directory
  - AnyAdapter dispatch for 5 built-in adapters
affects: [07-02-detection-integration, phase-08-plugin-sdk]

# Tech tracking
tech-stack:
  added: []
  patterns: [windsurf-yaml-frontmatter, codex-agents-md-symlink, idempotent-symlink-creation]

key-files:
  created:
    - crates/aisync-core/src/adapters/windsurf.rs
    - crates/aisync-core/src/adapters/codex.rs
    - fixtures/windsurf-only/.windsurf/rules/.gitkeep
    - fixtures/windsurf-legacy/.windsurfrules
    - fixtures/codex-only/.codex/.gitkeep
    - fixtures/codex-opencode/.codex/.gitkeep
    - fixtures/codex-opencode/opencode.json
  modified:
    - crates/aisync-core/src/types.rs
    - crates/aisync-core/src/adapter.rs
    - crates/aisync-core/src/adapters/mod.rs
    - crates/aisync-core/src/conditional.rs
    - crates/aisync-core/src/sync.rs
    - crates/aisync-core/src/diff.rs
    - crates/aisync-core/src/watch.rs

key-decisions:
  - "CreateSymlink made idempotent (skip if correct symlink exists) to handle Codex+OpenCode sharing AGENTS.md"
  - "Windsurf uses SyncAction::CreateFile (not GenerateMdc) since output is .md not .mdc"
  - "Codex detects only via .codex/ directory (not AGENTS.md) to avoid conflict with OpenCode medium-confidence detection"
  - "Full adapter implementations done in Task 1 alongside registration to avoid cascading test failures from stubs"

patterns-established:
  - "Windsurf frontmatter: trigger: always_on + description (distinct from Cursor's alwaysApply/globs)"
  - "Codex mirrors OpenCode symlink pattern but with distinct .codex/ detection marker"

requirements-completed: [ADPT-01, ADPT-02, ADPT-03, ADPT-05]

# Metrics
duration: 11min
completed: 2026-03-08
---

# Phase 7 Plan 1: Windsurf/Codex Adapter Registration and Implementation Summary

**First-class Windsurf and Codex ToolKind variants with full ToolAdapter implementations: Generate strategy with YAML frontmatter for Windsurf, Symlink strategy for Codex**

## Performance

- **Duration:** 11 min
- **Started:** 2026-03-08T18:52:54Z
- **Completed:** 2026-03-08T19:03:54Z
- **Tasks:** 3
- **Files modified:** 14

## Accomplishments
- Promoted Windsurf and Codex from Custom(String) to named ToolKind variants with serde support
- Implemented WindsurfAdapter with .windsurf/rules/project.md generation, YAML frontmatter, legacy .windsurfrules detection
- Implemented CodexAdapter with AGENTS.md symlink, .codex/ detection, conditional handling
- All 240 workspace tests pass (36 new adapter tests added), zero clippy warnings

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Windsurf/Codex ToolKind variants and adapter registration** - `2bbc2af` (feat)
2. **Task 2: Implement WindsurfAdapter (mirrors Cursor)** - `d3da076` (feat)
3. **Task 3: Implement CodexAdapter (mirrors OpenCode)** - `1289e98` (feat)

## Files Created/Modified
- `crates/aisync-core/src/types.rs` - Added Windsurf/Codex enum variants with as_str, display_name, serde
- `crates/aisync-core/src/adapter.rs` - Added adapter structs, AnyAdapter variants, dispatch, for_tool
- `crates/aisync-core/src/adapters/mod.rs` - Registered windsurf and codex modules
- `crates/aisync-core/src/adapters/windsurf.rs` - Full WindsurfAdapter (detect, read, plan_sync, sync_status, memory, hooks)
- `crates/aisync-core/src/adapters/codex.rs` - Full CodexAdapter (detect, read, plan_sync, sync_status, memory, hooks)
- `crates/aisync-core/src/conditional.rs` - Added windsurf-only/codex-only tag resolution
- `crates/aisync-core/src/sync.rs` - Memory status for Windsurf/Codex, idempotent CreateSymlink
- `crates/aisync-core/src/diff.rs` - Updated test counts for 5 adapters
- `crates/aisync-core/src/watch.rs` - Updated test counts for 5 adapters
- `fixtures/` - 4 new test fixture directories

## Decisions Made
- **Idempotent CreateSymlink:** Both OpenCode and Codex target AGENTS.md. Made CreateSymlink skip if correct symlink already exists, avoiding execution-order conflicts.
- **Windsurf uses CreateFile not GenerateMdc:** Windsurf generates a standard .md file (not .mdc), so CreateFile action is more appropriate than the Cursor-specific GenerateMdc.
- **Codex detection via .codex/ only:** Detecting via AGENTS.md would conflict with OpenCode's medium-confidence detection. Codex requires the .codex/ directory marker.
- **Merged stub + implementation:** Plan called for stubs in Task 1 and full implementations in Tasks 2/3, but stubs caused cascading test failures in existing tests (memory sync, execute). Implemented core functionality in Task 1, then added detect/read/hooks/tests in Tasks 2/3.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated existing tests for 5-adapter count**
- **Found during:** Task 1 (adapter registration)
- **Issue:** Existing tests in sync.rs, diff.rs, watch.rs hardcoded 3 adapters
- **Fix:** Updated assertions to 5 adapters; explicitly disabled Windsurf/Codex in skip-disabled test
- **Files modified:** sync.rs, diff.rs, watch.rs
- **Verification:** All 204+ existing tests pass

**2. [Rule 3 - Blocking] Added Windsurf/Codex memory status arms in sync.rs**
- **Found during:** Task 1 (compilation)
- **Issue:** Exhaustive match on ToolKind in memory_status lacked Windsurf/Codex arms
- **Fix:** Added memory status checks for Windsurf (.windsurf/rules/project.md) and Codex (AGENTS.md)
- **Files modified:** sync.rs
- **Verification:** cargo test passes

**3. [Rule 1 - Bug] Made CreateSymlink idempotent**
- **Found during:** Task 1 (test_execute_creates_symlinks_and_mdc failure)
- **Issue:** OpenCode and Codex both plan CreateSymlink for AGENTS.md; second execution fails with "file exists"
- **Fix:** Added check in execute_action: if symlink already points to correct target, skip
- **Files modified:** sync.rs
- **Verification:** test_execute_creates_symlinks_and_mdc passes

---

**Total deviations:** 3 auto-fixed (1 bug, 2 blocking)
**Impact on plan:** All fixes necessary for correctness with 5 adapters. No scope creep.

## Issues Encountered
None beyond the auto-fixed deviations above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Both adapters fully implemented and tested
- Ready for Plan 07-02 (detection integration, CLI tool-add support)
- All 240 workspace tests pass, zero clippy warnings

---
*Phase: 07-windsurf-codex-adapters*
*Completed: 2026-03-08*
