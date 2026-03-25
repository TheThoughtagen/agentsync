---
name: init
description: Initialize .ai/ directory with tool detection and config import
---

# /aisync:init

Run `aisync init` to set up the canonical `.ai/` directory structure and import existing tool configurations.

## Usage

1. Run the command:
   ```bash
   aisync init
   ```

2. Report the results to the user:
   - Which AI tools were **detected** in the project (Claude Code, Cursor, OpenCode)
   - Which existing configs were **imported** into `.ai/` (e.g., existing `CLAUDE.md` content imported into `.ai/instructions.md`)
   - What new files and directories were created

3. If `.ai/` already exists, **warn the user** before proceeding:
   > The `.ai/` directory already exists. Running `aisync init` again may overwrite imported content. Use `/aisync:status` to check current state instead, or pass `--force` if you really want to re-initialize.

## Notes

- This is typically run once when first adopting aisync in a project.
- Detection looks for `.claude/`, `.cursor/`, `.opencode/`, `CLAUDE.md`, `AGENTS.md`, and similar tool-native files.
- After init, run `/aisync:sync` to propagate the canonical config back to all tools.
