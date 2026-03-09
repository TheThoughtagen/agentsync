# Milestones

## v1.2 Real-World Hardening (Shipped: 2026-03-09)

**Phases completed:** 5 phases, 9 plans
**Stats:** 40 commits, 41 files changed, 16,917 LOC Rust (up from 12,960), 1 day (2026-03-09)
**Git range:** feat(12-01) → docs(phase-16)

**Key accomplishments:**
- Multi-file rule sync — `.ai/rules/*.md` with YAML frontmatter syncs to Cursor `.mdc`, Windsurf `.md`, and concatenated content for single-file tools
- MCP server config sync — canonical `.ai/mcp.toml` generates per-tool JSON for Claude Code and Cursor with automatic secret stripping
- Security scanner — regex-based API key detection in MCP configs with non-blocking warnings
- Command sync — `.ai/commands/` slash commands sync to Claude Code and Cursor with stale cleanup
- Init completeness — zero-drift after init, ghost tool filtering, correct sync messaging
- Type foundation — RuleFile, McpConfig, CommandFile types and three new adapter trait methods

---

## v1.1 Adapter Expansion & Plugin SDK (Shipped: 2026-03-09)

**Phases completed:** 6 phases, 13 plans
**Stats:** 22 feat commits, 151 files changed, 12,960 LOC Rust (up from 8,514), 3 days (2026-03-07 → 2026-03-09)
**Git range:** feat(06-01) → feat(11-02)

**Key accomplishments:**
- Refactored ToolAdapter trait into extensible system — new adapters are single-file additions
- Added Windsurf and Codex adapters — 5 built-in tools with detection, deduplication, size warnings
- Built `aisync add-tool` — interactive mid-lifecycle tool adoption with partial sync
- Extracted aisync-types and aisync-adapter SDK crates — publishable for community use
- Declarative TOML adapter authoring — define adapters without writing Rust
- Compile-time registration via inventory — community crates auto-discovered at build time

---

## v1.0 aisync universal AI agent context synchronizer (Shipped: 2026-03-07)

**Phases completed:** 5 phases, 20 plans, 0 tasks

**Key accomplishments:**
- Canonical data model with aisync.toml config, adapter trait, and tool detection engine
- Full forward-sync engine with CLI (init, sync, status, dry-run) for Claude Code, OpenCode, and Cursor
- Memory sync across tools with import/export and hook translation engine
- Bidirectional watch mode with reverse sync, conditional sections, diff, and CI check commands
- Cross-platform CI, GitHub releases, Homebrew formula, shell completions, and 188 passing tests

**Stats:** 109 commits, 65 files, 8,514 lines of Rust, 2 days (2026-03-05 to 2026-03-07)

---

