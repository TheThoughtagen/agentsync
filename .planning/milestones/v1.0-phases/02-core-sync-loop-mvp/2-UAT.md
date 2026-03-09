---
status: resolved
phase: 02-core-sync-loop-mvp
source: 02-01-SUMMARY.md, 02-02-SUMMARY.md, 02-03-SUMMARY.md, 02-04-SUMMARY.md
started: 2026-03-05T23:20:00Z
updated: 2026-03-06T00:00:00Z
---

## Current Test

[testing complete]

## Tests

### 1. aisync init scaffolds project
expected: Running `cargo run -- init` in a directory without .ai/ creates the .ai/ directory structure with instructions.md, memory/, hooks/, commands/, and aisync.toml. Interactive prompts ask about detected tools and import sources.
result: pass

### 2. aisync sync dry-run
expected: Running `cargo run -- sync --dry-run` shows a preview of planned sync actions for each detected tool (e.g., CreateSymlink for ClaudeCode, GenerateFile for Cursor) without making any filesystem changes.
result: issue
reported: "is that claude.md part right? — dry-run says 'Would skip ./CLAUDE.md: CLAUDE.md is a regular file, not managed by aisync' with no guidance on how to resolve (e.g., delete original and re-sync, or use --force)"
severity: minor

### 3. aisync sync executes
expected: Running `cargo run -- sync` creates the actual symlinks/files for detected tools. ClaudeCode gets a CLAUDE.md symlink, OpenCode gets AGENTS.md symlink, Cursor gets .cursor/rules/project.mdc generated file. Output shows colored results per tool.
result: pass

### 4. aisync status shows sync state
expected: Running `cargo run -- status` after sync shows a colored table with each tool's drift state (OK, DRIFTED, MISSING, etc.). All synced tools show OK status.
result: issue
reported: "no table — just shows 'All 3 tool(s) in sync' with no per-tool breakdown or drift states"
severity: minor

### 5. aisync status JSON output
expected: Running `cargo run -- status --json` outputs machine-readable JSON with tool sync states instead of the colored table.
result: pass

### 6. Idempotent sync
expected: Running `cargo run -- sync` a second time produces zero actions — output indicates nothing to do since all files are already in sync.
result: pass

### 7. Gitignore managed section
expected: After sync, .gitignore contains an aisync-managed section (between marker comments) listing the synced files. Running sync again does not duplicate entries.
result: pass

## Summary

total: 7
passed: 5
issues: 2
pending: 0
skipped: 0

## Gaps

- truth: "Dry-run shows clear, actionable preview of planned sync actions per tool"
  status: resolved
  reason: "User reported: dry-run says 'Would skip ./CLAUDE.md: CLAUDE.md is a regular file, not managed by aisync' with no guidance on how to resolve"
  severity: minor
  test: 2
  root_cause: "SkipExistingFile Display impl and print_dry_run provide no remediation guidance — describe conflict but never tell user how to resolve"
  artifacts:
    - path: "crates/aisync-core/src/types.rs"
      issue: "SkipExistingFile Display impl shows no remediation hint (lines 62-64)"
    - path: "crates/aisync/src/commands/sync.rs"
      issue: "print_dry_run has no special handling for skip actions (lines 48-70)"
  missing:
    - "Add hint text in print_dry_run for SkipExistingFile actions (e.g., 're-run without --dry-run to be prompted')"
    - "Consider --force flag for auto-replace without prompting"
  debug_session: ".planning/debug/sync-dryrun-skip-no-guidance.md"

- truth: "Status command shows colored table with per-tool drift states (OK, DRIFTED, MISSING, etc.)"
  status: resolved
  reason: "User reported: no table — just shows 'All 3 tool(s) in sync' with no per-tool breakdown or drift states"
  severity: minor
  test: 4
  root_cause: "print_status_table() early-returns at lines 42-58 when all_in_sync() is true, skipping the per-tool table entirely"
  artifacts:
    - path: "crates/aisync/src/commands/status.rs"
      issue: "Early return in print_status_table() skips table when all tools are in sync (lines 42-58)"
  missing:
    - "Remove early-return guard so per-tool table always renders regardless of sync state"
    - "Optionally keep green summary line after the table when everything is OK"
  debug_session: ".planning/debug/status-no-table-when-synced.md"
