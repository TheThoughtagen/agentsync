# aisync — Universal AI Agent Context Synchronizer

## Product Requirements Document

**Version**: 0.1.0-draft
**Date**: 2026-03-05
**Author**: Patrick Mannion + Claude Opus 4.6

---

## 1. Problem Statement

Modern software projects use multiple AI coding tools simultaneously — Claude Code, OpenCode, Cursor, Windsurf, Codex, and others. Each tool has its own conventions for project instructions, memory, hooks, and rules:

| Concept | Claude Code | OpenCode | Cursor | Windsurf | Codex |
|---------|------------|----------|--------|----------|-------|
| Instructions | `CLAUDE.md` | `AGENTS.md` (falls back to `CLAUDE.md`) | `.cursor/rules/*.mdc`, `AGENTS.md` | `.windsurfrules` | `AGENTS.md`, `codex.md` |
| Memory | `~/.claude/projects/.../memory/` | None (manual) | None (manual) | None | None |
| Hooks | `.claude/settings.json` (PostToolUse, etc.) | `opencode.json` plugins + `tool.execute.after` | None | None | None |
| Commands | `.claude/commands/*.md` | `~/.config/opencode/command/*.md` | N/A | N/A | N/A |
| Global rules | `~/.claude/CLAUDE.md` | `~/.config/opencode/AGENTS.md` | Cursor Settings > Rules | N/A | N/A |

This creates three problems:

1. **Drift** — Instructions written for one tool don't reach others. Teams using mixed tools get inconsistent behavior.
2. **Duplication** — The same context gets copy-pasted into 3-4 different files with tool-specific formatting.
3. **Memory loss** — Hard-won project knowledge (gotchas, conventions, debug findings) stays locked in one tool's proprietary memory format.

## 2. Solution

**aisync** is a standalone Rust CLI that:

1. **Detects** which AI tools are configured for a project
2. **Scaffolds** a canonical `.ai/` directory as the single source of truth
3. **Syncs** instructions, memory, and hooks to each tool's native format
4. **Watches** for changes and propagates them bidirectionally

## 3. Core Concepts

### 3.1 The `.ai/` Directory

The canonical source of truth for all AI agent context, committed to the repo:

```
.ai/
├── aisync.toml          # Configuration
├── instructions.md      # Canonical project instructions (→ CLAUDE.md, AGENTS.md, .mdc, etc.)
├── memory/              # Shared knowledge base
│   ├── index.md         # Overview / table of contents
│   └── *.md             # Topic files
├── hooks/               # Canonical hook definitions (tool-agnostic)
│   └── *.toml           # Hook specs (trigger, command, matcher)
└── commands/            # Slash commands / skills
    └── *.md             # Command definitions
```

### 3.2 Tool Adapters

Each supported tool gets an adapter that knows how to:
- **Read** the tool's native config (detect existing setup)
- **Write** the tool's native format (generate from `.ai/` canonical source)
- **Watch** for changes in tool-specific files (bidirectional sync)
- **Translate** hooks/commands into tool-native equivalents where possible

### 3.3 Sync Strategy

```
.ai/instructions.md  ──→  CLAUDE.md (symlink or copy)
                      ──→  AGENTS.md (symlink or copy)
                      ──→  .cursor/rules/project.mdc (generated)
                      ──→  .windsurfrules (generated)
                      ──→  codex.md (symlink or copy)

.ai/memory/*         ──→  ~/.claude/projects/.../memory/ (symlink)
                      ──→  Referenced in AGENTS.md for OpenCode
                      ──→  Referenced in .cursor/rules/*.mdc for Cursor

.ai/hooks/*.toml     ──→  .claude/settings.json hooks section
                      ──→  opencode.json plugin stubs
                      ──→  (warn-only for tools without hook support)
```

## 4. Supported Tools (Launch)

### Tier 1 — Full support (instructions + memory + hooks)
- **Claude Code** — `CLAUDE.md`, `.claude/hooks/`, `.claude/commands/`, auto-memory symlink
- **OpenCode** — `AGENTS.md`, `opencode.json` hooks/plugins, skills

### Tier 2 — Instructions + memory (no hooks)
- **Cursor** — `.cursor/rules/*.mdc`, `AGENTS.md`, `.cursorrules` (legacy)
- **Windsurf** — `.windsurfrules`
- **Codex** — `AGENTS.md`, `codex.md`

### Tier 3 — Community / plugin-based
- **Aider** — `.aider.conf.yml` conventions
- **Continue** — `.continue/config.json` rules
- **PearAI / Others** — added via adapter plugins

## 5. CLI Interface

### 5.1 `aisync init`

Interactive setup for a new project:

```bash
$ cd my-project
$ aisync init

  Detected tools:
    ✓ Claude Code  (.claude/ found, CLAUDE.md exists)
    ✓ OpenCode     (opencode.json found)
    ✓ Cursor       (.cursor/ found)
    ✗ Windsurf     (not detected)

  Actions:
    → Create .ai/ directory
    → Import CLAUDE.md → .ai/instructions.md
    → Import ~/.claude/projects/.../memory/ → .ai/memory/
    → Generate AGENTS.md (symlink → .ai/instructions.md)
    → Generate .cursor/rules/project.mdc
    → Symlink Claude memory → .ai/memory/
    → Import .claude/settings.json hooks → .ai/hooks/

  Proceed? [Y/n]
```

**Import behavior:**
- If `CLAUDE.md` exists, imports it as `.ai/instructions.md` (or prompts to merge if `.ai/` already exists)
- If `AGENTS.md` exists and differs from `CLAUDE.md`, prompts to merge or pick canonical
- If Claude auto-memory exists, moves files to `.ai/memory/` and creates symlink back
- Detects hooks in any tool and imports to `.ai/hooks/` canonical format

### 5.2 `aisync sync`

One-shot sync from `.ai/` to all detected tools:

```bash
$ aisync sync

  Syncing to 3 tools...
    Claude Code:  ✓ CLAUDE.md (symlink)
                  ✓ memory (symlink verified)
                  ✓ 2 hooks translated
    OpenCode:     ✓ AGENTS.md (symlink)
                  ✓ memory references updated
                  ⚠ 1 hook needs manual plugin (see .ai/hooks/ignition-lint.toml)
    Cursor:       ✓ .cursor/rules/project.mdc (regenerated)
                  ✓ memory file references updated
```

### 5.3 `aisync watch`

Daemon mode — watches for changes and auto-syncs:

```bash
$ aisync watch

  Watching .ai/, CLAUDE.md, AGENTS.md, .cursor/rules/...
  Press Ctrl+C to stop.

  [12:03:15] .ai/instructions.md changed → synced to 3 tools
  [12:05:22] .ai/memory/new-topic.md added → updated AGENTS.md references
  [12:10:01] CLAUDE.md changed (external edit) → reverse-synced to .ai/instructions.md
```

### 5.4 `aisync status`

Show current sync state:

```bash
$ aisync status

  Tool          Instructions  Memory  Hooks  Status
  Claude Code   ✓ symlinked   ✓ 6/6  ✓ 2/2  In sync
  OpenCode      ✓ symlinked   ✓ refs  ⚠ 1/2  1 hook needs manual porting
  Cursor        ✓ generated   ✓ refs  — n/a  In sync
```

### 5.5 `aisync add-tool <tool>`

Add support for a new tool to an existing `.ai/` project:

```bash
$ aisync add-tool windsurf
  → Generated .windsurfrules from .ai/instructions.md
```

### 5.6 `aisync memory <subcommand>`

Manage shared memory:

```bash
$ aisync memory list              # List all memory files
$ aisync memory add <topic>       # Create new memory file
$ aisync memory import claude     # Pull in Claude auto-memory updates
$ aisync memory export            # Write memory to all tools
```

### 5.7 `aisync hooks <subcommand>`

Manage cross-tool hooks:

```bash
$ aisync hooks list                # Show all hooks and their tool translations
$ aisync hooks add <name>          # Create canonical hook definition
$ aisync hooks translate <name>    # Show what each tool's version looks like
```

## 6. Configuration: `aisync.toml`

```toml
[project]
name = "WHK-Global"
description = "Ignition SCADA project for whiskey distillery"

[sync]
strategy = "symlink"     # "symlink" | "copy" | "generate"
watch = true             # Enable file watching in daemon mode
bidirectional = true     # Reverse-sync tool-native changes back to .ai/

[tools.claude]
enabled = true
instructions = "symlink"          # symlink .ai/instructions.md → CLAUDE.md
memory = "symlink"                # symlink .ai/memory/ → ~/.claude/projects/.../memory/
hooks = "translate"               # auto-generate .claude/settings.json hooks

[tools.opencode]
enabled = true
instructions = "symlink"          # symlink .ai/instructions.md → AGENTS.md
memory = "reference"              # add file references in AGENTS.md
hooks = "translate"               # generate opencode.json plugin stubs

[tools.cursor]
enabled = true
instructions = "generate"         # generate .cursor/rules/*.mdc from .ai/
memory = "reference"              # add @file references in .mdc rules
hooks = false                     # cursor doesn't support hooks

[tools.windsurf]
enabled = false

[tools.codex]
enabled = false

# Memory files to exclude from sync
[memory]
exclude = ["scratch.md", "draft-*.md"]

# Hook definitions reference .ai/hooks/*.toml
# See Section 7 for hook spec format
```

## 7. Hook Specification Format

Hooks are defined in `.ai/hooks/` as TOML files with a tool-agnostic schema:

```toml
# .ai/hooks/ignition-lint.toml

[hook]
name = "ignition-lint"
description = "Run ignition-lint on edited Python and JSON files"
trigger = "post-file-edit"        # post-file-edit | post-commit | pre-commit | on-save

[hook.matcher]
globs = ["*.py", "*.json"]
paths = ["/Users/pmannion/data/projects/"]   # only trigger in these dirs

[hook.action]
type = "command"
command = ".ai/hooks/ignition-lint.sh"       # relative to project root
timeout = 60
blocking = false                              # don't block the edit

# Tool-specific overrides (optional)
[hook.overrides.claude]
trigger_event = "PostToolUse"
matcher = "Edit|Write"
stdin_format = "json"                         # Claude passes JSON via stdin

[hook.overrides.opencode]
trigger_event = "tool.execute.after"
type = "plugin"                               # OpenCode uses JS plugins
# aisync generates a thin JS wrapper that calls the shell command
```

## 8. Instructions Translation

When generating tool-specific instruction files from `.ai/instructions.md`:

### Cursor `.mdc` generation
```
---
description: "<first line of instructions.md>"
alwaysApply: true
---

<contents of .ai/instructions.md>

## Project Memory

<auto-generated references to .ai/memory/ files>
```

### Windsurf `.windsurfrules` generation
Plain markdown copy with memory file contents appended (Windsurf doesn't support file references).

### Symlink targets
Claude Code and OpenCode get symlinks since they both read plain markdown natively.

## 9. Bidirectional Sync

When `bidirectional = true`, aisync watches tool-native files for external edits:

1. User edits `CLAUDE.md` directly (e.g., Claude Code adds to it via conversation)
2. aisync detects the change
3. If `CLAUDE.md` is a symlink → change is already in `.ai/instructions.md` (no action)
4. If `CLAUDE.md` is a copy → diff against `.ai/instructions.md`, prompt for merge or overwrite
5. Propagate to other tools

**Claude auto-memory reverse sync:**
When Claude Code writes new memory files to its auto-memory path (which is symlinked to `.ai/memory/`), the files appear directly in `.ai/memory/`. aisync detects the new file and updates references in AGENTS.md and .cursor/rules/*.mdc.

## 10. Non-Goals (v1)

- **MCP server config sync** — tool-specific, complex JSON schemas, out of scope
- **Plugin/extension sync** — each tool has its own plugin ecosystem
- **Auth/credential sharing** — security concern, explicitly out of scope
- **IDE settings sync** — VS Code settings, keybindings, etc. (use Settings Sync)
- **Chat history or session sync** — proprietary formats, ephemeral by nature
- **GSD or other workflow plugin sync** — hardcoded tool-specific paths, managed by plugin authors

## 11. Technical Architecture

### 11.1 Rust Crate Structure

```
aisync/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entrypoint (clap)
│   ├── config.rs            # aisync.toml parsing
│   ├── detect.rs            # Tool detection (scan for config files)
│   ├── sync.rs              # Core sync engine
│   ├── watch.rs             # File watcher (notify crate)
│   ├── memory.rs            # Memory file management
│   ├── hooks.rs             # Hook translation engine
│   ├── adapters/
│   │   ├── mod.rs           # Adapter trait
│   │   ├── claude.rs        # Claude Code adapter
│   │   ├── opencode.rs      # OpenCode adapter
│   │   ├── cursor.rs        # Cursor adapter
│   │   ├── windsurf.rs      # Windsurf adapter
│   │   ├── codex.rs         # Codex adapter
│   │   └── generic.rs       # AGENTS.md-only fallback
│   └── templates/           # Handlebars/minijinja templates
│       ├── cursor_rule.mdc
│       ├── opencode_plugin.js
│       └── hook_wrapper.sh
└── tests/
    ├── fixtures/            # Sample project layouts
    └── integration/         # End-to-end sync tests
```

### 11.2 Adapter Trait

```rust
pub trait ToolAdapter {
    /// Unique identifier for this tool
    fn name(&self) -> &str;

    /// Detect if this tool is configured for the given project
    fn detect(&self, project_root: &Path) -> DetectionResult;

    /// Read existing instructions from the tool's native format
    fn read_instructions(&self, project_root: &Path) -> Result<String>;

    /// Write instructions in the tool's native format
    fn write_instructions(&self, project_root: &Path, content: &str, strategy: SyncStrategy) -> Result<()>;

    /// Sync memory file references
    fn sync_memory(&self, project_root: &Path, memory_files: &[PathBuf], strategy: MemoryStrategy) -> Result<()>;

    /// Translate a canonical hook spec into tool-native config
    fn translate_hook(&self, hook: &HookSpec) -> Result<Option<TranslatedHook>>;

    /// Watch paths for this tool's native files
    fn watch_paths(&self, project_root: &Path) -> Vec<PathBuf>;
}
```

### 11.3 Key Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` | CLI argument parsing |
| `notify` | Cross-platform file watching |
| `toml` | Config parsing |
| `serde` | Serialization |
| `minijinja` | Template rendering for .mdc, .js, etc. |
| `similar` | Diff/merge for bidirectional sync |
| `dialoguer` | Interactive prompts |
| `indicatif` | Progress bars and status output |
| `dirs` | Platform-specific config paths (~/.claude, ~/.config/opencode) |

### 11.4 Platform Support

- macOS (primary — Darwin/arm64)
- Linux (x86_64, arm64)
- Windows (best-effort — symlinks require developer mode)

## 12. Distribution

- **Homebrew tap**: `brew install aisync`
- **Cargo**: `cargo install aisync`
- **GitHub Releases**: Pre-built binaries for macOS/Linux/Windows
- **Shell installer**: `curl -fsSL https://aisync.dev/install.sh | sh`

## 13. Future Work (v2+)

- **`aisync migrate`** — migrate from one tool to another (e.g., Cursor → Claude Code)
- **Team sync** — shared `.ai/` conventions across monorepo packages
- **Memory dedup** — detect and merge overlapping memory entries
- **Hook marketplace** — community-contributed hook definitions
- **Plugin SDK** — third-party tool adapters as Rust plugins or WASM
- **`aisync diff`** — show what each tool currently sees vs canonical `.ai/`
- **Git hooks** — auto-run `aisync sync` on commit/checkout

## 14. Open Questions

1. **Should `.ai/` be gitignored or committed?** Memory files may contain project-specific secrets or debugging notes. Recommend: committed by default with `.ai/memory/.gitignore` for sensitive files.

2. **Symlink vs copy tradeoff** — Symlinks are elegant but break on Windows without developer mode and can confuse some tools. Should we default to copy on Windows?

3. **How to handle tool-specific instruction sections?** Some content only makes sense for one tool (e.g., "use the Edit tool instead of sed" is Claude-specific). Options: conditional sections with `<!-- aisync:claude-only -->` markers, or separate override files per tool.

4. **Should aisync manage `.gitignore` entries?** Adding `.cursor/`, `.claude/settings.local.json`, etc. to gitignore is common. Should aisync handle this?

5. **How to handle the Claude auto-memory write path?** Claude Code expects to write to `~/.claude/projects/.../memory/`. The symlink approach works but the path includes a mangled project directory. Should aisync auto-detect this path?

