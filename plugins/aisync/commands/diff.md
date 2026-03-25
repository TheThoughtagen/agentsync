---
name: diff
description: Compare canonical .ai/ content vs tool-native files
---

# /aisync:diff

Run `aisync diff` to see the differences between canonical `.ai/` content and each tool's native configuration files.

## Usage

1. Run the command:
   ```bash
   aisync diff
   ```

2. Present the diff output to the user, showing:
   - Which files differ between canonical and tool-native versions
   - The specific changes (additions, deletions, modifications)
   - Which tool each difference belongs to

3. If there are no differences, confirm that all tools are in sync.

4. If differences exist, suggest:
   - `/aisync:sync` to overwrite tool-native files with canonical content
   - Or manually editing `.ai/` files if the tool-native changes should be kept

## Notes

- This is a read-only operation — it does not modify any files.
- Useful for reviewing drift before deciding whether to sync or update canonical files.
