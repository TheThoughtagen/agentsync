# Roadmap: aisync

## Overview

aisync delivers a universal AI agent context synchronizer in five phases: establish the data model and adapter architecture, then deliver a working forward-sync MVP for Tier 1 tools (Claude Code, OpenCode, Cursor), expand to memory and hook sync capabilities, add watch mode and bidirectional sync once the engine is proven stable, and finish with distribution and polish. Every phase delivers a coherent, testable capability that builds on the last.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Foundation and Data Model** - Canonical model, config schema, adapter trait, tool detection engine, error types
- [ ] **Phase 2: Core Sync Loop (MVP)** - aisync init, sync, status with Tier 1 adapters (Claude Code, OpenCode, Cursor) delivering end-to-end forward sync
- [ ] **Phase 3: Memory and Hooks** - Memory sync across tools, hook translation engine, CLI subcommands for both
- [ ] **Phase 4: Watch Mode and Bidirectional Sync** - File watching daemon, reverse sync from tool-native to canonical, conditional instruction sections, diff and CI check commands
- [ ] **Phase 5: Polish and Distribution** - Shell completions, error UX, Homebrew tap, cargo install, GitHub releases, cross-platform CI, test suite

## Phase Details

### Phase 1: Foundation and Data Model
**Goal**: The canonical data model, config schema, adapter trait, and tool detection exist as a compilable Rust library that all future phases build on
**Depends on**: Nothing (first phase)
**Requirements**: CLI-08, ADPT-04, ADPT-05
**Success Criteria** (what must be TRUE):
  1. A Cargo workspace exists with separate library and binary crates that compile and pass `cargo test`
  2. `aisync.toml` can be parsed and serialized with `schema_version = 1`, per-tool settings, and sync strategy fields
  3. The ToolAdapter trait is defined with detect and name methods (lean Phase 1 — remaining methods added in later phases)
  4. Tool detection engine scans a project directory and correctly identifies which AI tools are configured (Claude Code, OpenCode, Cursor)
**Plans**: 2 plans

Plans:
- [ ] 01-01-PLAN.md — Cargo workspace, shared types, error hierarchy, config parsing
- [ ] 01-02-PLAN.md — Adapter trait, tool adapters, detection engine

### Phase 2: Core Sync Loop (MVP)
**Goal**: Users can scaffold a canonical `.ai/` directory, import existing configs, and forward-sync instructions to Claude Code, OpenCode, and Cursor with a single command
**Depends on**: Phase 1
**Requirements**: CLI-01, CLI-02, CLI-03, CLI-04, CLI-09, CLI-11, INST-01, INST-02, INST-03, INST-04, INST-05, INST-06, INST-07, INST-10, ADPT-01, ADPT-02, ADPT-03
**Success Criteria** (what must be TRUE):
  1. User can run `aisync init` in a project with existing CLAUDE.md and .cursor/rules/ and have them imported into `.ai/instructions.md` with conflict resolution prompts
  2. User can run `aisync sync` and see `.ai/instructions.md` appear as CLAUDE.md (symlink), AGENTS.md (symlink), and `.cursor/rules/project.mdc` (generated with YAML frontmatter)
  3. User can run `aisync sync --dry-run` and see what would change without any files being modified
  4. User can run `aisync status` and see per-tool sync state including symlink validation and drift detection
  5. Running `aisync sync` twice in a row produces identical results (idempotent)
**Plans**: 4 plans

Plans:
- [ ] 02-01-PLAN.md — Workspace deps, sync types/errors, ToolAdapter extension, gitignore module
- [ ] 02-02-PLAN.md — Adapter sync implementations (read/write/status), sync engine
- [ ] 02-03-PLAN.md — Init engine and aisync init CLI command
- [ ] 02-04-PLAN.md — Clap CLI wiring, sync and status CLI commands

### Phase 3: Memory and Hooks
**Goal**: Users can sync memory files and hook definitions across all Tier 1 tools, with CLI subcommands for managing both
**Depends on**: Phase 2
**Requirements**: MEM-01, MEM-02, MEM-03, MEM-04, MEM-05, MEM-06, MEM-07, HOOK-01, HOOK-02, HOOK-03, HOOK-04, HOOK-05, HOOK-06, HOOK-07
**Success Criteria** (what must be TRUE):
  1. User can run `aisync memory add <topic>` to create a memory file in `.ai/memory/`, and `aisync sync` propagates it to Claude Code (symlink), OpenCode (AGENTS.md reference), and Cursor (.mdc reference)
  2. User can run `aisync memory import claude` to pull Claude auto-memory updates into `.ai/memory/`
  3. User can define a hook in `.ai/hooks.toml` and see it translated to Claude Code settings.json format and OpenCode plugin stubs after sync
  4. User can run `aisync hooks list` to see all hooks and their per-tool translation status, including warnings for tools that don't support hooks (Cursor)
**Plans**: 5 plans

Plans:
- [ ] 03-01-PLAN.md — Foundation types, errors, managed sections, trait extension, memory engine
- [ ] 03-02-PLAN.md — Adapter memory sync implementations, memory CLI subcommands
- [ ] 03-03-PLAN.md — Hook engine, adapter hook translations
- [ ] 03-04-PLAN.md — Hook CLI subcommands, sync engine hook integration, status extension
- [ ] 03-05-PLAN.md — Gap closure: memory add --content flag, graceful import handling

### Phase 4: Watch Mode and Bidirectional Sync
**Goal**: Users can run a file-watching daemon that auto-syncs on changes, and edits to tool-native files reverse-sync back to the canonical `.ai/` directory
**Depends on**: Phase 3
**Requirements**: CLI-05, CLI-06, CLI-07, INST-08, INST-09
**Success Criteria** (what must be TRUE):
  1. User can run `aisync watch` and have changes to `.ai/instructions.md` automatically propagate to all configured tools within seconds
  2. User edits CLAUDE.md directly and the change reverse-syncs to `.ai/instructions.md` and then forward-syncs to other tools without infinite loops
  3. User can run `aisync diff` to see a side-by-side comparison of canonical content vs each tool's native file
  4. User can run `aisync check` in CI and it exits non-zero if any tool is out of sync with `.ai/`
  5. Conditional sections (`<!-- aisync:claude-only -->`) in instructions.md are included only in the relevant tool's output
**Plans**: 5 plans

Plans:
- [ ] 04-01-PLAN.md — Workspace deps, conditional processor, diff engine, SyncEngine conditional wiring
- [ ] 04-02-PLAN.md — Watch engine with forward/reverse sync and loop prevention
- [ ] 04-03-PLAN.md — CLI commands: watch, diff, check
- [ ] 04-04-PLAN.md — Gap closure: fix watch Ctrl+C hang and reverse sync file watching
- [ ] 04-05-PLAN.md — Gap closure: fix conditional section filtering for symlink adapters

### Phase 5: Polish and Distribution
**Goal**: aisync is installable via Homebrew, cargo install, and GitHub releases, with shell completions, polished error messages, and a comprehensive test suite
**Depends on**: Phase 4
**Requirements**: CLI-10, DIST-01, DIST-02, DIST-03, DIST-04, DIST-05, QUAL-01, QUAL-02, QUAL-03
**Success Criteria** (what must be TRUE):
  1. User can install aisync via `brew install aisync`, `cargo install aisync`, or downloading a pre-built binary from GitHub releases
  2. Shell completions work for bash, zsh, and fish after installation
  3. All error messages are clear and actionable, with `--verbose` providing structured debug output
  4. CI matrix runs tests on macOS, Linux, and Windows, and all pass
  5. Integration tests exercise full init-sync-status workflows against fixture projects with multiple tool configurations
**Plans**: 3 plans

Plans:
- [ ] 05-01-PLAN.md — Shell completions (clap_complete) and Cargo.toml publishing metadata
- [ ] 05-02-PLAN.md — Integration test suite (init, sync, status, round-trip)
- [ ] 05-03-PLAN.md — Cross-platform CI and cargo-dist release automation

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation and Data Model | 0/2 | Not started | - |
| 2. Core Sync Loop (MVP) | 0/4 | Not started | - |
| 3. Memory and Hooks | 0/5 | Not started | - |
| 4. Watch Mode and Bidirectional Sync | 0/3 | Not started | - |
| 5. Polish and Distribution | 0/3 | Not started | - |
