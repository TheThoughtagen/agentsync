---
name: hook-authoring
description: Use when creating or editing .ai/hooks.toml hook definitions. Teaches the TOML format, valid events, matchers, and cross-tool translation.
version: 1.0.0
---

# Hook Authoring Guide

## File Format

Hooks are defined in `.ai/hooks.toml` using TOML. Each hook is grouped under an `[[EventName]]` section with an optional `matcher` field, and contains one or more `[[EventName.hooks]]` entries.

```toml
[[PreToolUse]]
matcher = "Bash"

[[PreToolUse.hooks]]
type = "command"
command = "bash .ai/scripts/validate-bash.sh"
timeout = 10
```

## Valid Events

### Shared Events (work in both Claude Code and Cursor via aisync translation)

| Event | Description |
|-------|-------------|
| `PreToolUse` | Fires before a tool is invoked. Use `matcher` to target specific tools. |
| `PostToolUse` | Fires after a tool completes. Use `matcher` to target specific tools. |
| `Stop` | Fires when the agent stops (completes a response). |
| `SubagentStop` | Fires when a sub-agent stops. |

### Claude Code Only (via aisync translation)

| Event | Description |
|-------|-------------|
| `Notification` | Fires when a notification is sent. |

### Cursor Only (via aisync translation)

| Event | Description |
|-------|-------------|
| `PostToolUseFailure` | Fires when a tool invocation fails. |
| `SessionStart` | Fires when a new session begins. |
| `SessionEnd` | Fires when a session ends. |
| `BeforeShellExecution` | Fires before a shell command runs. |
| `AfterShellExecution` | Fires after a shell command completes. |
| `BeforeReadFile` | Fires before reading a file. |
| `AfterFileEdit` | Fires after a file is edited. |
| `BeforeSubmitPrompt` | Fires before a prompt is submitted. |
| `PreCompact` | Fires before context compaction. |
| `AfterAgentResponse` | Fires after the agent responds. |
| `AfterAgentThought` | Fires after an agent thought step. |
| `SubagentStart` | Fires when a sub-agent starts. |
| `BeforeMCPExecution` | Fires before an MCP tool call. |
| `AfterMCPExecution` | Fires after an MCP tool call completes. |

> **Note:** Claude Code natively supports additional events (`SessionStart`, `SessionEnd`, `UserPromptSubmit`, `PreCompact`) that aisync does not yet translate. When writing hooks for these events in `.ai/hooks.toml`, aisync will currently skip them during Claude Code translation. To use these events, configure them directly in `.claude/settings.json` or via a Claude Code plugin's `hooks.json`.

## Matcher Syntax

The `matcher` field filters which tools trigger the hook:

| Pattern | Matches |
|---------|---------|
| `Edit` | Exact tool name match. |
| `Edit\|Write` | Multiple tools (pipe-separated). |
| `mcp__.*` | Regex pattern — matches all MCP tools. |

Omit `matcher` to match all tools for that event.

## Hook Types

### `command` — Shell command hook
Runs a bash command. Available for all events and all tools.

```toml
[[PostToolUse.hooks]]
type = "command"
command = "echo 'Tool finished'"
timeout = 30
```

### `prompt` — LLM-driven hook
Sends a prompt to the LLM for evaluation. **Claude Code only**, and only for these events via aisync: `PreToolUse`, `Stop`, `SubagentStop`. Note that `UserPromptSubmit` prompt hooks work in native Claude Code `hooks.json` but not through aisync's translation layer.

```toml
[[PreToolUse]]
matcher = "Bash"

[[PreToolUse.hooks]]
type = "prompt"
command = "Review this bash command for safety. Reject if it modifies system files."
```

## Cross-Tool Translation

When aisync syncs hooks to different tools, it translates the canonical format:

- **Cursor** uses camelCase event names (e.g., `PreToolUse` becomes `preToolUse`)
- **Tool name mapping**: Cursor uses different names for some tools:
  - `Edit` (Claude Code) maps to `Write` (Cursor)
  - `Bash` (Claude Code) maps to `Shell` (Cursor)

Use `aisync hooks list` to see all hooks and their per-tool support status.

Use `aisync hooks translate` to preview the exact output each tool will receive after translation.

## Complete Example

```toml
# Validate bash commands before execution
[[PreToolUse]]
matcher = "Bash"

[[PreToolUse.hooks]]
type = "command"
command = "bash .ai/scripts/validate-bash.sh"
timeout = 10

# Log all tool usage
[[PostToolUse]]

[[PostToolUse.hooks]]
type = "command"
command = "bash .ai/scripts/log-tool-use.sh"
timeout = 5

# Review agent output before stopping
[[Stop]]

[[Stop.hooks]]
type = "prompt"
command = "Review the agent's final response for completeness and accuracy."
```
