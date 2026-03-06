---
status: resolved
trigger: "sync --dry-run shows 'Would skip ./CLAUDE.md' with no actionable guidance"
created: 2026-03-05T00:00:00Z
updated: 2026-03-05T00:00:00Z
---

## Current Focus

hypothesis: The SkipExistingFile Display impl and dry-run rendering provide no remediation hints
test: Read Display impl for SkipExistingFile and print_dry_run function
expecting: No actionable text like "use --force" or "back up and remove"
next_action: document root cause

## Symptoms

expected: Dry-run shows clear, actionable preview -- when a file conflicts, tells user what to do
actual: Shows "Would skip ./CLAUDE.md: CLAUDE.md is a regular file, not managed by aisync" with no next steps
errors: N/A (not an error, just unhelpful UX)
reproduction: `aisync init` with existing CLAUDE.md, then `cargo run -- sync --dry-run`
started: Since SkipExistingFile was implemented

## Eliminated

(none needed -- root cause is clear from code reading)

## Evidence

- timestamp: 2026-03-05T00:00:00Z
  checked: SyncAction::SkipExistingFile Display impl (types.rs:62-64)
  found: Display writes "Would skip {path}: {reason}" -- no remediation hint
  implication: The dry-run message is purely descriptive, never prescriptive

- timestamp: 2026-03-05T00:00:00Z
  checked: print_dry_run in sync.rs:48-70
  found: Simply prints `{action}` via Display -- no special handling for SkipExistingFile
  implication: Dry-run path has no branch to add guidance for skip actions

- timestamp: 2026-03-05T00:00:00Z
  checked: plan_sync in claude_code.rs:90-97
  found: Returns SkipExistingFile with reason "CLAUDE.md is a regular file, not managed by aisync" -- factual but not actionable
  implication: The reason string itself lacks guidance

- timestamp: 2026-03-05T00:00:00Z
  checked: handle_interactive_prompts in sync.rs:72-127
  found: Non-dry-run path DOES handle SkipExistingFile interactively (prompts user to replace with symlink)
  implication: The interactive path has the right UX; only the dry-run path is missing it

## Resolution

root_cause: Two gaps -- (1) SkipExistingFile Display impl and dry-run print path provide no remediation guidance, and (2) there is no --force flag to allow non-interactive resolution
fix: See artifacts below
verification: N/A (diagnosis only)
files_changed: []
