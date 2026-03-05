# Feature Landscape

**Domain:** Developer CLI tool for AI coding assistant configuration synchronization
**Researched:** 2026-03-05
**Confidence:** MEDIUM (training data knowledge, no web search available to verify latest tool changes)

## Table Stakes

Features users expect. Missing = product feels incomplete or untrustworthy.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| `init` command with auto-detection | Every config tool (chezmoi, stow, mackup) starts with "detect what exists." Users won't manually declare tools. | Medium | Must scan for `.claude/`, `.cursor/`, `CLAUDE.md`, `AGENTS.md`, `opencode.json`, `.windsurfrules`, `codex.md`. Detection must be fast and correct. |
| One-shot sync (`aisync sync`) | The atomic unit of value. If this doesn't work perfectly, nothing else matters. | Medium | Must handle symlink creation, file generation, and copy strategies. |
| Status command showing sync state | Users need to verify the tool is working. "Trust but verify" is universal in dotfile/config tools. | Low | Table-formatted output showing per-tool sync status. chezmoi has `managed`, stow has `--no`, users expect a status check. |
| Symlink-based sync (with copy fallback) | Symlinks are the expected mechanism for config sync tools (GNU Stow, chezmoi, dotbot all use them). Copy is needed for Windows and tools that don't follow symlinks. | Medium | Windows symlinks require Developer Mode or admin. Must detect and fall back gracefully. |
| TOML/YAML config file | Every serious CLI tool has a declarative config. Users expect to version-control their sync preferences alongside the project. | Low | `aisync.toml` is the right call -- TOML is idiomatic for Rust CLIs (Cargo.toml precedent). |
| Idempotent operations | Running `sync` twice must produce the same result. Config tools that aren't idempotent are immediately distrusted. | Medium | Must check existing symlinks, skip unchanged files, not duplicate content. |
| Clear error messages and dry-run mode | Dotfile managers taught users to expect `--dry-run` / `--verbose`. Users are nervous about tools that modify config files. | Low | `--dry-run` flag on `sync`, `init`. Show exactly what will be created/modified/deleted before doing it. |
| Cross-platform support (macOS + Linux) | Any Rust CLI is expected to work on macOS and Linux. Windows is a bonus but macOS/Linux are table stakes. | Low | Rust handles this naturally. The complexity is in platform-specific paths (`~/.claude/` vs `~/.config/`). |
| Instructions sync (markdown to all tools) | This is the primary use case. If `.ai/instructions.md` doesn't correctly produce `CLAUDE.md`, `AGENTS.md`, `.cursor/rules/*.mdc`, and `.windsurfrules`, the tool has no reason to exist. | High | Each tool has different format requirements. Cursor needs YAML frontmatter in `.mdc`. Windsurf is plain markdown. Claude/OpenCode/Codex use plain markdown but different filenames. |
| Import existing configs on init | Users already have `CLAUDE.md` or `.cursorrules`. A tool that ignores existing config and overwrites it is hostile. Must import and merge. | Medium | Diff detection between existing tool configs, conflict resolution prompts. |
| `.gitignore` awareness | Tool-generated files (`.cursor/`, `.windsurfrules`) should be gitignored. The canonical `.ai/` should be committed. Users expect the tool to handle this or at least advise. | Low | Suggest additions to `.gitignore` during init, or auto-add with confirmation. |

## Differentiators

Features that set the product apart. Not expected (no real competitors exist yet), but high-value.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Bidirectional sync | Most config sync tools are unidirectional (source -> targets). Bidirectional sync -- detecting when a user edits `CLAUDE.md` directly and reverse-syncing to `.ai/instructions.md` -- is genuinely novel for this domain. | High | Symlinked files get this for free (edits propagate through the symlink). Copied/generated files need change detection, diffing, and merge logic. This is the hardest feature and the most valuable. |
| File watch daemon (`aisync watch`) | Eliminates the need to manually run `sync`. Changes to `.ai/` auto-propagate. This is what makes the tool feel "magical." | Medium | `notify` crate handles cross-platform file watching. Need debouncing, error recovery, and clean daemon lifecycle. |
| Memory sync across tools | No other tool does this. Claude Code has auto-memory in `~/.claude/projects/*/memory/`. Making that knowledge available to Cursor (via `.mdc` references) and OpenCode (via `AGENTS.md` references) is a unique value prop. | High | Claude's memory path uses a mangled project directory name. Need to auto-detect this path. Memory files need to be referenced differently per tool. |
| Hook translation engine | Translating a canonical hook definition into Claude Code's `settings.json` format and OpenCode's plugin format is genuinely useful. No one else does this. | High | Each tool has very different hook semantics. Claude uses stdin JSON with PostToolUse/PreToolUse events. OpenCode uses JS plugin functions. Cursor and Windsurf have no hook support at all. The translation must be honest about gaps. |
| Conditional instruction sections | Tool-specific content markers (`<!-- aisync:claude-only -->`) that include/exclude content per tool. "Use the Edit tool" is meaningless to Cursor; "use Cmd+K" is meaningless to Claude Code. | Medium | Template preprocessing with marker-based conditional blocks. Similar to C preprocessor `#ifdef` but for markdown sections. |
| `aisync diff` (canonical vs actual) | Show exactly what each tool currently sees versus what `.ai/` says it should see. Debugging tool for when things look wrong. | Medium | Compare canonical source against each tool's actual file content. Use `similar` crate for diffing. |
| Interactive conflict resolution | When bidirectional sync detects divergent edits, present a TUI merge interface rather than silently overwriting. | High | Requires a TUI library (ratatui or similar). Most users would accept "pick canonical or pick tool-native" as simpler UX. Full 3-way merge is overkill for v1. |
| `add-tool` command | Incrementally add support for a new tool to an existing project. Smooth onboarding when someone starts using Cursor alongside Claude Code. | Low | Detect the new tool, generate its config from existing `.ai/` content. Straightforward adapter invocation. |
| Memory subcommands (list, add, import, export) | Dedicated memory management makes the shared knowledge base a first-class concept, not just "files in a directory." | Low | Mostly filesystem operations with nice formatting. `import claude` needs to find and copy from Claude's auto-memory path. |
| Hooks subcommands (list, add, translate) | Manage cross-tool hooks without hand-editing TOML. The `translate` subcommand showing each tool's version is great for debugging. | Low | TOML scaffolding and adapter invocation for translation preview. |
| Shell completions | Tab completion for commands, tool names, and memory topics. Expected in polished CLIs. | Low | `clap` generates completions for bash/zsh/fish automatically via `clap_complete`. |
| `aisync check` (CI validation) | Verify that all tool configs are in sync with `.ai/` in CI. Fails if someone edited `CLAUDE.md` without running `aisync sync`. | Low | Compare expected vs actual, exit non-zero on drift. Valuable for teams. |

## Anti-Features

Features to explicitly NOT build. Each is a scope trap.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| MCP server config sync | MCP configs are deeply tool-specific JSON with server URIs, auth tokens, environment variables. The schema varies per MCP server. Syncing these is a different product. | Document in README that MCP config is out of scope. Users manage MCP per-tool. |
| IDE settings sync (themes, keybindings, extensions) | VS Code Settings Sync, JetBrains Sync already solve this. Duplicating their work adds maintenance burden with no unique value. | Point users to existing solutions. Stay focused on AI agent context. |
| Chat history / session sync | Proprietary formats, ephemeral by nature, potentially contains sensitive data. No tool exposes a stable API for this. | Not even worth mentioning. Users don't expect this. |
| Auth / credential sharing | Security liability. API keys, tokens, and credentials must never be synced by a config tool. | Explicitly warn if `.ai/` files contain anything that looks like a secret. Add `.ai/secrets/` to default `.gitignore`. |
| Plugin/extension marketplace | Building a plugin ecosystem is a product in itself. Adapter plugins (WASM, dynamic loading) add massive complexity for marginal v1 value. | Ship with built-in adapters for Tier 1 and Tier 2 tools. Design the adapter trait to be extensible for v2+ community contributions, but don't build the plugin loading infrastructure in v1. |
| GUI / TUI dashboard | A full TUI for managing sync state is overengineered for v1. The CLI output is sufficient. A TUI adds ratatui dependency, layout complexity, and testing burden. | Use `indicatif` for progress and `dialoguer` for prompts. Table-formatted status output. Save TUI for v2 if there's demand. |
| Global (non-project) config sync | Syncing `~/.claude/CLAUDE.md` with `~/.config/opencode/AGENTS.md` is a different use case (personal dotfiles vs project config). Mixing them conflates two concerns. | Focus on project-level `.ai/` sync. Document that global configs are managed separately. Consider a `--global` flag for v2. |
| Auto-updating / self-update | Self-updating binaries are a security and distribution headache. Homebrew and cargo handle updates already. | Distribute via Homebrew tap, cargo install, and GitHub releases. Let package managers handle updates. |
| Template / starter kit generation | Generating boilerplate `.ai/instructions.md` for different project types (React, Rust, Python) is content creation, not config sync. | Ship with a minimal example `instructions.md`. Link to community examples in docs. |

## Feature Dependencies

```
Detection Engine → Init (init requires detection to know what to import)
Detection Engine → Sync (sync needs to know which adapters to invoke)
Detection Engine → Status (status shows per-tool state)

Adapter Trait → All tool-specific adapters (claude, opencode, cursor, windsurf, codex)
Adapter Trait → Sync Engine (sync invokes adapters)
Adapter Trait → Hook Translation (adapters define how to translate hooks)

Init → Sync (init does an initial sync after scaffolding)
Init → Import Logic (init imports existing configs)

Sync Engine → Watch (watch triggers sync on file changes)
Sync Engine → Bidirectional Sync (reverse sync uses the same engine in reverse)

Instructions Sync → Memory Sync (memory references are embedded in generated instruction files)
Instructions Sync → Conditional Sections (tool-specific markers are processed during instructions generation)

Config Parsing (aisync.toml) → Everything (all commands read the config)

Diff Engine (similar) → Bidirectional Sync (detecting and resolving conflicts)
Diff Engine (similar) → Status (comparing canonical vs actual)
Diff Engine (similar) → aisync diff command

File Watcher (notify) → Watch command
File Watcher (notify) → Bidirectional Sync (detecting external edits)
```

## MVP Recommendation

Build in this priority order. Each layer unlocks testable, demoable value.

### Phase 1: Core Loop (must ship together)

1. **Detection Engine** -- scan project root for AI tool config files
2. **Config parsing** -- read/write `aisync.toml`
3. **Instructions sync** -- the primary value prop. `.ai/instructions.md` to all tool formats
4. **Claude Code adapter** (Tier 1) -- symlink to `CLAUDE.md`
5. **OpenCode adapter** (Tier 1) -- symlink to `AGENTS.md`
6. **`aisync init`** -- scaffold `.ai/`, import existing configs, initial sync
7. **`aisync sync`** -- one-shot sync with `--dry-run` support
8. **`aisync status`** -- show what's synced and what's drifted

**Rationale:** This is the minimum viable product. A developer with Claude Code and OpenCode can `aisync init`, see their configs imported, run `aisync sync`, and have both tools reading the same instructions. Every feature after this builds on this foundation.

### Phase 2: Breadth (more tools, more content types)

9. **Cursor adapter** (Tier 2) -- `.mdc` generation with YAML frontmatter
10. **Windsurf adapter** (Tier 2) -- `.windsurfrules` generation
11. **Codex adapter** (Tier 2) -- `AGENTS.md`/`codex.md` symlink
12. **Memory sync** -- `.ai/memory/` files referenced/symlinked per tool
13. **`aisync memory` subcommands** -- list, add, import, export
14. **`aisync add-tool`** -- add a new tool to existing project

### Phase 3: Depth (automation, hooks, watch)

15. **File watch daemon** (`aisync watch`) -- auto-sync on changes
16. **Bidirectional sync** -- detect external edits, reverse-sync to `.ai/`
17. **Hook translation engine** -- canonical `.ai/hooks/*.toml` to tool-native formats
18. **`aisync hooks` subcommands** -- list, add, translate
19. **Conditional instruction sections** -- tool-specific content markers

### Defer to v2+

- **`aisync diff`** -- valuable but not essential for launch
- **`aisync check`** (CI mode) -- important for teams, not solo devs
- **Interactive conflict resolution** -- simple "pick one" prompts are fine for v1
- **Shell completions** -- nice polish, easy to add later
- **Plugin SDK / Tier 3 adapters** -- community-driven growth after proving the concept

## Competitive Landscape Notes

There is no direct competitor doing exactly what aisync proposes. The closest analogues are:

- **Dotfile managers** (chezmoi, GNU Stow, dotbot, yadm): Manage personal config files across machines, not AI tool configs across tools. They solve a related problem (config sync) but in a different dimension (across machines vs across tools on one machine). aisync can learn from their UX patterns (idempotent operations, dry-run, status checks, template rendering) without competing with them.

- **`.cursorrules` / `.windsurfrules` generators**: Various community scripts and repos that generate rule files for Cursor or Windsurf. These are one-off generators, not sync tools. They solve "get started" but not "stay in sync."

- **AGENTS.md convention**: Emerging as an informal standard across tools (OpenCode, Cursor, Codex all read it). aisync should treat AGENTS.md as a sync target, not fight the convention. The `.ai/instructions.md` canonical source generates AGENTS.md as one of its outputs.

- **Claude Code's CLAUDE.md**: The most mature per-project instruction convention. Many developers' instructions already live here. aisync must handle importing from CLAUDE.md gracefully -- this is likely where most users' canonical content already lives.

The real competition is "doing nothing" -- developers who just maintain separate files manually or who only use one AI tool. aisync's pitch is that multi-tool usage is increasing and manual sync doesn't scale.

## Sources

- PROJECT.md and PRD.md from this repository (primary context)
- Training data knowledge of chezmoi, GNU Stow, dotbot, yadm feature sets
- Training data knowledge of Claude Code, Cursor, Windsurf, OpenCode, Codex configuration conventions
- Confidence is MEDIUM overall due to inability to verify latest tool configuration format changes via web search
