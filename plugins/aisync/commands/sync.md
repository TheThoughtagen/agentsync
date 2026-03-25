---
name: sync
description: Sync .ai/ canonical config to all configured AI tools
---

# /aisync:sync

Run `aisync sync` in the project root to propagate canonical `.ai/` configuration to all configured AI tools (Claude Code, Cursor, OpenCode).

## Usage

1. Run the command:
   ```bash
   aisync sync
   ```
   If the user mentions `--dry-run`, add that flag to preview changes without writing:
   ```bash
   aisync sync --dry-run
   ```

2. Parse the output and report to the user:
   - Which files were **created** (new synced files)
   - Which files were **updated** (changed synced files)
   - Which files were **symlinked** (linked rather than copied)
   - Group results by tool (Claude Code, Cursor, OpenCode)

3. If `aisync` is not installed or the command is not found, tell the user:
   > `aisync` CLI is not installed. Install it with `cargo install --path crates/aisync` from the agentsync repository, or see the README for installation instructions.

## Notes

- This command reads from `.ai/` (the canonical source of truth) and writes to tool-native locations like `CLAUDE.md`, `.cursor/rules/`, and `AGENTS.md`.
- Always run from the project root directory where `.ai/` exists.
- Use `--dry-run` first if you want to preview what will change.
