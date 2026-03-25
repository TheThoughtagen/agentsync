---
name: status
description: Show per-tool sync status and drift detection
---

# /aisync:status

Run `aisync status --json` to check the current sync state of all configured AI tools.

## Usage

1. Run the command:
   ```bash
   aisync status --json
   ```

2. Parse the JSON output and present a readable summary to the user. For each tool, show:
   - **Synced** — tool-native files match the canonical `.ai/` content
   - **Drifted** — tool-native files have been modified outside of aisync and differ from canonical
   - **Not configured** — tool is detected but no sync configuration exists

3. Format the summary as a clear table or list so the user can quickly see which tools need attention.

4. If drift is detected, suggest running `/aisync:sync` to bring things back in line, or `/aisync:diff` to inspect the differences first.

## Notes

- The `--json` flag produces machine-readable output for easier parsing.
- Without `--json`, human-readable output is produced which can be shown directly.
