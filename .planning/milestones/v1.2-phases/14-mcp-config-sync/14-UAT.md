---
status: complete
phase: 14-mcp-config-sync
source: 14-01-SUMMARY.md, 14-02-SUMMARY.md
started: 2026-03-09T17:10:00Z
updated: 2026-03-09T17:25:00Z
---

## Current Test

[testing complete]

## Tests

### 1. MCP TOML Loading
expected: McpEngine loads .ai/mcp.toml and parses server definitions (command, args, env) into McpConfig struct. `cargo test -- mcp::tests::test_load_parses_valid_toml` passes.
result: pass

### 2. MCP JSON Generation for Claude Code
expected: Running sync with MCP config generates .claude/.mcp.json with correct mcpServers JSON. Empty args/env fields are omitted for cleaner output. `cargo test -- claude_code::tests::test_plan_mcp_sync` passes.
result: pass

### 3. MCP JSON Generation for Cursor
expected: Running sync with MCP config generates .cursor/mcp.json with correct mcpServers JSON. `cargo test -- cursor::tests::test_plan_mcp_sync` passes.
result: pass

### 4. Security Scanner Detects API Keys
expected: SecurityScanner detects 6 API key patterns: AWS (AKIA...), GitHub (ghp_/gho_/ghs_), GitHub Fine-grained (github_pat_), Slack (xoxb-/xoxp-), Anthropic (sk-ant-), OpenAI (sk-). `cargo test -- security::tests` all pass (14 tests).
result: pass

### 5. Secret Sanitization
expected: McpEngine.sanitize_env replaces hardcoded secrets with ${KEY_NAME} variable references (e.g., AWS key in ANTHROPIC_API_KEY env var becomes ${ANTHROPIC_API_KEY}). Normal values and existing ${VAR} references are left untouched.
result: pass

### 6. Unsupported Tool Warnings
expected: Windsurf, OpenCode, and Codex adapters emit WarnUnsupportedDimension with dimension="mcp" when MCP sync is attempted, since these tools don't support project-level MCP config.
result: pass

### 7. MCP Import During Init
expected: `aisync init` (scaffold) imports existing MCP configs from Claude Code (.claude/.mcp.json or root .mcp.json) and Cursor (.cursor/mcp.json), merging with first-seen-wins priority. Claude Code sources are checked first. Creates .ai/mcp.toml.
result: pass

### 8. Import Skips Non-stdio Servers
expected: parse_mcp_json skips HTTP/SSE MCP servers (entries without a "command" field) and only imports stdio-based servers.
result: pass

### 9. Import Sanitizes Secrets
expected: During init import, any hardcoded secrets found in existing MCP JSON configs are automatically sanitized to ${VAR} references before writing to .ai/mcp.toml.
result: pass

### 10. Full Test Suite Passes
expected: Running `cargo test -p aisync-core` passes all tests (420+ including 34 MCP + 14 security tests). No regressions from phase 14 changes.
result: pass

## Summary

total: 10
passed: 10
issues: 0
pending: 0
skipped: 0

## Gaps

[none yet]
