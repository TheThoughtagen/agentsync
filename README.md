# aisync

Sync AI tool configurations across Claude Code, Cursor, and OpenCode.

Every AI tool working on a project sees the same instructions, memory, and hooks -- always in sync, zero manual copying.

## Install

```sh
cargo install aisync
```

## Usage

```sh
# Initialize .ai/ directory with tool detection
aisync init

# Sync instructions to all configured tools
aisync sync

# Show per-tool sync status
aisync status

# Watch for changes and auto-sync
aisync watch
```

## License

MIT
