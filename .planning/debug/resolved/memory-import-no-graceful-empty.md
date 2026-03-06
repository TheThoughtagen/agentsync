---
status: resolved
trigger: "memory import claude errors with ClaudeMemoryNotFound instead of graceful empty message"
created: 2026-03-06T00:00:00Z
updated: 2026-03-06T04:00:00Z
---

## Current Focus

hypothesis: CONFIRMED - core layer throws error on missing path; CLI propagates it unhandled
test: code trace through import_claude and run_import
expecting: n/a - diagnosis complete
next_action: none (find_root_cause_only)

## Symptoms

expected: `aisync memory import claude` shows friendly "no memories found" when Claude has no memories for this project
actual: Errors out with "claude memory path not found: ~/.claude/projects/<key>/memory"
errors: MemoryError::ClaudeMemoryNotFound propagated as Box<dyn Error>
reproduction: Run `aisync memory import claude` in a project where Claude has no auto-memories
started: Since import_claude was implemented

## Eliminated

(none needed - root cause found on first pass)

## Evidence

- timestamp: 2026-03-06
  checked: MemoryEngine::import_claude() in memory.rs lines 98-105
  found: When claude_path does not exist, immediately returns Err(MemoryError::ClaudeMemoryNotFound)
  implication: Core treats "no memories directory" as an error condition rather than an empty-result condition

- timestamp: 2026-03-06
  checked: run_import() in commands/memory.rs line 88
  found: Calls MemoryEngine::import_claude(project_root)? with ? operator - error propagates directly to caller with no match/handling
  implication: CLI has no chance to intercept ClaudeMemoryNotFound and show a friendly message

- timestamp: 2026-03-06
  checked: run_import() lines 90-93
  found: There IS already a graceful empty-state message ("No memory files found to import") but it only triggers when import succeeds with empty imported+conflicts vecs
  implication: The graceful path exists but is unreachable when the directory doesn't exist because the error fires first

- timestamp: 2026-03-06
  checked: MemoryEngine::list() lines 24-27 for comparison
  found: list() returns Ok(vec![]) when .ai/memory/ doesn't exist - graceful empty handling
  implication: Precedent in the codebase is to return empty results for missing directories, not errors

- timestamp: 2026-03-06
  checked: test_import_claude_errors_when_path_missing lines 362-375
  found: Test explicitly asserts that missing path IS an error
  implication: Test will need updating when behavior changes

## Resolution

root_cause: |
  MemoryEngine::import_claude() (memory.rs:101-104) treats a non-existent Claude memory directory
  as an error (MemoryError::ClaudeMemoryNotFound) rather than returning an empty ImportResult.
  The CLI's run_import() (commands/memory.rs:88) uses the ? operator, so this error propagates
  directly to the user as an ugly error message. The graceful "No memory files found to import"
  message at line 90-93 is unreachable in this scenario because the error fires before it.

  This is inconsistent with MemoryEngine::list() which returns Ok(vec![]) for a missing directory.

fix: (not applied - diagnosis only)
verification: (not applied - diagnosis only)
files_changed: []
