---
phase: 14-mcp-config-sync
plan: 02
subsystem: sync
tags: [mcp, json, toml, import, init]

# Dependency graph
requires:
  - phase: 14-mcp-config-sync-01
    provides: McpEngine with load/sanitize_env/generate_mcp_json, SecurityScanner with looks_like_secret/scan_mcp_config
provides:
  - McpEngine::parse_mcp_json for parsing tool-native MCP JSON files
  - InitEngine::import_mcp for importing MCP configs during aisync init
  - scaffold() wired to automatically import MCP configs
affects: [15-command-sync]

# Tech tracking
tech-stack:
  added: []
  patterns: [parse_mcp_json JSON-to-McpConfig pattern, import_mcp merge-with-priority pattern]

key-files:
  created: []
  modified:
    - crates/aisync-core/src/mcp.rs
    - crates/aisync-core/src/init.rs

key-decisions:
  - "parse_mcp_json returns empty McpConfig for missing/invalid files rather than erroring (non-fatal import)"
  - "Claude Code checked first (.claude/.mcp.json then root .mcp.json), Cursor second, ensuring Claude Code priority in first-seen-wins merge"

patterns-established:
  - "MCP JSON import: parse tool-native JSON with manual serde_json::Value extraction to skip non-stdio servers"
  - "First-seen-wins merge via BTreeMap::entry().or_insert() for multi-source config import"

requirements-completed: [MCP-05, SEC-02]

# Metrics
duration: 3min
completed: 2026-03-09
---

# Phase 14 Plan 02: MCP Config Import Summary

**parse_mcp_json extracts stdio MCP servers from tool-native JSON, import_mcp merges Claude Code and Cursor configs with first-seen-wins priority and automatic secret sanitization during aisync init**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-09T16:58:35Z
- **Completed:** 2026-03-09T17:01:48Z
- **Tasks:** 1 (TDD: RED + GREEN)
- **Files modified:** 2

## Accomplishments
- McpEngine::parse_mcp_json parses {"mcpServers": {...}} JSON, skipping HTTP/SSE servers without command field
- InitEngine::import_mcp merges Claude Code (.claude/.mcp.json with root .mcp.json fallback) and Cursor (.cursor/mcp.json) with first-seen-wins priority
- Imported secrets automatically sanitized to ${VAR} references before writing .ai/mcp.toml
- scaffold() calls import_mcp after import_commands, completing the init pipeline
- 15 new tests covering all edge cases (parse, merge, sanitize, skip, fallback)

## Task Commits

Each task was committed atomically:

1. **Task 1 (RED): Add failing tests for parse_mcp_json and import_mcp** - `0ddc4ec` (test)
2. **Task 1 (GREEN): Implement parse_mcp_json and import_mcp** - `9276d13` (feat)

_Note: TDD task with RED/GREEN commits._

## Files Created/Modified
- `crates/aisync-core/src/mcp.rs` - Added parse_mcp_json: tool-native JSON parsing with HTTP server filtering
- `crates/aisync-core/src/init.rs` - Added import_mcp: multi-source merge, sanitization, TOML output; wired into scaffold()

## Decisions Made
- parse_mcp_json returns empty McpConfig for missing/invalid files rather than erroring, making import non-fatal
- Claude Code sources checked first (.claude/.mcp.json, then root .mcp.json fallback) before Cursor, ensuring first-seen-wins priority
- Security scan runs before sanitize_env (warnings reference original secret values) but results stored for future CLI display

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- MCP config import complete, phase 14 fully done
- All 477 workspace tests pass (420 aisync-core including 15 new)
- Ready for Phase 15 (command sync)

---
*Phase: 14-mcp-config-sync*
*Completed: 2026-03-09*
