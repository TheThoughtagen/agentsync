# AgentSync Claude Code Plugin & Plugin Sync Infrastructure

**Date:** 2026-03-25
**Status:** Draft

## Overview

Two co-designed deliverables:

1. **Plugin sync infrastructure** — Adds plugin reference syncing to `aisync-core`, following the same canonical-to-native pattern as hooks, MCP, rules, etc.
2. **`aisync` Claude Code plugin** — A marketplace-compatible plugin providing commands, skills, and hooks so Claude Code sessions know how to use AgentSync.

The repo itself serves as a Claude Code plugin marketplace via the standard `plugins/<name>/` layout.

---

## Part 1: Plugin Sync Infrastructure (Rust)

### Canonical Format

New file: `.ai/plugins.toml`

```toml
[plugins.aisync]
source = "github:whiskeyhouse/agentsync"
description = "Sync AI tool configs across harnesses"

[plugins.some-linter]
source = "npm:@company/linter-plugin"

[plugins.local-thing]
source = "path:./tools/my-plugin"
```

**Source formats:**
- `github:<owner>/<repo>` — Git repo as marketplace
- `npm:<package>` — npm-published plugin
- `path:<relative-path>` — Local directory

### New Engine: `PluginEngine`

Location: `crates/aisync-core/src/plugins.rs`

Responsibilities:
- `load()` — Parse `.ai/plugins.toml` into `PluginsConfig` (BTreeMap of plugin name to `PluginRef`)

Note: `PluginEngine` only loads data. Translation is handled by each adapter's `plan_plugins_sync()`, matching the pattern used by `McpEngine`, `HookEngine`, etc.

Types:
```rust
pub struct PluginRef {
    pub source: PluginSource,
    pub description: Option<String>,
}

pub enum PluginSource {
    GitHub { owner: String, repo: String },
    Npm { package: String },
    Path { path: PathBuf },
}

pub type PluginsConfig = BTreeMap<String, PluginRef>;
```

### Adapter Trait Extension

Add to `ToolAdapter` trait:
```rust
fn plan_plugins_sync(&self, project_root: &Path, config: &PluginsConfig) -> Vec<SyncAction>;
```

**Claude Code adapter** — Writes plugin refs into `.claude/settings.json`:
```json
{
  "plugins": {
    "marketplaces": ["github:whiskeyhouse/agentsync"],
    "installed": {
      "aisync": { "enabled": true }
    }
  }
}
```
*(Exact schema to be verified against Claude Code's actual settings format during implementation.)*

**Cursor adapter** — Translates to Cursor's extension/plugin config format where possible. Returns `PluginTranslation::Unsupported { reason }` for source types Cursor can't represent.

**OpenCode adapter** — Translates to `opencode.json` plugin references where possible. Same unsupported pattern.

**New `SyncAction` variants** — Plugin sync requires merging into existing JSON files (e.g., `.claude/settings.json` may already have other settings). Add `MergeJsonFile { path, key, value }` variant to `SyncAction` for partial JSON updates, or reuse the existing `CreateFile` approach with a read-merge-write strategy in the execution phase. Decision deferred to implementation.

### Integration with Existing Commands

- `aisync sync` — Includes plugin reference syncing in the plan/execute cycle
- `aisync status` — Reports plugin sync state per tool (synced/drifted/unsupported)
- `aisync check` — Exits non-zero if plugin refs have drifted
- `aisync init` — Detects existing plugin references in tool configs and imports to `.ai/plugins.toml`
- `aisync diff` — Shows plugin ref differences between canonical and tool-native

---

## Part 2: `aisync` Claude Code Plugin

### Directory Layout

```
plugins/
└── aisync/
    ├── .claude-plugin/
    │   └── plugin.json
    ├── commands/
    │   ├── sync.md
    │   ├── status.md
    │   ├── init.md
    │   ├── diff.md
    │   ├── check.md
    │   ├── hooks.md
    │   └── memory.md
    ├── skills/
    │   ├── ai-config/
    │   │   └── SKILL.md
    │   └── hook-authoring/
    │       └── SKILL.md
    └── hooks/
        ├── hooks.json
        └── scripts/
            ├── drift-check.sh
            ├── direct-edit-warn.sh
            └── hooks-validate.sh
```

### Plugin Manifest

`plugins/aisync/.claude-plugin/plugin.json`:
```json
{
  "name": "aisync",
  "version": "0.1.0",
  "description": "AgentSync integration for Claude Code — sync AI tool configs across harnesses",
  "author": {
    "name": "Whiskeyhouse"
  },
  "repository": "https://github.com/whiskeyhouse/agentsync",
  "license": "MIT",
  "keywords": ["sync", "config", "cursor", "opencode", "hooks", "mcp"]
}
```

### Commands

Each command is a markdown file with YAML frontmatter that wraps the corresponding `aisync` CLI subcommand.

**`commands/sync.md`** — `/aisync:sync`
```yaml
---
name: sync
description: Sync .ai/ canonical config to all configured AI tools
---
```
Runs `aisync sync`, reports which files were created/updated/symlinked per tool. Supports `--dry-run` flag.

**`commands/status.md`** — `/aisync:status`
```yaml
---
name: status
description: Show per-tool sync status and drift detection
---
```
Runs `aisync status --json`, presents a readable summary of which tools are synced, drifted, or not configured.

**`commands/init.md`** — `/aisync:init`
```yaml
---
name: init
description: Initialize .ai/ directory with tool detection and config import
---
```
Runs `aisync init`, reports detected tools and imported configs.

**`commands/diff.md`** — `/aisync:diff`
```yaml
---
name: diff
description: Compare canonical .ai/ content vs tool-native files
---
```
Runs `aisync diff`, shows differences between canonical and tool-native files.

**`commands/check.md`** — `/aisync:check`
```yaml
---
name: check
description: Check sync state (CI-friendly, exits non-zero on drift)
---
```
Runs `aisync check`, reports pass/fail.

**`commands/hooks.md`** — `/aisync:hooks`
```yaml
---
name: hooks
description: Manage hook definitions — list, add, or translate hooks across tools
---
```
Wraps `aisync hooks list|add|translate` subcommands.

**`commands/memory.md`** — `/aisync:memory`
```yaml
---
name: memory
description: Manage memory files — list, add, import, or export across tools
---
```
Wraps `aisync memory list|add|import|export` subcommands.

### Skills

**`skills/ai-config/SKILL.md`**
```yaml
---
name: ai-config
description: Use when working with .ai/ directory files, aisync.toml, or AI tool configuration. Teaches the canonical format, conditional tags, and sync relationships.
version: 1.0.0
---
```

Content teaches Claude:
- The `.ai/` directory is the canonical source of truth
- File purposes: `instructions.md`, `hooks.toml`, `mcp.toml`, `plugins.toml`, `aisync.toml`
- Subdirectories: `memory/`, `rules/`, `commands/`, `skills/`, `agents/`
- Conditional tags: `<!-- aisync:claude-only -->...<!-- /aisync:claude-only -->`
- Always edit canonical files, not synced copies (`CLAUDE.md`, `.cursor/rules/project.mdc`, `AGENTS.md`)
- Run `aisync sync` after making changes to propagate them
- The relationship between canonical format and tool-native formats

**`skills/hook-authoring/SKILL.md`**
```yaml
---
name: hook-authoring
description: Use when creating or editing .ai/hooks.toml hook definitions. Teaches the TOML format, valid events, matchers, and cross-tool translation.
version: 1.0.0
---
```

Content teaches Claude:
- The `.ai/hooks.toml` format: `[[EventName]]` sections with matchers and hook definitions
- Valid events: `PreToolUse`, `PostToolUse`, `Stop`, `SubagentStop`, `Notification`, `PostToolUseFailure`, `SessionStart`, `SessionEnd`, `BeforeShellExecution`, `AfterShellExecution`, `BeforeReadFile`, `AfterFileEdit`, `BeforeSubmitPrompt`, `PreCompact`, `AfterAgentResponse`, `AfterAgentThought`, `SubagentStart`, `BeforeMCPExecution`, `AfterMCPExecution`
- Which events are shared vs tool-specific
- Hook types: `command` with `type`, `command`, `timeout` fields
- Matcher syntax for tool names
- How `aisync hooks translate` previews per-tool output
- Tool-specific translation: Claude Code → `settings.json`, Cursor → `hooks.json` with event name/matcher translation, OpenCode → plugin stub

### Hooks

**Important note on event names:** The plugin's `hooks.json` is consumed directly by Claude Code's native hook system, *not* by aisync's hook translator. Claude Code natively supports `SessionStart`, `UserPromptSubmit`, `PostToolUse`, etc. These events are only classified as "Cursor-only" in aisync's `VALID_EVENTS` / `CLAUDE_CODE_EVENTS` for the purposes of aisync's cross-tool *translation* layer. A separate task should update aisync's event classification to reflect that Claude Code now supports these events natively.

**`hooks/hooks.json`** (plugin wrapper format):
```json
{
  "description": "AgentSync drift detection, edit validation, and config validation hooks",
  "hooks": {
    "SessionStart": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "bash ${CLAUDE_PLUGIN_ROOT}/hooks/scripts/drift-check.sh",
            "timeout": 15
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "bash ${CLAUDE_PLUGIN_ROOT}/hooks/scripts/direct-edit-warn.sh",
            "timeout": 10
          },
          {
            "type": "command",
            "command": "bash ${CLAUDE_PLUGIN_ROOT}/hooks/scripts/hooks-validate.sh",
            "timeout": 10
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "prompt",
            "prompt": "The user is about to submit a prompt. If their prompt involves committing code (mentions 'commit', 'git commit', '/commit', creating a commit, etc.), first run `aisync check` to verify sync state. If .ai/ changes exist that haven't been synced to tool-native files, warn the user: 'AgentSync detected unsynced changes in .ai/ — run /aisync:sync first to propagate changes before committing.' If the prompt is not about committing, approve silently.",
            "timeout": 15
          }
        ]
      }
    ]
  }
}
```

### Hook Scripts

**`hooks/scripts/drift-check.sh`** — SessionStart drift detection:
```bash
#!/bin/bash
set -euo pipefail

# Check if aisync is available
if ! command -v aisync &>/dev/null; then
  echo "aisync CLI not found — skipping drift check"
  exit 0
fi

# Check if this is an aisync-managed project
if [ ! -f "$CLAUDE_PROJECT_DIR/aisync.toml" ]; then
  exit 0
fi

# Run drift check
check_status=0
output=$(aisync check --json 2>/dev/null) || check_status=$?

if [ $check_status -ne 0 ]; then
  drifted=$(echo "$output" | jq -r '.drifted // [] | .[] // empty' 2>/dev/null || true)
  if [ -n "$drifted" ]; then
    echo "AgentSync: Config drift detected. Run /aisync:sync to fix."
    echo "Drifted tools: $drifted"
  fi
fi

exit 0
```

**`hooks/scripts/direct-edit-warn.sh`** — PostToolUse edit warning (exit 2 feeds `systemMessage` back to Claude as context, not a hard block):
```bash
#!/bin/bash
set -euo pipefail

# Require jq for JSON parsing
if ! command -v jq &>/dev/null; then
  exit 0
fi

input=$(cat)
tool_name=$(echo "$input" | jq -r '.tool_name // empty')
file_path=$(echo "$input" | jq -r '.tool_input.file_path // empty')

# Only care about Edit/Write operations
if [[ "$tool_name" != "Edit" && "$tool_name" != "Write" ]]; then
  exit 0
fi

# Check if this is an aisync-managed project
if [ ! -f "$CLAUDE_PROJECT_DIR/aisync.toml" ]; then
  exit 0
fi

# Synced target files that shouldn't be edited directly
synced_targets=(
  "CLAUDE.md"
  "AGENTS.md"
  ".cursor/rules/project.mdc"
  ".cursor/hooks.json"
  ".cursor/mcp.json"
  ".opencode/plugins/aisync-hooks.js"
)

# Get relative path from project root
rel_path="${file_path#"$CLAUDE_PROJECT_DIR"/}"

for target in "${synced_targets[@]}"; do
  if [[ "$rel_path" == "$target" ]]; then
    echo "{\"systemMessage\": \"WARNING: '$rel_path' is managed by AgentSync and will be overwritten on next sync. Edit the canonical source in .ai/ instead (e.g., .ai/instructions.md for CLAUDE.md/AGENTS.md).\"}" >&2
    exit 2
  fi
done

exit 0
```

**`hooks/scripts/hooks-validate.sh`** — PostToolUse hooks.toml validation:
```bash
#!/bin/bash
set -euo pipefail

# Require jq for JSON parsing
if ! command -v jq &>/dev/null; then
  exit 0
fi

input=$(cat)
file_path=$(echo "$input" | jq -r '.tool_input.file_path // empty')

# Only trigger for hooks.toml edits
rel_path="${file_path#"$CLAUDE_PROJECT_DIR"/}"
if [[ "$rel_path" != ".ai/hooks.toml" ]]; then
  exit 0
fi

# Check if aisync is available
if ! command -v aisync &>/dev/null; then
  exit 0
fi

# Validate by attempting translation
output=$(aisync hooks translate 2>&1) || {
  echo "{\"systemMessage\": \"hooks.toml validation failed: $output\"}" >&2
  exit 2
}

echo "hooks.toml validated successfully"
exit 0
```

---

## Out of Scope

- **MCP server or agents** — Not enough value to justify the overhead
- **Cursor/OpenCode/Windsurf plugin equivalents** — Future work; the plugin sync infrastructure supports them when ready
- **npm publishing** — Can be added later; repo-as-marketplace is the initial distribution method
- **Bidirectional plugin sync** — Import from tool-native plugin configs is init-time only, not continuous

## Dependencies

- `aisync` CLI must be installed and on PATH for hooks and commands to function
- `jq` used by hook scripts for JSON parsing (scripts gracefully no-op if `jq` is missing)
- Project must have `aisync.toml` for hooks to activate (graceful no-op otherwise)

## Testing Strategy

- **Plugin structure**: Validate against Claude Code plugin spec (manifest, auto-discovery, naming)
- **Hook scripts**: Unit test each script with mock JSON input via stdin
- **Commands**: Integration test that each command produces expected CLI output
- **Plugin sync engine**: Unit tests for TOML parsing, adapter translation, round-trip sync
- **Adapter translations**: Test each adapter's `plan_plugins_sync()` produces correct native format
