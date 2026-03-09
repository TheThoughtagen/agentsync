---
phase: 03-memory-and-hooks
plan: 03
subsystem: core
tags: [hooks, toml, serde, json, adapter-translation, tdd]

requires:
  - phase: 03-memory-and-hooks
    provides: HooksConfig, HookGroup, HookHandler, HookTranslation types, HookError, translate_hooks trait method
provides:
  - HookEngine with parse, validate, list_hooks, add_hook, serialize
  - Claude Code JSON hook translation with ms-to-seconds timeout conversion
  - OpenCode JS plugin stub with event name mapping
  - Cursor unsupported hook warning
affects: [03-04-cli-wiring]

tech-stack:
  added: []
  patterns: [TDD for hook engine, serde flatten BTreeMap for TOML arrays-of-tables, adapter-specific format translation]

key-files:
  created:
    - crates/aisync-core/src/hooks.rs
  modified:
    - crates/aisync-core/src/lib.rs
    - crates/aisync-core/src/adapters/claude_code.rs
    - crates/aisync-core/src/adapters/opencode.rs
    - crates/aisync-core/src/adapters/cursor.rs

key-decisions:
  - "HookEngine as struct with associated functions matching SyncEngine and MemoryEngine patterns"
  - "TOML round-trip via serde flatten BTreeMap with toml::to_string_pretty"
  - "OpenCode event mapping: PreToolUse->tool.execute.before, PostToolUse->tool.execute.after, Stop->session.idle"

patterns-established:
  - "Hook translation pattern: each adapter overrides translate_hooks returning Supported or Unsupported"
  - "TOML arrays-of-tables via serde flatten on BTreeMap<String, Vec<T>>"

requirements-completed: [HOOK-01, HOOK-02, HOOK-03, HOOK-07]

duration: 5min
completed: 2026-03-06
---

# Phase 03 Plan 03: Hook Engine and Adapter Translations Summary

**HookEngine with TOML parse/validate/add/serialize plus Claude Code JSON, OpenCode JS plugin stub, and Cursor unsupported translations**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-06T02:34:18Z
- **Completed:** 2026-03-06T02:38:55Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- HookEngine with parse, validate, list_hooks, add_hook, serialize -- TOML round-trip verified via serde flatten BTreeMap
- Claude Code translate_hooks producing JSON with ms-to-seconds timeout conversion and null-matcher omission
- OpenCode translate_hooks producing JS plugin stub with event name mapping and unsupported event comments
- Cursor translate_hooks returning HookTranslation::Unsupported with clear reason

## Task Commits

Each task was committed atomically:

1. **Task 1: Hook engine TDD RED** - `4461bbf` (test)
2. **Task 1: Hook engine TDD GREEN** - `fca3dcf` (feat)
3. **Task 2: Adapter translation TDD RED** - `49bfc4f` (test)
4. **Task 2: Adapter translation TDD GREEN** - `fea7d5b` (feat)

_Note: TDD tasks have separate test and implementation commits_

## Files Created/Modified
- `crates/aisync-core/src/hooks.rs` - HookEngine with parse, validate, list_hooks, add_hook, serialize and 9 tests
- `crates/aisync-core/src/lib.rs` - Added hooks module declaration and exports
- `crates/aisync-core/src/adapters/claude_code.rs` - translate_hooks JSON output with 2 tests
- `crates/aisync-core/src/adapters/opencode.rs` - translate_hooks JS plugin stub with 3 tests
- `crates/aisync-core/src/adapters/cursor.rs` - translate_hooks Unsupported return with 1 test

## Decisions Made
- HookEngine as struct with associated functions (no state), consistent with SyncEngine and MemoryEngine patterns
- TOML round-trip uses serde flatten on BTreeMap<String, Vec<HookGroup>> with toml::to_string_pretty
- OpenCode event mapping: PreToolUse->tool.execute.before, PostToolUse->tool.execute.after, Stop->session.idle; Notification and SubagentStop skipped with comment

## Deviations from Plan

None - plan executed exactly as written. The adapter translate_hooks implementations were already present from prior 03-02 work in the working tree.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- HookEngine ready for CLI wiring in Plan 04
- All adapter translations tested and functional
- 139 total tests pass across aisync-core

## Self-Check: PASSED

All files and commits verified.

---
*Phase: 03-memory-and-hooks*
*Completed: 2026-03-06*
