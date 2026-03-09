# aisync — Universal AI Agent Context Synchronizer

## What This Is

A standalone Rust CLI that keeps AI coding tool configurations in sync across Claude Code, OpenCode, Cursor, Windsurf, and Codex. It scaffolds a canonical `.ai/` directory as the single source of truth and syncs instructions, memory, hooks, rules, MCP servers, and commands to each tool's native format — bidirectionally. Includes a two-layer Plugin SDK (declarative TOML + Rust trait) for community adapter development, and a security scanner that prevents API key leaks in synced configs.

## Core Value

Every AI tool working on a project sees the same instructions, memory, hooks, rules, MCP servers, and commands — always in sync, zero manual copying.

## Requirements

### Validated

- ✓ Detect which AI tools are configured for a project — v1.0
- ✓ Scaffold canonical `.ai/` directory with `aisync.toml`, `instructions.md`, `memory/`, `hooks/` — v1.0
- ✓ `aisync init` — interactive setup that imports existing configs and creates `.ai/` — v1.0
- ✓ `aisync sync` — one-shot sync from `.ai/` to all detected tools — v1.0
- ✓ `aisync watch` — daemon mode with file watching and auto-sync — v1.0
- ✓ `aisync status` — show current sync state across all tools — v1.0
- ✓ `aisync memory` subcommands — list, add, import, export — v1.0
- ✓ `aisync hooks` subcommands — list, add, translate — v1.0
- ✓ Claude Code adapter (Tier 1) — instructions symlink, memory symlink, hooks translation — v1.0
- ✓ OpenCode adapter (Tier 1) — AGENTS.md symlink, memory references, hook plugin stubs — v1.0
- ✓ Cursor adapter (Tier 1) — .mdc generation, memory references — v1.0
- ✓ Bidirectional sync — detect external edits to tool-native files, reverse-sync to `.ai/` — v1.0
- ✓ Hook translation engine — canonical `.ai/hooks/*.toml` to tool-native formats — v1.0
- ✓ Conditional instruction sections — tool-specific content markers — v1.0
- ✓ `aisync.toml` configuration — per-tool sync strategy, watch, bidirectional settings — v1.0
- ✓ Community-grade CLI UX — clear error messages, help text, shell completions — v1.0
- ✓ Cross-platform — macOS (primary), Linux, Windows (copy fallback for symlinks) — v1.0
- ✓ Distribution — Homebrew tap, cargo install, GitHub releases, shell installer — v1.0
- ✓ Comprehensive test suite — 188 tests (174 unit + 14 integration) — v1.0
- ✓ ToolAdapter trait provides all tool metadata — v1.1
- ✓ ToolsConfig supports arbitrary tool names via BTreeMap — v1.1
- ✓ AnyAdapter Plugin variant for dynamic dispatch — v1.1
- ✓ Display name logic consolidated — v1.1
- ✓ Windsurf adapter — `.windsurf/rules/project.md` with YAML frontmatter — v1.1
- ✓ Codex adapter — AGENTS.md symlink with `.codex/` detection — v1.1
- ✓ AGENTS.md deduplication when both Codex and OpenCode present — v1.1
- ✓ Legacy `.windsurfrules` detection with migration hint — v1.1
- ✓ Content size limit warnings (Windsurf 12K chars, Codex 32 KiB) — v1.1
- ✓ `aisync add-tool` — auto-detect, interactive select, partial sync — v1.1
- ✓ `aisync-types` crate — shared types for community adapters — v1.1
- ✓ `aisync-adapter` crate — ToolAdapter trait SDK — v1.1
- ✓ Declarative TOML adapter schema with detection, mapping, templates — v1.1
- ✓ `.ai/adapters/*.toml` auto-discovery — v1.1
- ✓ Compile-time registration via inventory — v1.1
- ✓ Adapter authoring documentation (TOML + Rust paths) — v1.1
- ✓ Multi-file rule sync — `.ai/rules/*.md` with YAML frontmatter to Cursor `.mdc` and Windsurf `.md` — v1.2
- ✓ Single-file tool rule concatenation — Claude Code, OpenCode, Codex get appended rule content — v1.2
- ✓ Rule import during init — Cursor `.mdc` and Windsurf `.md` imported to `.ai/rules/` — v1.2
- ✓ Managed file prefix (`aisync-`) — user-created native rules never overwritten — v1.2
- ✓ Stale managed file cleanup — removed when canonical source deleted — v1.2
- ✓ MCP server config sync — `.ai/mcp.toml` generates Claude Code and Cursor JSON — v1.2
- ✓ MCP secret stripping — hardcoded env values replaced with `${VAR}` references — v1.2
- ✓ MCP import — existing tool configs merged into `.ai/mcp.toml` — v1.2
- ✓ Security scanner — regex-based API key detection with non-blocking warnings — v1.2
- ✓ Command sync — `.ai/commands/` to `.claude/commands/` and `.cursor/commands/` — v1.2
- ✓ Command import — existing `.claude/commands/` imported during init — v1.2
- ✓ Init completeness — zero drift after init, ghost tool filtering, correct messaging — v1.2
- ✓ Type foundation — RuleFile, McpConfig, CommandFile types and adapter trait methods — v1.2

### Active

(No active requirements — next milestone not yet planned)

### Out of Scope

- MCP server config sync beyond instructions-level — complex tool-specific JSON schemas (auth flows, SSE transports)
- Plugin/extension sync — each tool's own ecosystem
- Auth/credential sharing — security concern
- IDE settings sync — use Settings Sync for that
- Chat history/session sync — proprietary, ephemeral
- Dynamic plugin loading (dylib/WASM) — prove interface stability first
- Aider adapter — good first community adapter candidate via Plugin SDK
- Continue adapter — good first community adapter candidate via Plugin SDK
- PearAI/Tier 3 tools — deferred to community adapters
- Codex hierarchical AGENTS.md — per-subdirectory sync adds significant complexity
- Runtime adapter hot-reloading — add in future version
- Windsurf MCP writing — global-only config, too risky to write to
- Skill supporting files sync — complex tool-specific assets
- Bidirectional multi-file rule sync — reverse-sync external edits back to `.ai/rules/` (deferred to v1.3)
- Cursor folder-based rules — post-v2.2 format, currently unstable (deferred to v1.3)

## Context

Shipped v1.2 with 16,917 lines of Rust across 4 workspace crates (aisync, aisync-core, aisync-types, aisync-adapter). Three milestones completed in 5 days (2026-03-05 to 2026-03-09), 16 phases, 42 plans total.

Tech stack: Rust 2024 edition, clap, notify, toml, serde, similar, dialoguer, dirs, inventory, minijinja, regex.
5 built-in adapters (Claude Code, OpenCode, Cursor, Windsurf, Codex) + TOML and inventory plugin adapters.
5 sync dimensions: instructions, memory, hooks, rules, MCP servers, commands.
339+ tests passing. Security scanner with 6 regex patterns for API key detection.

Stress-tested against whk-wms (production NestJS/Next.js monorepo) — all identified gaps closed in v1.2.

## Constraints

- **Tech stack**: Rust — non-negotiable, chosen for binary distribution and performance
- **Platform**: macOS primary, Linux and Windows supported
- **Dependencies**: Prefer well-maintained crates (clap, notify, toml, serde, minijinja, similar, dialoguer, indicatif, dirs, inventory, regex)
- **Quality bar**: Dogfooded on author's projects + public launch ready (Homebrew, docs, README, GH releases)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust for CLI | Binary distribution, performance for file watching, cross-platform | ✓ Good — 17k LOC, fast builds, single binary |
| `.ai/` as canonical directory | Neutral name, not tied to any tool vendor | ✓ Good — clean separation |
| Adapter trait pattern | Clean separation for adding new tools | ✓ Good — 5 adapters with consistent interface |
| Symlink-first sync strategy | Elegant, zero-copy, instant propagation | ✓ Good — with copy fallback for Windows/Cursor |
| TOML for hook specs | Readable, familiar to Rust ecosystem | ✓ Good — clean hook definitions |
| Enum-dispatch (AnyAdapter) | Fixed adapter set, no dyn Trait overhead | ✓ Good — compile-time dispatch + Plugin variant for extensibility |
| Cursor promoted to Tier 1 | Same implementation effort as Tier 2 | ✓ Good — shipped with Claude Code and OpenCode |
| Forward sync before bidirectional | Prove engine stable before adding complexity | ✓ Good — Phase 4 built on solid Phase 2 foundation |
| notify crate in core (not CLI) | WatchEngine needs filesystem events at library level | ✓ Good — clean architecture |
| Refactor before expanding | ToolAdapter trait expansion before new adapters | ✓ Good — eliminated shotgun surgery, single-file adapter additions |
| Two-layer Plugin SDK | TOML for simple adapters, Rust for complex ones | ✓ Good — low barrier to entry while keeping full power available |
| Compile-time registration | inventory crate for adapter discovery | ✓ Good — zero runtime overhead, proven pattern |
| Crate extraction (types + adapter) | Publishable SDK for community | ✓ Good — minimal deps, clean API surface |
| BTreeMap for ToolsConfig | Support arbitrary tool names from TOML/plugins | ✓ Good — backward compatible with named tools |
| Arc<dyn ToolAdapter> for Plugin | Clone+Send+Sync compatibility | ✓ Good — clean dispatch through AnyAdapter |
| Three-tier deduplication | builtin > TOML > inventory priority | ✓ Good — predictable behavior, no conflicts |
| Hand-parsed YAML frontmatter | No serde_yml dependency for simple key-value schema | ✓ Good — minimal dependencies, fast parsing |
| aisync- prefix for managed files | User-created native rules/commands never overwritten | ✓ Good — clean coexistence |
| LazyLock for regex compilation | Stable since Rust 1.80, zero-cost after first use | ✓ Good — thread-safe, efficient pattern matching |
| Forward-only multi-file rules | Bidirectional deferred to v1.3 for complexity | ✓ Good — simpler v1.2, cleaner mental model |
| Non-blocking security warnings | Scanner warns but doesn't block sync operations | ✓ Good — user awareness without friction |

---
*Last updated: 2026-03-09 after v1.2 milestone*
