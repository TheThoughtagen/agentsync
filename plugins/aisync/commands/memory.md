---
name: memory
description: Manage memory files — list, add, import, or export across tools
---

# /aisync:memory

Manage memory files in `.ai/memory/`. Accepts a subcommand argument: `list`, `add`, `import`, or `export`.

## Usage

### List memory files
```bash
aisync memory list
```
Run this to display all memory files in `.ai/memory/`. Present each file's name and a brief description of its contents.

### Add a memory file
```bash
aisync memory add <name> [--content <content>]
```
Run with the user's specified name to create a new memory file in `.ai/memory/`.

### Import memory
```bash
aisync memory import
```
Run this to import memory/context files from tool-native locations into `.ai/memory/`. Report which files were imported and from which tools.

### Export memory
```bash
aisync memory export
```
Run this to export `.ai/memory/` files to tool-native locations. Report which files were written and to which tools.

## Notes

- Memory files are markdown files in `.ai/memory/` that provide persistent context across sessions.
- These are synced to tool-native locations (e.g., `.claude/memory/`, `.cursor/context/`) via `/aisync:sync`.
- Always edit memory files in `.ai/memory/`, not in tool-native copies.
