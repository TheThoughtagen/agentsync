# Roadmap: aisync

## Milestones

- ✅ **v1.0 aisync** — Phases 1-5 (shipped 2026-03-07)
- ✅ **v1.1 Adapter Expansion & Plugin SDK** — Phases 6-11 (shipped 2026-03-09)
- ✅ **v1.2 Real-World Hardening** — Phases 12-16 (shipped 2026-03-09)

## Phases

<details>
<summary>✅ v1.0 aisync (Phases 1-5) — SHIPPED 2026-03-07</summary>

- [x] Phase 1: Foundation and Data Model (2/2 plans) — completed 2026-03-05
- [x] Phase 2: Core Sync Loop MVP (5/5 plans) — completed 2026-03-05
- [x] Phase 3: Memory and Hooks (5/5 plans) — completed 2026-03-06
- [x] Phase 4: Watch Mode and Bidirectional Sync (5/5 plans) — completed 2026-03-06
- [x] Phase 5: Polish and Distribution (3/3 plans) — completed 2026-03-06

</details>

<details>
<summary>✅ v1.1 Adapter Expansion & Plugin SDK (Phases 6-11) — SHIPPED 2026-03-09</summary>

- [x] Phase 6: Core Refactoring (3/3 plans) — completed 2026-03-08
- [x] Phase 7: Windsurf & Codex Adapters (2/2 plans) — completed 2026-03-08
- [x] Phase 8: Add-Tool Command (2/2 plans) — completed 2026-03-09
- [x] Phase 9: Plugin SDK Crate Extraction (2/2 plans) — completed 2026-03-09
- [x] Phase 10: Declarative TOML Adapters (2/2 plans) — completed 2026-03-09
- [x] Phase 11: Compile-Time Registration (2/2 plans) — completed 2026-03-09

</details>

### ✅ v1.2 Real-World Hardening (Shipped 2026-03-09)

**Milestone Goal:** Make aisync work correctly on real production codebases with multi-file rules, MCP servers, and complete init-to-sync workflows.

- [x] **Phase 12: Types & Trait Foundation** - New types and trait methods that unblock all subsequent phases (completed 2026-03-09)
- [x] **Phase 13: Multi-File Rule Sync** - Users can define rules once and have them sync to every tool's native format (completed 2026-03-09)
- [x] **Phase 14: MCP Server Config & Security** - Users can define MCP servers once and share them across tools without leaking secrets (completed 2026-03-09)
- [x] **Phase 15: Command Sync** - Users can define slash commands once and have them available in all supporting tools (completed 2026-03-09)
- [x] **Phase 16: Init Completeness** - Init workflow produces a fully synced project with zero drift (completed 2026-03-09)

## Phase Details

### Phase 12: Types & Trait Foundation
**Goal**: The type system and adapter trait support rules, MCP, and command sync dimensions
**Depends on**: Phase 11
**Requirements**: TYPE-01, TYPE-02, TYPE-03, TYPE-04
**Success Criteria** (what must be TRUE):
  1. `aisync-types` crate exports `RuleFile`, `RuleMetadata`, `McpConfig`, `McpServer`, and `CommandFile` types that compile and are usable by downstream crates
  2. `ToolAdapter` trait has `plan_rules_sync()`, `plan_mcp_sync()`, and `plan_commands_sync()` methods with default no-op implementations so existing adapters continue to compile unchanged
  3. `SyncAction` enum has variants for rule file creation, MCP file generation, and command file copying
  4. `AnyAdapter` dispatches the three new trait methods to all adapter variants including Plugin
**Plans**: 1 plan
Plans:
- [ ] 12-01-PLAN.md — New types, SyncAction variants, trait methods, and AnyAdapter dispatch

### Phase 13: Multi-File Rule Sync
**Goal**: Users can place multiple rule files in `.ai/rules/` and have them sync to every tool's native format with correct metadata
**Depends on**: Phase 12
**Requirements**: RULES-01, RULES-02, RULES-03, RULES-04, RULES-05, RULES-06, RULES-07
**Success Criteria** (what must be TRUE):
  1. User can create `.ai/rules/*.md` files with YAML frontmatter (description, globs, always_apply) and `aisync sync` generates corresponding `.mdc` files in `.cursor/rules/` with correct Cursor frontmatter
  2. User can create `.ai/rules/*.md` files and `aisync sync` generates corresponding `.md` files in `.windsurf/rules/` with correct Windsurf trigger-type frontmatter
  3. Single-file tools (Claude Code, OpenCode, Codex) receive concatenated effective content from all rules appended to their instructions file
  4. `aisync init` imports existing Cursor `.mdc` and Windsurf `.md` rule files into `.ai/rules/` with frontmatter translated to canonical format
  5. Managed rule files use `aisync-` prefix so user-created native rules are never overwritten, and stale managed files are cleaned up when their canonical source is removed
**Plans**: 2 plans
Plans:
- [ ] 13-01-PLAN.md — Rule loader, Cursor/Windsurf adapter implementations, sync engine wiring
- [ ] 13-02-PLAN.md — Single-file tool concatenation and init rule import

### Phase 14: MCP Server Config & Security
**Goal**: Users can define MCP servers once in `.ai/mcp.toml` and have them sync to Claude Code and Cursor with hardcoded secrets detected and stripped
**Depends on**: Phase 12
**Requirements**: MCP-01, MCP-02, MCP-03, MCP-04, MCP-05, MCP-06, MCP-07, SEC-01, SEC-02, SEC-03
**Success Criteria** (what must be TRUE):
  1. User can define MCP servers in `.ai/mcp.toml` and `aisync sync` generates `.claude/.mcp.json` and `.cursor/mcp.json` with correct per-tool JSON format
  2. MCP sync automatically strips hardcoded env values and replaces them with `${VAR}` references to prevent API key leaks in generated files
  3. Security scanner detects hardcoded API keys (AWS, GitHub, Slack, generic patterns) in MCP configs and displays warnings during sync and init without blocking the operation
  4. `aisync init` imports existing tool MCP configs into `.ai/mcp.toml`, merging across tools
  5. Windsurf MCP is skipped with a warning explaining it uses global-only config, and unsupported transports trigger a warning per server per tool
**Plans**: 2 plans
Plans:
- [ ] 14-01-PLAN.md — McpEngine, SecurityScanner, adapter plan_mcp_sync, sync pipeline wiring
- [ ] 14-02-PLAN.md — MCP import during init (parse JSON, merge, sanitize, write TOML)

### Phase 15: Command Sync
**Goal**: Users can define slash commands in `.ai/commands/` and have them available in Claude Code and Cursor
**Depends on**: Phase 12
**Requirements**: CMD-01, CMD-02, CMD-03, CMD-04
**Success Criteria** (what must be TRUE):
  1. User can place `.md` files in `.ai/commands/` and `aisync sync` copies them to `.claude/commands/` and `.cursor/commands/`
  2. `aisync init` imports existing `.claude/commands/` files into `.ai/commands/`
  3. Stale aisync-managed command files are cleaned up when their canonical source is removed
**Plans**: 2 plans
Plans:
- [ ] 15-01-PLAN.md — CommandEngine loader, shared adapter helper, Claude Code + Cursor sync, stale cleanup
- [ ] 15-02-PLAN.md — Init command import from .claude/commands/

### Phase 16: Init Completeness
**Goal**: The init workflow produces a fully synced project -- `aisync status` shows all tools OK immediately after init with no manual sync needed
**Depends on**: Phase 13, Phase 14, Phase 15
**Requirements**: INIT-01, INIT-02, INIT-03, INIT-04
**Success Criteria** (what must be TRUE):
  1. Running `aisync init` followed immediately by `aisync status` shows all configured tools as OK with zero drift
  2. `aisync status` only shows tools that are configured in `aisync.toml` or actually detected in the project -- no ghost tools
  3. When multiple instruction sources exist during init, user is presented with an interactive source tool selection
  4. `aisync sync` output uses correct action messages (no "Would create" phrasing during real sync)
**Plans**: 2 plans
Plans:
- [ ] 16-01-PLAN.md — Fix ghost tool filtering and sync action messaging
- [ ] 16-02-PLAN.md — Add auto-sync to init workflow, verify source selection

## Progress

**Execution Order:**
Phases execute in numeric order. Phase 16 depends on 13, 14, and 15 completing. Phases 13, 14, and 15 all depend on 12 but are independent of each other.

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation and Data Model | v1.0 | 2/2 | Complete | 2026-03-05 |
| 2. Core Sync Loop MVP | v1.0 | 5/5 | Complete | 2026-03-05 |
| 3. Memory and Hooks | v1.0 | 5/5 | Complete | 2026-03-06 |
| 4. Watch Mode and Bidirectional Sync | v1.0 | 5/5 | Complete | 2026-03-06 |
| 5. Polish and Distribution | v1.0 | 3/3 | Complete | 2026-03-06 |
| 6. Core Refactoring | v1.1 | 3/3 | Complete | 2026-03-08 |
| 7. Windsurf & Codex Adapters | v1.1 | 2/2 | Complete | 2026-03-08 |
| 8. Add-Tool Command | v1.1 | 2/2 | Complete | 2026-03-09 |
| 9. Plugin SDK Crate Extraction | v1.1 | 2/2 | Complete | 2026-03-09 |
| 10. Declarative TOML Adapters | v1.1 | 2/2 | Complete | 2026-03-09 |
| 11. Compile-Time Registration | v1.1 | 2/2 | Complete | 2026-03-09 |
| 12. Types & Trait Foundation | 1/1 | Complete    | 2026-03-09 | - |
| 13. Multi-File Rule Sync | 2/2 | Complete    | 2026-03-09 | - |
| 14. MCP Server Config & Security | 2/2 | Complete    | 2026-03-09 | - |
| 15. Command Sync | 2/2 | Complete    | 2026-03-09 | - |
| 16. Init Completeness | v1.2 | Complete    | 2026-03-09 | 2026-03-09 |

---
*Roadmap created: 2026-03-08*
*v1.1 milestone shipped: 2026-03-09*
*v1.2 milestone started: 2026-03-09*
