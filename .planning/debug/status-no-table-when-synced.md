---
status: diagnosed
trigger: "status command shows only 'All 3 tool(s) in sync' with no per-tool breakdown"
created: 2026-03-05T00:00:00Z
updated: 2026-03-05T00:00:00Z
---

## Current Focus

hypothesis: print_status_table early-returns with a summary line when all_in_sync(), skipping the per-tool table
test: read the function logic
expecting: early return before table rendering
next_action: return diagnosis

## Symptoms

expected: Colored table showing each tool name, strategy, and drift state (OK/DRIFTED/MISSING)
actual: Only shows "All 3 tool(s) in sync" with no breakdown
errors: none
reproduction: cargo run -- status (when all tools are in sync)
started: initial implementation

## Eliminated

(none needed - root cause found on first pass)

## Evidence

- timestamp: 2026-03-05
  checked: print_status_table() in crates/aisync/src/commands/status.rs lines 41-58
  found: When status.all_in_sync() is true, function prints one summary line and returns immediately (line 58). The per-tool table (lines 61-83) is only rendered when tools are NOT all in sync.
  implication: The "all in sync" happy path intentionally skips the table. Verbose mode shows per-tool info on stderr, but default mode has no breakdown.

## Resolution

root_cause: print_status_table() early-returns on line 42-58 when all_in_sync() is true, printing only a summary count and skipping the per-tool colored table entirely
fix: (not applied - diagnosis only)
verification: (not applied)
files_changed: []
