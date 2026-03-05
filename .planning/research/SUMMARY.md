# Project Research Summary

**Project:** aisync -- Universal AI Agent Context Synchronizer
**Domain:** Developer CLI tool (Rust) for cross-tool AI coding assistant configuration synchronization
**Researched:** 2026-03-05
**Confidence:** MEDIUM

## Executive Summary

aisync is a local-only Rust CLI tool that synchronizes AI coding assistant configurations across tools (Claude Code, OpenCode, Cursor, Windsurf, Codex) using a canonical `.ai/` directory as the single source of truth. The established pattern for this class of tool is a **core-adapter architecture**: a sync engine owns the canonical model, and tool-specific adapters translate to/from native formats (symlinks for simple cases, template generation for tools like Cursor that need YAML frontmatter in `.mdc` files). The closest analogues are dotfile managers (chezmoi, GNU Stow), but aisync operates in a different dimension -- syncing across tools on one machine rather than across machines. There are no direct competitors; the real competition is "doing nothing."

The recommended approach is synchronous Rust (no async runtime), a Cargo workspace with separate library and binary crates, and trait-based adapters compiled into a single binary. The stack is mature and well-understood: clap for CLI, serde+toml for config, notify for file watching, minijinja for template generation, similar for diffing. All core crates are high-confidence choices. The critical architectural decision -- explicitly synchronous, no tokio -- is correct because all I/O is local filesystem with no network calls.

The primary risks are: (1) bidirectional sync infinite loops in watch mode, which is a distributed consensus problem that must be solved with write tracking and debouncing before shipping; (2) Windows symlink permissions requiring a first-class copy fallback from day one; (3) AI tool config format drift causing silent breakage where sync "works" but the tool ignores the output. The mitigation strategy is to build forward-only sync first, ship it, dogfood it with Claude Code and OpenCode, and defer bidirectional sync and watch mode to a later phase. The adapter trait should include a `validate()` method from the start to catch format drift.

## Key Findings

### Recommended Stack

The stack is entirely synchronous Rust with well-established crates. No async runtime, no network dependencies. This keeps the binary small, the code simple, and the dependency tree manageable.

**Core technologies:**
- **clap (derive)**: CLI parsing with subcommands -- industry standard, compile-time validation, shell completions for free
- **serde + toml**: Config serialization -- TOML is idiomatic for Rust CLIs (Cargo.toml precedent), serde is non-negotiable
- **notify + notify-debouncer-full**: Cross-platform file watching -- only serious option in Rust, wraps OS-native APIs
- **minijinja**: Template engine for generating tool-native formats -- lightweight, Jinja2-compatible, by Armin Ronacher
- **similar**: Text diffing with patience algorithm -- needed for conflict detection in bidirectional sync
- **dialoguer + indicatif + console**: Interactive CLI UX -- prompts for init wizard, progress bars for sync
- **anyhow + thiserror**: Error handling -- anyhow in CLI code for context, thiserror in library code for typed errors
- **tracing**: Structured logging -- better than `log` for debugging sync operations across adapters

**Critical version note:** All version numbers are from training data (early 2025). Run `cargo search <crate>` to verify before adding to Cargo.toml. API recommendations are high-confidence regardless of exact versions.

### Expected Features

**Must have (table stakes):**
- `aisync init` with auto-detection of existing tool configs and import flow
- `aisync sync` one-shot forward sync with `--dry-run` support
- `aisync status` showing per-tool sync state and drift detection
- Instructions sync: `.ai/instructions.md` correctly producing CLAUDE.md, AGENTS.md, .mdc, .windsurfrules
- Symlink-based sync with copy fallback (especially Windows)
- Idempotent operations -- running sync twice produces same result
- TOML config file (`aisync.toml`) with schema versioning from day one
- Import existing configs on init (never overwrite without consent)
- `.gitignore` awareness and guidance

**Should have (differentiators):**
- Bidirectional sync (detecting edits to tool-native files, reverse-syncing to canonical)
- File watch daemon (`aisync watch`) for auto-sync on changes
- Memory sync across tools (Claude Code's auto-memory shared with other tools)
- Conditional instruction sections (tool-specific content markers)
- `aisync diff` showing canonical vs actual per tool
- `aisync add-tool` for incremental tool adoption
- `aisync check` for CI validation of sync drift

**Defer (v2+):**
- Interactive TUI conflict resolution (simple "pick one" prompts suffice for v1)
- Shell completions (easy to add, low priority)
- Plugin SDK / community adapter loading
- Hook translation engine (tool hook formats vary wildly, high complexity)
- Global (non-project) config sync

**Anti-features (do NOT build):**
- MCP server config sync, IDE settings sync, chat history sync, auth/credential sharing, GUI/TUI dashboard, auto-updating, template/starter kit generation

### Architecture Approach

Core-adapter architecture with a Cargo workspace: `aisync-core` (library with models, traits, engine, adapters) and `aisync-cli` (thin binary shell). The canonical model is an intermediate representation -- all translations go through it, never directly between tools (avoids N*N translation paths). Adapters implement a `ToolAdapter` trait with `detect()`, `read_native()`, `write_native()`, `to_canonical()`, and `watched_paths()` methods.

**Major components:**
1. **Canonical Model** -- in-memory representation of `.ai/` contents (instructions, memory, hooks, commands)
2. **Sync Engine** -- orchestrates forward/reverse sync, invokes adapters, handles conflicts
3. **Tool Adapters** -- trait implementations for each tool (Claude, OpenCode, Cursor, Windsurf, Codex)
4. **Config Parser** -- reads/writes `aisync.toml`, schema versioned from day one
5. **Tool Detector** -- scans project for tool markers to auto-discover active tools
6. **Differ** -- compares canonical model vs tool-native state using `similar` crate
7. **Watcher** -- file system monitoring via `notify` with debouncing and loop guard

**Key architectural decisions:**
- Explicitly synchronous (no tokio) -- all I/O is local filesystem
- Symlink default on macOS/Linux, copy default on Windows
- Templates embedded in adapter code (not external files) for single-binary distribution
- State tracking via `.ai/.aisync-state.json` (file hashes, timestamps, sync direction)

### Critical Pitfalls

1. **Bidirectional sync infinite loop** -- Watch mode writes trigger re-detection, causing spiraling sync. Prevent with write-origin tracking, debounce windows (200-500ms), and generation counters in generated files. Do NOT ship watch mode without loop prevention.
2. **Windows symlink permissions** -- Symlinks require Developer Mode or admin. Must detect capability at runtime, fall back to copy strategy, and surface clear error messages. Design the `FileStrategy` enum (Symlink | Copy) into the adapter trait from phase 1.
3. **Tool config format drift** -- AI tools change config formats between versions (e.g., Cursor's .cursorrules to .cursor/rules/ migration). Sync appears to work but tool ignores output. Prevent with adapter-level `validate()` method and version detection.
4. **Race conditions in file watcher event batching** -- Editors use atomic save (write-tmp-rename), producing multiple events. Debounce all events per path (100-300ms) and verify file readability before processing.
5. **Dangling symlinks after git operations** -- Branch switching can leave symlinks pointing at missing targets. Always validate symlink targets in `aisync status`. Design for `.ai/` being git-tracked as default.

## Implications for Roadmap

### Phase 1: Foundation and Data Model
**Rationale:** Everything depends on the data structures. Get the canonical model, config schema, error types, and adapter trait right before writing any sync logic. Extract the trait from two working adapters rather than designing it upfront.
**Delivers:** Canonical model structs, `aisync.toml` schema (with `schema_version = 1`), `ToolAdapter` trait definition, error types, tool detection engine.
**Addresses:** Config parsing (table stakes), schema versioning (Pitfall 6)
**Avoids:** Overengineered adapter trait (Pitfall 14) -- implement Claude and OpenCode adapters first, then extract shared trait

### Phase 2: Core Sync Loop (MVP)
**Rationale:** This is the minimum viable product. Forward-only sync from `.ai/` to tool-native formats, with the two Tier 1 adapters the author uses daily. Must be testable end-to-end.
**Delivers:** `aisync init` (with import flow and backup), `aisync sync` (forward-only, with `--dry-run`), `aisync status`, Claude Code adapter, OpenCode adapter
**Addresses:** Init with auto-detection, one-shot sync, status command, instructions sync, import existing configs, idempotency, `.gitignore` awareness
**Avoids:** Windows symlink issues (Pitfall 2) by building FileStrategy abstraction; first-run confusion (Pitfall 11) with import flow; dangling symlinks (Pitfall 3) with validation in status

### Phase 3: Tool Breadth (Tier 2 Adapters)
**Rationale:** Expand to Cursor, Windsurf, and Codex. These adapters are primarily template-based (generating .mdc with frontmatter, .windsurfrules) and benefit from the template engine. Memory sync fits here because it extends the canonical model established in Phase 1.
**Delivers:** Cursor adapter (.mdc generation), Windsurf adapter, Codex adapter, memory sync (`.ai/memory/`), `aisync add-tool`, `aisync memory` subcommands
**Addresses:** All Tier 2 tool support, memory sync differentiator
**Avoids:** Content translation fidelity loss (Pitfall 9) with round-trip tests; format drift (Pitfall 5) with validate-after-sync

### Phase 4: Watch Mode and Bidirectional Sync
**Rationale:** This is the hardest phase and the most valuable differentiator. Defer until forward sync is proven stable through dogfooding. Bidirectional sync is a distributed consensus problem that needs careful loop prevention.
**Delivers:** `aisync watch` daemon, bidirectional sync (tool-native changes reverse-sync to canonical), conflict resolution (prefer-canonical | prefer-tool | prompt), conditional instruction sections
**Addresses:** Watch daemon, bidirectional sync, conditional sections
**Avoids:** Infinite loop (Pitfall 1) with write tracking + debounce + generation counters; race conditions (Pitfall 4) with debounced events; platform divergence (Pitfall 8) with periodic reconciliation; stale lock files (Pitfall 12) with PID-based locks

### Phase 5: Polish and Distribution
**Rationale:** CI validation, diff command, and distribution are polish items that round out the product after core functionality is proven.
**Delivers:** `aisync diff`, `aisync check` (CI mode), shell completions, Homebrew tap, cargo-dist GitHub releases, cross-platform CI matrix
**Addresses:** CI validation differentiator, distribution

### Phase Ordering Rationale

- **Data model before sync logic** because every adapter and the engine depend on the canonical model structs. Changing these later cascades through the entire codebase.
- **Two adapters before trait extraction** (Pitfall 14) because real adapter code reveals what the trait actually needs. The Claude and OpenCode adapters are the simplest (symlink-based) and the author dogfoods them daily.
- **Forward sync before bidirectional** because forward sync is immediately useful and dramatically simpler. Bidirectional sync introduces conflict resolution, loop prevention, and distributed state tracking -- each a significant subsystem.
- **Tier 2 adapters before watch mode** because breadth (more tools) delivers more user value faster than depth (automation) and is lower risk. Template-based adapters for Cursor/Windsurf are well-understood patterns.
- **Watch mode last among core features** because it concentrates the hardest pitfalls (loops, race conditions, platform divergence) and benefits from a stable sync engine underneath.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 3 (Cursor adapter):** Cursor's .mdc format with YAML frontmatter needs verification against current Cursor version. The format has changed before and may change again.
- **Phase 4 (Watch + Bidirectional):** Complex integration of notify crate cross-platform behavior, loop prevention strategies, and conflict resolution UX. Needs phase-level research.
- **Phase 4 (Conditional sections):** The marker syntax and template preprocessing approach needs design research -- no established pattern exists for this specific use case.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Foundation):** Standard Rust data modeling, serde derives, trait definition. Well-documented patterns.
- **Phase 2 (Core Sync):** CLI scaffolding with clap, symlink creation, basic file operations. Straightforward Rust.
- **Phase 5 (Distribution):** cargo-dist, Homebrew taps, GitHub Actions CI -- all well-documented.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All recommended crates are established Rust ecosystem staples. Exact version numbers need verification via `cargo search`. |
| Features | MEDIUM | Feature landscape is sound but AI tool config formats are in flux (2024-2026). Specific format details for Cursor .mdc, Windsurf rules need verification. |
| Architecture | HIGH | Core-adapter pattern is well-established. Cargo workspace with lib+bin split is standard Rust practice. Sync engine design follows proven patterns from dotfile managers. |
| Pitfalls | MEDIUM | Pitfalls are drawn from domain expertise in file sync systems and cross-platform development. Specific notify crate behavior per platform should be verified with current crate docs. |

**Overall confidence:** MEDIUM -- The architecture and stack choices are high-confidence. The uncertainty is concentrated in AI tool config format specifics (which change frequently) and the bidirectional sync / watch mode complexity (which is inherently hard and needs implementation-time validation).

### Gaps to Address

- **AI tool config format verification:** Current Cursor .mdc format, Windsurf .windsurfrules location, Codex config conventions, and Claude Code memory path structure all need verification against latest tool versions before implementing adapters. Research each tool's current docs during the phase that implements its adapter.
- **notify crate version confirmation:** Research references notify v6/v7 behavior. Confirm current stable version and API surface with `cargo search notify` before Phase 4.
- **Windows CI availability:** Cross-platform testing on Windows is flagged as important but no specific CI configuration was researched. Determine GitHub Actions Windows runner capabilities during Phase 2.
- **Markdown parsing strategy:** PITFALLS.md recommends using a proper Markdown parser (pulldown-cmark or comrak) for content translation. STACK.md does not include this dependency. Evaluate whether minijinja templates are sufficient or a Markdown parser is needed during Phase 3 adapter work.
- **Claude Code memory path discovery:** The exact mechanism for finding Claude Code's auto-memory directory (which uses a mangled project directory name) needs investigation during Phase 3 memory sync work.

## Sources

### Primary (HIGH confidence)
- Project requirements: PROJECT.md and PRD.md from this repository
- Rust crate ecosystem: clap, serde, notify, minijinja, similar -- well-documented, stable APIs
- Architecture patterns: Adapter/strategy pattern, Cargo workspace conventions

### Secondary (MEDIUM confidence)
- AI tool config formats: Based on training data knowledge of Claude Code, Cursor, Windsurf, OpenCode, Codex conventions as of early 2025
- Dotfile manager patterns: chezmoi, GNU Stow, dotbot feature sets and UX patterns
- File sync domain: Unison, Syncthing, rsync architectural lessons applied to this domain
- Cross-platform file system behavior: Windows symlink restrictions, editor save patterns, notify crate platform divergence

### Tertiary (LOW confidence)
- Exact crate version numbers: Need verification via `cargo search` before implementation
- Current AI tool config format specifics: Tools are in rapid flux; verify against current docs before each adapter phase

---
*Research completed: 2026-03-05*
*Ready for roadmap: yes*
