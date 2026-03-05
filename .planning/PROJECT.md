# aisync — Universal AI Agent Context Synchronizer

## What This Is

A standalone Rust CLI that keeps AI coding tool configurations in sync across Claude Code, OpenCode, Cursor, Windsurf, Codex, and others. It scaffolds a canonical `.ai/` directory as the single source of truth and syncs instructions, memory, hooks, and commands to each tool's native format — bidirectionally. Built for developers who use multiple AI tools on the same codebase and are tired of duplicating context.

## Core Value

Every AI tool working on a project sees the same instructions, memory, and hooks — always in sync, zero manual copying.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Detect which AI tools are configured for a project
- [ ] Scaffold canonical `.ai/` directory with `aisync.toml`, `instructions.md`, `memory/`, `hooks/`, `commands/`
- [ ] `aisync init` — interactive setup that imports existing configs and creates `.ai/`
- [ ] `aisync sync` — one-shot sync from `.ai/` to all detected tools
- [ ] `aisync watch` — daemon mode with file watching and auto-sync
- [ ] `aisync status` — show current sync state across all tools
- [ ] `aisync add-tool <tool>` — add support for a new tool to existing project
- [ ] `aisync memory` subcommands — list, add, import, export
- [ ] `aisync hooks` subcommands — list, add, translate
- [ ] Claude Code adapter (Tier 1) — instructions symlink, memory symlink, hooks translation
- [ ] OpenCode adapter (Tier 1) — AGENTS.md symlink, memory references, hook plugin stubs
- [ ] Cursor adapter (Tier 2) — .mdc generation, memory references
- [ ] Windsurf adapter (Tier 2) — .windsurfrules generation
- [ ] Codex adapter (Tier 2) — AGENTS.md/codex.md symlink
- [ ] Bidirectional sync — detect external edits to tool-native files, reverse-sync to `.ai/`
- [ ] Hook translation engine — canonical `.ai/hooks/*.toml` to tool-native formats
- [ ] Instructions translation — generate .mdc, .windsurfrules from `.ai/instructions.md`
- [ ] Conditional instruction sections — tool-specific content markers
- [ ] `aisync.toml` configuration — per-tool sync strategy, watch, bidirectional settings
- [ ] Community-grade CLI UX — clear error messages, help text, progress indicators
- [ ] Cross-platform — macOS (primary), Linux, Windows (best-effort, copy fallback for symlinks)
- [ ] Distribution — Homebrew tap, cargo install, GitHub releases with pre-built binaries, shell installer
- [ ] Comprehensive test suite — unit tests, integration tests with fixture projects

### Out of Scope

- MCP server config sync — complex tool-specific JSON schemas
- Plugin/extension sync — each tool's own ecosystem
- Auth/credential sharing — security concern
- IDE settings sync — use Settings Sync for that
- Chat history/session sync — proprietary, ephemeral
- GSD or workflow plugin sync — managed by plugin authors
- Tier 3 tools (Aider, Continue, PearAI) — deferred to v2+ community adapters

## Context

- Author uses Claude Code + OpenCode + Cursor daily on production projects
- Problem is felt viscerally — instructions drift across tools is a real productivity drain
- The `.ai/` directory convention doesn't exist yet; aisync would establish it
- Rust chosen for performance (file watching), cross-platform binary distribution, and reliability
- Several open questions to resolve during research: .ai/ gitignore policy, symlink vs copy defaults, tool-specific instruction sections, .gitignore management, Claude auto-memory path detection

## Constraints

- **Tech stack**: Rust — non-negotiable, chosen for binary distribution and performance
- **Platform**: macOS primary, Linux and Windows supported
- **Dependencies**: Prefer well-maintained crates (clap, notify, toml, serde, minijinja, similar, dialoguer, indicatif, dirs)
- **Quality bar**: Dogfooded on author's projects + public launch ready (Homebrew, docs, README, GH releases)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust for CLI | Binary distribution, performance for file watching, cross-platform | — Pending |
| `.ai/` as canonical directory | Neutral name, not tied to any tool vendor | — Pending |
| Adapter trait pattern | Clean separation for adding new tools | — Pending |
| Symlink-first sync strategy | Elegant, zero-copy, instant propagation | — Pending |
| TOML for hook specs | Readable, familiar to Rust ecosystem | — Pending |

---
*Last updated: 2026-03-05 after initialization*
