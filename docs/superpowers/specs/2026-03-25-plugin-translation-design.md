# Plugin Translation: Import & Export Across Tools

**Date:** 2026-03-25
**Status:** Draft

## Overview

Adds plugin translation to aisync — import a plugin written for one tool into a canonical format, then export it to any supported tool. Composes existing engines (hooks, MCP, rules, etc.) rather than duplicating translation logic.

---

## Canonical Plugin Format

A canonical plugin lives at `.ai/plugins/<name>/` and mirrors the top-level `.ai/` structure:

```
.ai/plugins/<name>/
├── plugin.toml              # Plugin metadata + component manifest
├── instructions.md          # Plugin-specific instructions (optional)
├── hooks.toml               # Plugin hooks (optional)
├── mcp.toml                 # Plugin MCP servers (optional)
├── rules/                   # Plugin rules (optional)
│   └── *.md
├── commands/                # Plugin commands (optional)
│   └── *.md
├── skills/                  # Plugin skills (optional)
│   └── */SKILL.md
└── agents/                  # Plugin agents (optional)
    └── *.md
```

Each component file uses the exact same format as its top-level `.ai/` equivalent. No new schemas.

### plugin.toml

```toml
[metadata]
name = "aisync"
version = "0.1.0"
description = "Sync AI tool configs across harnesses"
source_tool = "claude-code"   # which tool this was imported from (if any)

[components]
has_instructions = true
has_hooks = true
has_mcp = false
has_rules = false
has_commands = true
has_skills = true
has_agents = false
```

The `[components]` section is auto-generated during import/export, not manually edited.

---

## PluginTranslator Orchestrator

New module: `crates/aisync-core/src/plugin_translator.rs`

Coordinates import and export by delegating to existing engines. Contains no translation logic of its own.

### Types

```rust
pub struct PluginTranslator;

impl PluginTranslator {
    /// Import a tool-native plugin into canonical format.
    /// Auto-detects source tool if `tool` is None.
    pub fn import(
        source_path: &Path,
        tool: Option<ToolKind>,
        output_root: &Path,
    ) -> Result<ImportReport, AisyncError>;

    /// Export a canonical plugin to one or more target tools.
    pub fn export(
        plugin_path: &Path,
        targets: &[ToolKind],
        project_root: &Path,
    ) -> Result<Vec<ExportReport>, AisyncError>;
}

pub struct ImportReport {
    pub name: String,
    pub source_tool: ToolKind,
    pub components_imported: Vec<ComponentKind>,
    pub components_skipped: Vec<(ComponentKind, String)>,  // (what, why)
}

pub struct ExportReport {
    pub tool: ToolKind,
    pub components_exported: Vec<(ComponentKind, Vec<PathBuf>)>,
    pub components_skipped: Vec<(ComponentKind, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComponentKind {
    Instructions,
    Hooks,
    Mcp,
    Rules,
    Commands,
    Skills,
    Agents,
}
```

### Auto-Detection

When `--from` is omitted, detect the source tool by marker files:

| Marker | Tool |
|--------|------|
| `.claude-plugin/plugin.json` | Claude Code |
| `.cursor/` directory | Cursor |
| `opencode.json` or `.opencode/` | OpenCode |

---

## Import Flow

### Claude Code → Canonical (full fidelity)

1. Read `.claude-plugin/plugin.json` → extract name, version, description → write `plugin.toml` `[metadata]`
2. `commands/*.md` → copy to `.ai/plugins/<name>/commands/` (1:1, same format)
3. `skills/*/SKILL.md` → copy to `.ai/plugins/<name>/skills/` (1:1, same format)
4. `agents/*.md` → copy to `.ai/plugins/<name>/agents/` (1:1, same format)
5. `hooks/hooks.json` → parse using existing hook JSON parsing → serialize as `hooks.toml`
6. `.mcp.json` → parse using `McpEngine::parse_mcp_json()` → serialize as `mcp.toml`
7. Generate `[components]` flags in `plugin.toml`

### Cursor → Canonical (hooks, MCP, rules)

1. `rules/*.mdc` → strip `.mdc` frontmatter, write as `rules/*.md` with YAML frontmatter
2. `hooks.json` → parse Cursor hook format, reverse-translate event names (camelCase → PascalCase) and matchers (`Write` → `Edit`, `Shell` → `Bash`) → write as `hooks.toml`
3. `mcp.json` → parse with `McpEngine::parse_mcp_json()`, reverse env var format (`${env:VAR}` → `${VAR}`) → write as `mcp.toml`
4. No commands, skills, or agents (Cursor doesn't have them) → skipped in report

### OpenCode → Canonical (hooks, instructions)

1. `AGENTS.md` → extract as `instructions.md`
2. Plugin stub `.js` → reverse-parse hook definitions → write as `hooks.toml`
3. No commands, skills, agents, MCP, or rules → skipped in report

### Import Report

```
Imported plugin 'aisync' from claude-code → .ai/plugins/aisync/
  ✓ instructions
  ✓ commands (7 files)
  ✓ skills (2 directories)
  ✓ hooks (4 hooks)
  ✗ mcp (none found)
  ✗ agents (none found)
```

---

## Export Flow

### Canonical → Claude Code (full fidelity)

1. `plugin.toml` metadata → `.claude-plugin/plugin.json`
2. `commands/*.md` → `commands/*.md` (1:1)
3. `skills/*/SKILL.md` → `skills/*/SKILL.md` (1:1)
4. `agents/*.md` → `agents/*.md` (1:1)
5. `hooks.toml` → `hooks/hooks.json` (via `ClaudeCodeAdapter::translate_hooks()`)
6. `mcp.toml` → `.mcp.json` (via existing MCP translation)

### Canonical → Cursor (hooks, MCP, rules, instructions)

1. `instructions.md` → `.cursor/rules/<plugin-name>.mdc` with frontmatter
2. `rules/*.md` → `.cursor/rules/<plugin-name>-*.mdc` with frontmatter
3. `hooks.toml` → hook entries in `.cursor/hooks.json` (via `CursorAdapter::translate_hooks()`)
4. `mcp.toml` → entries in `.cursor/mcp.json` (via existing MCP translation)
5. Commands, skills, agents → **skipped** with report

### Canonical → OpenCode (hooks, instructions)

1. `instructions.md` → section in `AGENTS.md`
2. `hooks.toml` → `.opencode/plugins/<name>.js` stub (via `OpenCodeAdapter::translate_hooks()`)
3. Commands, skills, agents, MCP, rules → **skipped** with report

### Export Report

```
Exported plugin 'aisync' → claude-code
  ✓ commands (7 files)
  ✓ skills (2 directories)
  ✓ hooks (4 hooks)
  ✗ agents (none in plugin)

Exported plugin 'aisync' → cursor
  ✓ instructions → .cursor/rules/aisync.mdc
  ✓ hooks (3 of 4 translated)
  ✗ commands (7 skipped — no cursor equivalent)
  ✗ skills (2 skipped — no cursor equivalent)
```

---

## CLI Commands

Three new subcommands under `aisync plugin`:

### `aisync plugin import <path> [--from <tool>] [--name <name>]`

- `<path>` — path to tool-native plugin directory
- `--from` — source tool (auto-detect if omitted)
- `--name` — override plugin name (default: derived from manifest)
- Writes to `.ai/plugins/<name>/`
- Prints import report

### `aisync plugin export <name> [--to <tool>] [--all]`

- `<name>` — canonical plugin name (directory under `.ai/plugins/`)
- `--to` — specific target tool
- `--all` — export to all configured tools (default behavior if no flags)
- Writes to each tool's expected plugin location
- Prints export report per tool

### `aisync plugin list`

- Lists all canonical plugins in `.ai/plugins/`
- Shows component summary per plugin (which components exist)

---

## Sync Integration

`aisync sync` automatically exports all canonical plugins:

1. During plan phase, scan `.ai/plugins/` for directories containing `plugin.toml`
2. For each plugin × each configured tool, call `PluginTranslator::export()`
3. Include generated files in the `SyncReport`
4. Generated output paths added to `.gitignore` tracking

`aisync status` — shows plugin export state per tool (synced/drifted/not-exported).

`aisync check` — exits non-zero if any plugin exports have drifted.

`aisync diff` — shows differences between canonical plugins and their exported tool-native versions.

---

## Component Translation Matrix

| Component | Claude Code | Cursor | OpenCode | Codex | Windsurf |
|-----------|:-----------:|:------:|:--------:|:-----:|:--------:|
| Instructions | ✓ import/export | ✓ import/export | ✓ import/export | — | — |
| Hooks | ✓ import/export | ✓ import/export | ✓ import/export | — | — |
| MCP | ✓ import/export | ✓ import/export | — | — | — |
| Rules | — | ✓ import/export | — | — | — |
| Commands | ✓ import/export | ✗ no equivalent | ✗ no equivalent | — | — |
| Skills | ✓ import/export | ✗ no equivalent | ✗ no equivalent | — | — |
| Agents | ✓ import/export | ✗ no equivalent | ✗ no equivalent | — | — |

"—" = placeholder/future work (adapter not yet implemented).

---

## Out of Scope

- **Lossy approximations** — No converting skills into rules or commands into instructions. Skipped with report.
- **Windsurf/Codex adapters** — Placeholders only, matching current state.
- **Plugin dependency management** — No inter-plugin dependencies.
- **Plugin versioning/update detection** — No checking if a canonical plugin is newer than its export.
- **Remote plugin fetching** — Handled by the separate `.ai/plugins.toml` reference system (already built).

## Testing Strategy

- **Import tests:** For each tool, provide a fixture plugin in tool-native format, import it, verify canonical output matches expectations. Test auto-detection of source tool.
- **Export tests:** Create canonical plugins with various component combinations, export to each tool, verify output format and structure.
- **Round-trip tests:** Import a Claude Code plugin → export back to Claude Code → compare with original.
- **Skip reporting:** Verify that components without tool equivalents appear in the skip report.
- **Sync integration:** Test that `aisync sync` picks up canonical plugins and includes them in the plan.
- **CLI tests:** Integration tests for `plugin import`, `plugin export`, `plugin list` subcommands.
