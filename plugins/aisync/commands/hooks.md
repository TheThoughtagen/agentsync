---
name: hooks
description: Manage hook definitions — list, add, or translate hooks across tools
---

# /aisync:hooks

Manage hook definitions in `.ai/hooks.toml`. Accepts a subcommand argument: `list`, `add`, or `translate`.

## Usage

### List hooks
```bash
aisync hooks list
```
Run this to display all defined hooks. Present the output showing each hook's event, matcher, command, and per-tool support info (which tools support each hook natively).

### Add a hook
```bash
aisync hooks add <event> --matcher <matcher> --command <command> [--timeout <seconds>]
```
Run with the user's specified parameters to add a new hook definition to `.ai/hooks.toml`.

### Translate hooks
```bash
aisync hooks translate
```
Run this to preview how canonical hooks translate to each tool's native format. Show the translated output grouped by tool so the user can see exactly what each tool will receive.

## Notes

- Hook definitions live in `.ai/hooks.toml` — always edit there, not in tool-native files.
- Use `list` to audit current hooks and check tool compatibility.
- Use `translate` to verify cross-tool behavior before syncing.
- After adding or modifying hooks, run `/aisync:sync` to propagate changes.
