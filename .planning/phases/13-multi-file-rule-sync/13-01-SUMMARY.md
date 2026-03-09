---
phase: 13-multi-file-rule-sync
plan: 01
subsystem: sync
tags: [rules, frontmatter, cursor, windsurf, yaml-parsing]

requires:
  - phase: 12-types-trait-foundation
    provides: RuleFile, RuleMetadata types, plan_rules_sync trait method, SyncAction::CreateRuleFile variant
provides:
  - RuleEngine with load() and parse_frontmatter() for .ai/rules/*.md
  - CursorAdapter plan_rules_sync generating aisync-{name}.mdc files
  - WindsurfAdapter plan_rules_sync generating aisync-{name}.md files
  - Stale aisync-* managed file cleanup for both adapters
  - Rule loading wired into SyncEngine::plan_all_internal()
affects: [13-02, 14-mcp-config-sync, 15-command-sync]

tech-stack:
  added: []
  patterns: [hand-parsed YAML frontmatter, aisync- prefix for managed files, per-adapter rule frontmatter translation]

key-files:
  created:
    - crates/aisync-core/src/rules.rs
  modified:
    - crates/aisync-core/src/lib.rs
    - crates/aisync-core/src/sync.rs
    - crates/aisync-core/src/adapters/cursor.rs
    - crates/aisync-core/src/adapters/windsurf.rs

key-decisions:
  - "Hand-parse YAML frontmatter instead of adding serde_yml dependency -- frontmatter is simple key-value, controlled schema"
  - "Empty frontmatter handled by checking for immediate closing --- after opening delimiter"

patterns-established:
  - "Rule frontmatter translation: each adapter maps canonical RuleMetadata to tool-native YAML fields"
  - "Stale file cleanup: scan for aisync-* prefix files not in expected set, emit RemoveFile actions"
  - "Idempotent rule sync: compare generated content to existing file, skip if identical"

requirements-completed: [RULES-01, RULES-02, RULES-03, RULES-06, RULES-07]

duration: 6min
completed: 2026-03-09
---

# Phase 13 Plan 01: Rule Engine & Adapter Rule Sync Summary

**RuleEngine loads .ai/rules/*.md with YAML frontmatter parsing, CursorAdapter generates aisync-*.mdc with camelCase frontmatter, WindsurfAdapter generates aisync-*.md with trigger-type frontmatter, both with stale file cleanup**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-09T15:18:59Z
- **Completed:** 2026-03-09T15:25:21Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- RuleEngine loads and parses .ai/rules/*.md with YAML frontmatter (description, globs, always_apply) using hand-parser
- CursorAdapter generates aisync-{name}.mdc files with Cursor-format frontmatter (description, globs as comma-separated string, alwaysApply camelCase)
- WindsurfAdapter generates aisync-{name}.md files with Windsurf trigger types (always_on, glob, model_decision, manual)
- Both adapters scan for and remove stale aisync-* managed files not in current rule set
- SyncEngine wires rule loading into plan_all_internal with graceful error handling
- 34 new tests covering rule loading, frontmatter parsing, adapter generation, stale cleanup, idempotency

## Task Commits

Each task was committed atomically:

1. **Task 1: Create RuleEngine module and wire into sync pipeline** - `10e12ed` (feat)
2. **Task 2: Implement CursorAdapter and WindsurfAdapter plan_rules_sync** - `bf3cf7d` (feat)

## Files Created/Modified
- `crates/aisync-core/src/rules.rs` - RuleEngine with load(), parse_frontmatter(), parse_yaml_metadata()
- `crates/aisync-core/src/lib.rs` - Added rules module declaration and RuleEngine re-export
- `crates/aisync-core/src/sync.rs` - Wired rule loading and per-adapter plan_rules_sync dispatch
- `crates/aisync-core/src/adapters/cursor.rs` - Added plan_rules_sync, generate_cursor_rule_frontmatter/content helpers
- `crates/aisync-core/src/adapters/windsurf.rs` - Added plan_rules_sync, generate_windsurf_rule_frontmatter/content helpers

## Decisions Made
- Hand-parse YAML frontmatter instead of adding serde_yml dependency -- canonical format is controlled by aisync, simple key-value only
- Handle empty frontmatter (---\n---) as a special case by checking for immediate closing delimiter after opening

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed empty frontmatter parsing edge case**
- **Found during:** Task 1 (RuleEngine implementation)
- **Issue:** `parse_frontmatter("---\n---\nBody")` failed because after stripping the opening `---\n`, the remaining `---\nBody` has the closing `---` at position 0, not preceded by `\n`, so `find("\n---")` didn't match
- **Fix:** Added explicit check for `strip_prefix("---")` on the after-open content before the general `\n---` search
- **Files modified:** crates/aisync-core/src/rules.rs
- **Verification:** Test `test_parse_frontmatter_handles_empty_frontmatter` passes
- **Committed in:** 10e12ed (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Essential edge case fix for correctness. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Rule engine and multi-file adapter sync complete for Cursor and Windsurf
- Plan 02 can implement single-file tool concatenation (ClaudeCode/OpenCode/Codex) and rule import during init
- All 335+ workspace tests pass

---
*Phase: 13-multi-file-rule-sync*
*Completed: 2026-03-09*
