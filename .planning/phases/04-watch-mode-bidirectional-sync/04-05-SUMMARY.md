---
phase: 04-watch-mode-bidirectional-sync
plan: 05
subsystem: sync
tags: [conditional-sections, adapters, claude-code, opencode, symlink, file-write]

# Dependency graph
requires:
  - phase: 04-01
    provides: ConditionalProcessor and SyncEngine conditional processing per-tool
provides:
  - ClaudeCode adapter writes processed file when conditionals strip content
  - OpenCode adapter writes processed file when conditionals strip content
  - CreateFile executor safely removes existing symlinks before writing
  - Integration tests verifying per-tool conditional filtering end-to-end
affects: [watch-mode, sync-engine, status-reporting]

# Tech tracking
tech-stack:
  added: []
  patterns: [plan_sync_with_conditionals helper pattern for symlink-based adapters]

key-files:
  modified:
    - crates/aisync-core/src/adapters/claude_code.rs
    - crates/aisync-core/src/adapters/opencode.rs
    - crates/aisync-core/src/sync.rs

key-decisions:
  - "Compare processed content against raw .ai/instructions.md to detect active conditionals"
  - "CreateFile executor removes existing symlinks to prevent writing through to canonical"
  - "Fallback to no-conditionals when raw file cannot be read (backward compat)"

patterns-established:
  - "plan_sync_with_conditionals: helper method on adapter impl for conditional content handling"

requirements-completed: [INST-08]

# Metrics
duration: 7min
completed: 2026-03-06
---

# Phase 04 Plan 05: Conditional Section Filtering for ClaudeCode and OpenCode Summary

**ClaudeCode and OpenCode adapters now write processed files when conditional sections strip content, falling back to symlinks when no conditionals apply**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-06T15:14:21Z
- **Completed:** 2026-03-06T15:21:34Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Fixed ClaudeCode adapter to use `canonical_content` parameter instead of ignoring it
- Fixed OpenCode adapter with same conditional content handling pattern
- Added 4 unit tests for ClaudeCode conditional behavior (TDD RED/GREEN)
- Added 3 integration tests verifying per-tool conditional filtering end-to-end
- CreateFile executor now safely removes existing symlinks before writing
- All 174 tests pass with no regressions

## Task Commits

Each task was committed atomically:

1. **Task 1 (RED): Failing tests for ClaudeCode conditional handling** - `4e71338` (test)
2. **Task 1 (GREEN): ClaudeCode adapter writes processed file** - `05b7a77` (feat)
3. **Task 2: OpenCode fix + integration tests** - `1829423` (feat)

_Note: Task 1 followed TDD with RED/GREEN commits._

## Files Created/Modified
- `crates/aisync-core/src/adapters/claude_code.rs` - Added plan_sync_with_conditionals helper, renamed _canonical_content to canonical_content, conditional detection via raw file comparison
- `crates/aisync-core/src/adapters/opencode.rs` - Same conditional handling pattern as ClaudeCode
- `crates/aisync-core/src/sync.rs` - Integration tests for conditional sync across all tools, regression test for symlink behavior

## Decisions Made
- Compare processed content against raw .ai/instructions.md to detect whether conditionals are active (simple string comparison, no extra parsing)
- CreateFile executor removes existing symlinks before writing to prevent corrupting canonical file through symlink
- When raw canonical file cannot be read, assume no conditionals (backward compatibility with tests that don't set up .ai/instructions.md)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed OpenCode adapter same conditional leaking bug**
- **Found during:** Task 2 (integration tests)
- **Issue:** OpenCode adapter also ignored `_canonical_content`, causing AGENTS.md symlink to point to unprocessed content
- **Fix:** Applied same pattern as ClaudeCode: plan_sync_with_conditionals helper, canonical_content comparison
- **Files modified:** crates/aisync-core/src/adapters/opencode.rs
- **Verification:** Integration tests pass, AGENTS.md correctly excludes conditional sections
- **Committed in:** 1829423 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Essential fix for correctness. Same bug pattern existed in both symlink-based adapters.

## Issues Encountered
- Git stash/pop during investigation caused loss of uncommitted Task 2 changes; re-applied from scratch without issue

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Conditional section filtering now works correctly for all three tools (ClaudeCode, Cursor, OpenCode)
- UAT failure for conditional sections should now be resolved
- Remaining gap closure plans (04-04) address watch mode issues

---
*Phase: 04-watch-mode-bidirectional-sync*
*Completed: 2026-03-06*
