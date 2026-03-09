---
phase: 14-mcp-config-sync
plan: 01
subsystem: sync
tags: [mcp, security, json, regex, toml]

# Dependency graph
requires:
  - phase: 12-types-trait-foundation
    provides: McpConfig/McpServer types, ToolAdapter trait with plan_mcp_sync, WriteMcpConfig SyncAction
  - phase: 13-multi-file-rule-sync
    provides: sync pipeline pattern for per-adapter dispatch, RuleEngine loading pattern
provides:
  - McpEngine for loading .ai/mcp.toml and generating tool-native JSON
  - SecurityScanner with regex-based secret detection (6 patterns)
  - Adapter plan_mcp_sync implementations for all 5 tools
  - MCP sync wired into SyncEngine plan_all_internal pipeline
affects: [14-02-cli-integration, 15-command-sync]

# Tech tracking
tech-stack:
  added: [regex]
  patterns: [McpEngine load/sanitize/generate pattern, SecurityScanner LazyLock regex patterns]

key-files:
  created:
    - crates/aisync-core/src/mcp.rs
    - crates/aisync-core/src/security.rs
  modified:
    - crates/aisync-core/Cargo.toml
    - crates/aisync-core/src/lib.rs
    - crates/aisync-core/src/sync.rs
    - crates/aisync-core/src/adapters/claude_code.rs
    - crates/aisync-core/src/adapters/cursor.rs
    - crates/aisync-core/src/adapters/windsurf.rs
    - crates/aisync-core/src/adapters/opencode.rs
    - crates/aisync-core/src/adapters/codex.rs

key-decisions:
  - "Used std::sync::LazyLock for regex pattern compilation -- stable since Rust 1.80, no external dep needed"
  - "Security warnings emitted as WarnUnsupportedDimension actions with dimension=security to reuse existing CLI display pipeline"
  - "McpEngine::generate_mcp_json omits empty args/env for cleaner output"
  - "Sanitize_env replaces secrets with ${KEY_NAME} references using env key name"

patterns-established:
  - "MCP engine pattern: load from .ai/mcp.toml, sanitize, generate JSON -- mirrors RuleEngine load pattern"
  - "Security scan runs before sanitize_env so warnings reference original values"

requirements-completed: [MCP-01, MCP-02, MCP-03, MCP-04, MCP-06, MCP-07, SEC-01, SEC-02, SEC-03]

# Metrics
duration: 8min
completed: 2026-03-09
---

# Phase 14 Plan 01: MCP Config Sync Summary

**McpEngine loads .ai/mcp.toml and generates tool-native mcpServers JSON for Claude Code and Cursor, with regex-based SecurityScanner detecting 6 API key patterns and automatic secret sanitization**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-09T16:46:48Z
- **Completed:** 2026-03-09T16:55:04Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments
- McpEngine loads TOML config, sanitizes hardcoded secrets to ${VAR} references, generates pretty-printed JSON
- SecurityScanner detects AWS, GitHub, GitHub Fine-grained, Slack, Anthropic, and OpenAI API key patterns
- Claude Code and Cursor adapters generate correct mcpServers JSON at .claude/.mcp.json and .cursor/mcp.json
- Windsurf, OpenCode, and Codex adapters emit WarnUnsupportedDimension for MCP
- Full pipeline wiring: load -> security scan -> sanitize -> per-adapter dispatch

## Task Commits

Each task was committed atomically:

1. **Task 1: Create McpEngine and SecurityScanner modules** - `f4f32f8` (feat)
2. **Task 2: Implement adapter plan_mcp_sync and wire into sync pipeline** - `39e8463` (feat)

## Files Created/Modified
- `crates/aisync-core/src/mcp.rs` - McpEngine: load, sanitize_env, generate_mcp_json
- `crates/aisync-core/src/security.rs` - SecurityScanner: looks_like_secret, scan_mcp_config with 6 regex patterns
- `crates/aisync-core/src/lib.rs` - Module registration and re-exports
- `crates/aisync-core/src/sync.rs` - MCP loading, security scanning, sanitization, per-adapter dispatch
- `crates/aisync-core/Cargo.toml` - Added regex dependency
- `crates/aisync-core/src/adapters/claude_code.rs` - plan_mcp_sync targeting .claude/.mcp.json
- `crates/aisync-core/src/adapters/cursor.rs` - plan_mcp_sync targeting .cursor/mcp.json
- `crates/aisync-core/src/adapters/windsurf.rs` - plan_mcp_sync returning global-only warning
- `crates/aisync-core/src/adapters/opencode.rs` - plan_mcp_sync returning no-support warning
- `crates/aisync-core/src/adapters/codex.rs` - plan_mcp_sync returning no-support warning

## Decisions Made
- Used `std::sync::LazyLock` for regex compilation (stable since Rust 1.80, avoids external lazy_static dep)
- Security warnings flow as `WarnUnsupportedDimension` with `dimension: "security"` to reuse existing CLI display pipeline without modifying SyncReport
- `generate_mcp_json` omits empty `args` and `env` fields for cleaner output matching tool expectations
- `sanitize_env` uses env key name for ${KEY_NAME} substitution (e.g., AWS_KEY -> ${AWS_KEY})

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- ConfigError::ParseFailed variant did not exist; used ConfigError::Parse(toml::de::Error) instead for TOML parse errors
- Linter removed module declarations from lib.rs between commits; re-added mcp and security modules

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- MCP config sync core is complete, ready for CLI integration (14-02)
- All 399 aisync-core tests pass including 31 new MCP/security tests
- Workspace builds cleanly

---
*Phase: 14-mcp-config-sync*
*Completed: 2026-03-09*
