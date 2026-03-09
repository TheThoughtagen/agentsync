# Requirements: aisync v1.2 — Real-World Hardening

**Defined:** 2026-03-09
**Core Value:** Every AI tool working on a project sees the same instructions, memory, and hooks — always in sync, zero manual copying.

## v1.2 Requirements

Requirements for v1.2 release. Each maps to roadmap phases.

### Multi-File Rules

- [x] **RULES-01**: User can place multiple rule files in `.ai/rules/` with YAML frontmatter (description, globs, always_apply)
- [x] **RULES-02**: `aisync sync` generates per-rule `.mdc` files in `.cursor/rules/` with correct Cursor frontmatter
- [x] **RULES-03**: `aisync sync` generates per-rule `.md` files in `.windsurf/rules/` with correct Windsurf frontmatter (trigger types)
- [x] **RULES-04**: Single-file tools (Claude Code, OpenCode, Codex) receive concatenated effective content from all rules appended to instructions
- [x] **RULES-05**: `aisync init` imports existing Cursor `.mdc` and Windsurf `.md` rule files into `.ai/rules/` with frontmatter translation
- [x] **RULES-06**: Managed rule files use `aisync-` prefix to avoid overwriting user-created native rules
- [x] **RULES-07**: `aisync sync` removes stale `aisync-` managed files that no longer have a canonical source

### MCP Server Config Sync

- [x] **MCP-01**: User can define MCP servers in `.ai/mcp.toml` with server name, command, args, and env references
- [x] **MCP-02**: `aisync sync` generates `.claude/.mcp.json` from canonical MCP config
- [x] **MCP-03**: `aisync sync` generates `.cursor/mcp.json` from canonical MCP config
- [x] **MCP-04**: MCP sync strips hardcoded env values and replaces with `${VAR}` references to prevent API key leaks
- [x] **MCP-05**: `aisync init` imports existing tool MCP configs into `.ai/mcp.toml` (merging across tools)
- [x] **MCP-06**: Windsurf MCP is skipped with a warning (global-only config, not project-scoped)
- [x] **MCP-07**: MCP sync scopes to stdio transport only; warns when a server uses unsupported transport for a target tool

### Security

- [x] **SEC-01**: Security scanner detects hardcoded API keys in MCP configs using regex patterns (AWS, GitHub, Slack, generic API keys)
- [x] **SEC-02**: Security warnings are displayed during sync and init, showing which files contain potential secrets
- [x] **SEC-03**: Security scanner warns but does not block — user can proceed after seeing warnings

### Command Sync

- [x] **CMD-01**: `aisync sync` copies `.ai/commands/*.md` to `.claude/commands/` for Claude Code
- [x] **CMD-02**: `aisync sync` copies `.ai/commands/*.md` to `.cursor/commands/` for Cursor
- [x] **CMD-03**: `aisync init` imports existing `.claude/commands/` into `.ai/commands/`
- [x] **CMD-04**: Stale aisync-managed command files are cleaned up when canonical source is removed

### Init & Status Fixes

- [ ] **INIT-01**: `aisync init` completes with zero drift — `aisync status` shows all tools OK immediately after init
- [ ] **INIT-02**: `aisync status` only shows tools that are configured in `aisync.toml` or detected, not all possible adapters
- [ ] **INIT-03**: `aisync init` presents interactive source tool selection when multiple instruction sources exist
- [ ] **INIT-04**: `aisync sync` output uses correct messages (no "Would create" during real sync)

### Types & Trait Foundation

- [x] **TYPE-01**: `aisync-types` crate exports `RuleFile`, `RuleMetadata`, `McpConfig`, `McpServer`, `CommandFile` types
- [x] **TYPE-02**: `ToolAdapter` trait has `plan_rules_sync()`, `plan_mcp_sync()`, `plan_commands_sync()` methods with default no-op implementations
- [x] **TYPE-03**: `SyncAction` enum has variants for rule file creation, MCP file generation, and command file copying
- [x] **TYPE-04**: `AnyAdapter` dispatch updated for new trait methods

## v1.3 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Advanced Rules

- **RULES-08**: Bidirectional multi-file rule sync (reverse-sync external edits back to `.ai/rules/`)
- **RULES-09**: Cursor folder-based rules support (post-v2.2 format, currently unstable)

### Advanced MCP

- **MCP-08**: Windsurf project-level MCP config (when/if Windsurf adds support)
- **MCP-09**: MCP health checking (verify servers are reachable)
- **MCP-10**: Per-tool MCP server filtering (`tools: ["claude-code", "cursor"]` in `.ai/mcp.toml`)
- **MCP-11**: SSE/streamable-http transport support beyond stdio

### Watch Mode

- **WATCH-01**: Watch mode monitors `.ai/rules/` and `.ai/mcp.toml` for changes
- **WATCH-02**: Watch mode monitors `.ai/commands/` for changes

## Out of Scope

| Feature | Reason |
|---------|--------|
| Dynamic plugin loading (dylib/WASM) | Prove interface stability first |
| Plugin/extension sync | Each tool's own ecosystem |
| Auth/credential sharing | Security concern — aisync explicitly strips secrets |
| IDE settings sync | Use Settings Sync for that |
| Chat history/session sync | Proprietary, ephemeral |
| Codex hierarchical AGENTS.md | Per-subdirectory sync adds significant complexity |
| Runtime adapter hot-reloading | Add in future version |
| Windsurf MCP writing | Global-only config — too risky to write to |
| Skill supporting files sync | Complex tool-specific assets |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| TYPE-01 | Phase 12 | Complete |
| TYPE-02 | Phase 12 | Complete |
| TYPE-03 | Phase 12 | Complete |
| TYPE-04 | Phase 12 | Complete |
| RULES-01 | Phase 13 | Complete |
| RULES-02 | Phase 13 | Complete |
| RULES-03 | Phase 13 | Complete |
| RULES-04 | Phase 13 | Complete |
| RULES-05 | Phase 13 | Complete |
| RULES-06 | Phase 13 | Complete |
| RULES-07 | Phase 13 | Complete |
| MCP-01 | Phase 14 | Complete |
| MCP-02 | Phase 14 | Complete |
| MCP-03 | Phase 14 | Complete |
| MCP-04 | Phase 14 | Complete |
| MCP-05 | Phase 14 | Complete |
| MCP-06 | Phase 14 | Complete |
| MCP-07 | Phase 14 | Complete |
| SEC-01 | Phase 14 | Complete |
| SEC-02 | Phase 14 | Complete |
| SEC-03 | Phase 14 | Complete |
| CMD-01 | Phase 15 | Complete |
| CMD-02 | Phase 15 | Complete |
| CMD-03 | Phase 15 | Complete |
| CMD-04 | Phase 15 | Complete |
| INIT-01 | Phase 16 | Pending |
| INIT-02 | Phase 16 | Pending |
| INIT-03 | Phase 16 | Pending |
| INIT-04 | Phase 16 | Pending |

**Coverage:**
- v1.2 requirements: 29 total
- Mapped to phases: 29
- Unmapped: 0 ✓

---
*Requirements defined: 2026-03-09*
*Last updated: 2026-03-09 after initial definition*
