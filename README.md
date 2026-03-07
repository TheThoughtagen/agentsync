# aisync

[![CI](https://github.com/pmannion/agentsync/actions/workflows/ci.yml/badge.svg)](https://github.com/pmannion/agentsync/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/aisync.svg)](https://crates.io/crates/aisync)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Sync AI tool configurations across Claude Code, Cursor, and OpenCode.

Every AI tool working on your project sees the same instructions, memory, and hooks -- always in sync, zero manual copying.

## Why?

Modern projects use multiple AI coding tools simultaneously. Each tool has its own config format and location. Keeping instructions consistent across tools is tedious and error-prone. **aisync** maintains a single canonical config (`.ai/`) and syncs it everywhere.

## Supported Tools

| Tool | Config Location | Status |
|------|----------------|--------|
| Claude Code | `CLAUDE.md`, `.claude/` | Supported |
| Cursor | `.cursor/rules/` | Supported |
| OpenCode | `AGENTS.md`, `.opencode/` | Supported |

## Installation

### From crates.io

```sh
cargo install aisync
```

### From source

```sh
git clone https://github.com/pmannion/agentsync.git
cd agentsync
cargo install --path crates/aisync
```

## Quick Start

```sh
# Initialize -- detects tools and creates .ai/ canonical config
aisync init

# Sync canonical config to all detected tools
aisync sync

# Check what's in sync and what's drifted
aisync status

# Watch for changes and auto-sync bidirectionally
aisync watch
```

## Configuration

aisync is configured via `aisync.toml` in your project root:

```toml
schema_version = 1

[defaults]
sync_strategy = "symlink"  # or "copy"

[tools.claude-code]
enabled = true

[tools.cursor]
enabled = true
sync_strategy = "copy"  # override per tool

[tools.opencode]
enabled = true
```

### Sync Strategies

- **symlink** (default): Creates symlinks from tool config locations to `.ai/`. Changes in any location are instantly reflected everywhere.
- **copy**: Copies files from `.ai/` to tool config locations. Use when symlinks aren't supported (e.g., some Windows setups).

### Managed Sections

When syncing to files that contain tool-specific content (like `.gitignore`), aisync uses managed sections to insert its content without overwriting existing entries:

```
# existing content stays untouched

# aisync-managed
.cursor/rules/project.mdc
# /aisync-managed
```

## Features

- **Canonical config**: Single `.ai/` directory is the source of truth
- **Bidirectional watch**: Changes in any tool's config sync back to canonical and out to all others
- **Drift detection**: `aisync status` shows which tools are out of sync and why
- **Managed sections**: Non-destructive syncing that preserves tool-specific content
- **Conditional content**: Include tool-specific blocks in shared instructions
- **Hook sync**: Translate hooks between tool formats
- **Memory sync**: Keep memory/context files in sync across tools
- **Shell completions**: `aisync completions bash|zsh|fish`

## Architecture

aisync is a Rust workspace with two crates:

- **`aisync`** -- CLI binary with commands for init, sync, status, and watch
- **`aisync-core`** -- Core library with the adapter pattern, sync engine, and detection logic

Each AI tool has an adapter implementing the `ToolAdapter` trait, making it straightforward to add support for new tools.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for details.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, code style, and PR guidelines.

## License

[MIT](LICENSE)
