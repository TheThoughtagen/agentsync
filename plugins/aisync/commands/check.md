---
name: check
description: Check sync state (CI-friendly, exits non-zero on drift)
---

# /aisync:check

Run `aisync check` to verify that all tool-native files are in sync with canonical `.ai/` content. This command is designed for CI pipelines — it exits with a non-zero status code if drift is detected.

## Usage

1. Run the command:
   ```bash
   aisync check
   ```

2. Interpret the result:
   - **Exit code 0** — All tools are in sync. Report a **pass** to the user.
   - **Non-zero exit code** — Drift detected. Report a **fail** and show which files are out of sync.

3. On failure, suggest running `/aisync:sync` to resolve the drift:
   > Sync check failed — some tool-native files have drifted from canonical `.ai/` content. Run `/aisync:sync` to bring them back in line.

## Notes

- This is the recommended command for CI/CD integration and pre-commit hooks.
- Use `/aisync:diff` to see the specific differences before syncing.
- The command is read-only — it never modifies files.
