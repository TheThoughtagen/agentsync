---
name: ai-config
description: Use when working with .ai/ directory files, aisync.toml, or AI tool configuration. Teaches the canonical format, conditional tags, and sync relationships.
version: 1.0.0
---

# AI Configuration with AgentSync

## Canonical Source of Truth

The `.ai/` directory is the single source of truth for all AI tool configurations. Never edit synced copies directly — always edit files in `.ai/` and then sync.

## File Purposes

| File | Purpose |
|------|---------|
| `.ai/instructions.md` | Main project instructions. Synced to `CLAUDE.md`, `AGENTS.md`, and `.cursor/rules/project.mdc`. |
| `.ai/hooks.toml` | Event hook definitions (pre/post tool use, shell execution, etc.). |
| `.ai/mcp.toml` | MCP (Model Context Protocol) server definitions. |
| `.ai/plugins.toml` | Plugin references and configuration. |
| `aisync.toml` | AgentSync configuration — controls sync strategy per tool (symlink, copy, generate). Lives at project root. |

## Subdirectories

| Directory | Purpose |
|-----------|---------|
| `.ai/memory/` | Context files providing persistent knowledge across sessions. |
| `.ai/rules/` | Coding rules with YAML frontmatter (style, conventions, patterns). |
| `.ai/commands/` | Slash command definitions for AI tools. |
| `.ai/skills/` | Skill modules that teach AI tools domain-specific knowledge. |
| `.ai/agents/` | Agent definitions for multi-agent workflows. |

## Conditional Tags

Use conditional tags to include content only for specific tools:

```markdown
<!-- aisync:claude-only -->
This content only appears in Claude Code output.
<!-- /aisync:claude-only -->

<!-- aisync:cursor-only -->
This content only appears in Cursor output.
<!-- /aisync:cursor-only -->

<!-- aisync:opencode-only -->
This content only appears in OpenCode output.
<!-- /aisync:opencode-only -->
```

Content outside conditional tags is included in all tool outputs.

## Critical Rules

1. **Always edit canonical `.ai/` files.** Never edit synced copies directly. The following are generated outputs and will be overwritten on sync:
   - `CLAUDE.md`
   - `.cursor/rules/project.mdc`
   - `AGENTS.md`

2. **Run `aisync sync` (or `/aisync:sync`) after making changes** to propagate updates to all configured tools.

3. **Check sync state** with `aisync status` (or `/aisync:status`) to see if tool-native files have drifted from canonical content.

## Sync Strategies

The `aisync.toml` file controls how each tool receives its configuration:

- **symlink** — Tool-native file is a symlink to the canonical file (fast, always in sync).
- **copy** — Canonical content is copied to the tool-native location (for tools that don't support symlinks).
- **generate** — Content is transformed/generated from canonical format to tool-native format (e.g., adding frontmatter for `.mdc` files).
