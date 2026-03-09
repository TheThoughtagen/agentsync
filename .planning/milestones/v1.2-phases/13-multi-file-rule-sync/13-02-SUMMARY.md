---
phase: 13-multi-file-rule-sync
plan: 02
subsystem: sync
tags: [rules, concatenation, import, cursor, windsurf, frontmatter]

requires:
  - phase: 13-multi-file-rule-sync
    plan: 01
    provides: RuleEngine, RuleFile/RuleMetadata types, Cursor/Windsurf adapter plan_rules_sync
provides:
  - ClaudeCodeAdapter plan_rules_sync concatenating rules into CLAUDE.md managed section
  - OpenCodeAdapter plan_rules_sync concatenating rules into AGENTS.md managed section
  - CodexAdapter plan_rules_sync concatenating rules into AGENTS.md managed section
  - import_rules() in InitEngine for importing Cursor .mdc and Windsurf .md rules during init
  - Shared plan_single_file_rules_sync helper for single-file tool adapters
affects: [14-mcp-config-sync, 15-command-sync]

tech-stack:
  added: []
  patterns: [shared single-file rule concatenation helper, frontmatter translation during import, split_frontmatter utility]

key-files:
  created: []
  modified:
    - crates/aisync-core/src/adapters/mod.rs
    - crates/aisync-core/src/adapters/claude_code.rs
    - crates/aisync-core/src/adapters/opencode.rs
    - crates/aisync-core/src/adapters/codex.rs
    - crates/aisync-core/src/init.rs

key-decisions:
  - "Shared helper plan_single_file_rules_sync in adapters/mod.rs avoids code duplication across three adapters"
  - "Rule content concatenated with '## Rule: {name}' headers for readability in single-file tools"

patterns-established:
  - "Single-file tools use UpdateMemoryReferences with aisync:rules markers for rule concatenation"
  - "Import translation: Cursor alwaysApply -> always_apply, Windsurf trigger types -> always_apply boolean"

requirements-completed: [RULES-04, RULES-05]

duration: 4min
completed: 2026-03-09
---

# Phase 13 Plan 02: Single-File Rule Sync & Rule Import Summary

**Claude Code/OpenCode/Codex concatenate rules into managed sections, aisync init imports Cursor .mdc and Windsurf .md rules with frontmatter translation**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-09T15:28:53Z
- **Completed:** 2026-03-09T15:33:19Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- ClaudeCodeAdapter, OpenCodeAdapter, and CodexAdapter implement plan_rules_sync returning UpdateMemoryReferences with concatenated rule content in aisync:rules managed sections
- InitEngine::import_rules() scans .cursor/rules/*.mdc and .windsurf/rules/*.md, translates frontmatter to canonical format, and writes .ai/rules/{name}.md files
- scaffold() automatically imports existing rules during aisync init
- 18 new tests covering rule concatenation, import from both tools, frontmatter translation, skip logic, and canonical output format
- All 411 workspace tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement single-file tool plan_rules_sync** - `bbe4a26` (feat)
2. **Task 2: Implement rule import in InitEngine** - `1c438f4` (feat)

## Files Created/Modified
- `crates/aisync-core/src/adapters/mod.rs` - Shared plan_single_file_rules_sync helper for single-file tools
- `crates/aisync-core/src/adapters/claude_code.rs` - Added plan_rules_sync targeting CLAUDE.md
- `crates/aisync-core/src/adapters/opencode.rs` - Added plan_rules_sync targeting AGENTS.md
- `crates/aisync-core/src/adapters/codex.rs` - Added plan_rules_sync targeting AGENTS.md
- `crates/aisync-core/src/init.rs` - Added import_rules(), parse_cursor_rule(), parse_windsurf_rule(), write_canonical_rule(), wired into scaffold()

## Decisions Made
- Shared helper in adapters/mod.rs avoids duplicating concatenation logic across three adapters
- Rule content formatted with "## Rule: {name}" headers for readability in single-file target files

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All rule sync dimensions complete for v1.2 (multi-file for Cursor/Windsurf, single-file concatenation for Claude Code/OpenCode/Codex, import during init)
- Phase 14 (MCP config sync) can proceed independently
- All 411 workspace tests pass

---
*Phase: 13-multi-file-rule-sync*
*Completed: 2026-03-09*
