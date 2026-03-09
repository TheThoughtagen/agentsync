# Roadmap: aisync

## Milestones

- [x] **v1.0 aisync** -- Phases 1-5 (shipped 2026-03-07)
- [ ] **v1.1 Adapter Expansion & Plugin SDK** -- Phases 6-11 (in progress)

## Phases

<details>
<summary>v1.0 aisync (Phases 1-5) -- SHIPPED 2026-03-07</summary>

- [x] Phase 1: Foundation and Data Model (2/2 plans) -- completed 2026-03-05
- [x] Phase 2: Core Sync Loop MVP (5/5 plans) -- completed 2026-03-05
- [x] Phase 3: Memory and Hooks (5/5 plans) -- completed 2026-03-06
- [x] Phase 4: Watch Mode and Bidirectional Sync (5/5 plans) -- completed 2026-03-06
- [x] Phase 5: Polish and Distribution (3/3 plans) -- completed 2026-03-06

</details>

### v1.1 Adapter Expansion & Plugin SDK

**Milestone Goal:** Expand tool coverage to 5 tools (adding Windsurf and Codex), introduce `add-tool` for incremental adoption, and build a two-layer Plugin SDK (declarative TOML + Rust trait) enabling community adapter development.

- [ ] **Phase 6: Core Refactoring** - Pull tool metadata into ToolAdapter trait, unblock extensible adapter system
- [x] **Phase 7: Windsurf & Codex Adapters** - Add two new built-in adapters using refactored trait system (completed 2026-03-08)
- [ ] **Phase 8: Add-Tool Command** - Interactive CLI for mid-lifecycle tool adoption
- [ ] **Phase 9: Plugin SDK Crate Extraction** - Extract shared types and adapter trait into publishable crates
- [ ] **Phase 10: Declarative TOML Adapters** - Enable adapter authoring without Rust via TOML definitions
- [ ] **Phase 11: Compile-Time Registration** - Community adapter crates register at build time

## Phase Details

### Phase 6: Core Refactoring
**Goal**: Tool-specific metadata lives in the ToolAdapter trait, not scattered across match arms -- making new adapters a single-file addition
**Depends on**: Phase 5 (v1.0 complete)
**Requirements**: REFAC-01, REFAC-02, REFAC-03, REFAC-04
**Success Criteria** (what must be TRUE):
  1. Adding a new adapter requires implementing ToolAdapter only -- no changes to match arms in sync, watch, init, status, or CLI modules
  2. `aisync.toml` with arbitrary tool names (e.g., `[tools.windsurf]`) deserializes correctly alongside existing named tool configs
  3. AnyAdapter enum accepts a Plugin variant that dispatches to any ToolAdapter implementation
  4. Display name for each tool comes from a single ToolAdapter method (no duplicate functions)
**Plans**: 3 plans

Plans:
- [x] 06-01-PLAN.md — ToolKind Custom(String) variant + Copy-to-Clone migration
- [x] 06-02-PLAN.md — ToolAdapter trait expansion, dispatch macro, Plugin variant, display name consolidation
- [x] 06-03-PLAN.md — ToolsConfig BTreeMap migration + enabled_tools refactor

### Phase 7: Windsurf & Codex Adapters
**Goal**: Users with Windsurf or Codex installed get automatic sync from `.ai/` to their tool's native format
**Depends on**: Phase 6
**Requirements**: ADPT-01, ADPT-02, ADPT-03, ADPT-04, ADPT-05, ADPT-06
**Success Criteria** (what must be TRUE):
  1. Running `aisync sync` in a project with Windsurf generates `.windsurf/rules/project.md` with correct YAML frontmatter
  2. Running `aisync sync` in a project with Codex creates an `AGENTS.md` symlink pointing to `.ai/instructions.md`
  3. When both Codex and OpenCode are present, `aisync status` shows both tools detected and `aisync sync` produces a single non-duplicated AGENTS.md action
  4. Legacy `.windsurfrules` file is detected with a migration hint to the modern `.windsurf/rules/` format
  5. Content exceeding tool limits (Windsurf 12K chars, Codex 32 KiB) triggers a visible warning
**Plans**: 2 plans

Plans:
- [ ] 07-01-PLAN.md — ToolKind variants, adapter registration, WindsurfAdapter + CodexAdapter implementations
- [ ] 07-02-PLAN.md — SyncEngine AGENTS.md deduplication + content size limit warnings

### Phase 8: Add-Tool Command
**Goal**: Users can adopt new AI tools mid-project without manual config editing or full re-initialization
**Depends on**: Phase 7
**Requirements**: TOOL-01, TOOL-02, TOOL-03, TOOL-04
**Success Criteria** (what must be TRUE):
  1. Running `aisync add-tool` in a project with unconfigured tools shows which tools are available to add
  2. User can interactively select tools and they are added to `aisync.toml` with default settings
  3. After adding a tool, only that tool's files are synced (no full re-sync of existing tools)
**Plans**: 2 plans

Plans:
- [ ] 08-01-PLAN.md — AddToolEngine core logic + SyncEngine partial sync (plan_for_tools)
- [ ] 08-02-PLAN.md — CLI add-tool command with interactive/non-interactive modes + integration tests

### Phase 9: Plugin SDK Crate Extraction
**Goal**: Community developers can depend on published `aisync-types` and `aisync-adapter` crates to build custom adapters in Rust
**Depends on**: Phase 7
**Requirements**: SDK-01, SDK-02
**Success Criteria** (what must be TRUE):
  1. `aisync-types` crate compiles independently with only serde/thiserror dependencies and exports ToolKind, SyncStrategy, and related types
  2. `aisync-adapter` crate exports the ToolAdapter trait and can be added as a dependency by an external crate
  3. `aisync-core` depends on `aisync-types` and `aisync-adapter` (inverted dependency -- core depends on SDK, not vice versa)
**Plans**: 2 plans

Plans:
- [ ] 09-01-PLAN.md — aisync-types crate extraction (shared types with serde+thiserror only)
- [ ] 09-02-PLAN.md — aisync-adapter crate extraction (ToolAdapter trait + AdapterError)

### Phase 10: Declarative TOML Adapters
**Goal**: Users can define new tool adapters via TOML files in `.ai/adapters/` without writing Rust
**Depends on**: Phase 9
**Requirements**: SDK-03, SDK-04, SDK-05
**Success Criteria** (what must be TRUE):
  1. A `.ai/adapters/mytool.toml` file with detection rules, file mappings, and sync strategy is auto-discovered on `aisync sync`
  2. The TOML-defined adapter appears in `aisync status` output alongside built-in adapters
  3. A TOML adapter can generate output files using template syntax with instruction content interpolation
**Plans**: TBD

Plans:
- [ ] 10-01: TBD

### Phase 11: Compile-Time Registration
**Goal**: Community Rust adapter crates register automatically at compile time -- no central enum modification required
**Depends on**: Phase 10
**Requirements**: SDK-06, SDK-07
**Success Criteria** (what must be TRUE):
  1. A community adapter crate using `inventory::submit!` is picked up by `aisync` binary without modifying any source in the main repository
  2. Documentation exists for both TOML and Rust adapter authoring paths, with working examples
**Plans**: TBD

Plans:
- [ ] 11-01: TBD

## Progress

**Execution Order:** Phases 6 through 11, sequential. Phase 9 depends on Phase 7 (not Phase 8), so Phases 8 and 9 could theoretically run in parallel, but are sequenced for clarity.

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation and Data Model | v1.0 | 2/2 | Complete | 2026-03-05 |
| 2. Core Sync Loop MVP | v1.0 | 5/5 | Complete | 2026-03-05 |
| 3. Memory and Hooks | v1.0 | 5/5 | Complete | 2026-03-06 |
| 4. Watch Mode and Bidirectional Sync | v1.0 | 5/5 | Complete | 2026-03-06 |
| 5. Polish and Distribution | v1.0 | 3/3 | Complete | 2026-03-06 |
| 6. Core Refactoring | v1.1 | 3/3 | Complete | 2026-03-08 |
| 7. Windsurf & Codex Adapters | 2/2 | Complete   | 2026-03-08 | - |
| 8. Add-Tool Command | v1.1 | 0/2 | Not started | - |
| 9. Plugin SDK Crate Extraction | v1.1 | 0/TBD | Not started | - |
| 10. Declarative TOML Adapters | v1.1 | 0/TBD | Not started | - |
| 11. Compile-Time Registration | v1.1 | 0/TBD | Not started | - |

---
*Roadmap created: 2026-03-08*
*v1.1 milestone: Adapter Expansion & Plugin SDK*
