---
status: complete
phase: 07-windsurf-codex-adapters
source: 07-01-SUMMARY.md, 07-02-SUMMARY.md
started: 2026-03-08T19:30:00Z
updated: 2026-03-08T19:45:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Windsurf Detection
expected: Windsurf is detected when .windsurf/rules/ directory exists, and legacy .windsurfrules file is also recognized
result: pass

### 2. Codex Detection
expected: Codex is detected when .codex/ directory exists, but NOT detected via AGENTS.md alone (to avoid conflict with OpenCode)
result: pass

### 3. Windsurf Sync Output
expected: WindsurfAdapter generates .windsurf/rules/project.md with YAML frontmatter (trigger: always_on, description field) — not .mdc format
result: pass

### 4. Codex Sync Output
expected: CodexAdapter creates an AGENTS.md symlink pointing to the canonical source, mirroring OpenCode's symlink strategy
result: pass

### 5. Idempotent CreateSymlink (Codex+OpenCode)
expected: When both Codex and OpenCode are enabled and both target AGENTS.md, CreateSymlink skips if correct symlink already exists — no "file exists" error
result: pass

### 6. SyncEngine Deduplication
expected: When both Codex and OpenCode are enabled, dry-run produces only one AGENTS.md action (first-adapter-wins), not duplicate actions
result: pass

### 7. Content Size Warnings
expected: Windsurf warns when content exceeds 12,000 chars; Codex warns when content exceeds 32,768 bytes. Warnings are advisory WarnContentSize actions (no filesystem changes)
result: pass

### 8. All Tests Pass
expected: `cargo test --workspace` passes all 261+ tests with zero failures, and `cargo clippy --workspace -- -D warnings` is clean
result: pass

## Summary

total: 8
passed: 8
issues: 0
pending: 0
skipped: 0

## Gaps

[none yet]
