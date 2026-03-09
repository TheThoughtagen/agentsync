---
status: complete
phase: 10-declarative-toml-adapters
source: 10-01-SUMMARY.md, 10-02-SUMMARY.md
started: 2026-03-08T12:00:00Z
updated: 2026-03-08T12:05:00Z
---

## Current Test

[testing complete]

## Tests

### 1. TOML Adapter Auto-Discovery
expected: Place a valid .toml adapter file in .ai/adapters/ and run `cargo test` — discover_toml_adapters() finds and parses it into a working DeclarativeAdapter. All 297 aisync-core tests pass.
result: pass

### 2. Builtin Name Collision Guard
expected: Attempting to load a TOML adapter with name "claude-code", "cursor", "opencode", "windsurf", or "codex" is rejected. The collision guard prevents overriding builtin adapters.
result: pass

### 3. Detection with Match Semantics
expected: A TOML adapter with detection rules (files/directories) is detected by DetectionEngine.scan(). match_any (OR) and match_all (AND) semantics work correctly.
result: pass

### 4. Generate Strategy with Template Interpolation
expected: A TOML adapter using Generate strategy applies its template with {{content}} placeholder replaced by actual content during sync planning.
result: pass

### 5. Malformed TOML Graceful Skip
expected: A malformed .toml file in .ai/adapters/ is skipped with a warning instead of crashing the discovery process. Other valid adapters still load successfully.
result: pass

### 6. Strategy Fallback to Adapter Default
expected: When no explicit tool_config exists for a TOML adapter, the sync engine uses the adapter's own default_sync_strategy() instead of the global config default.
result: pass

### 7. SyncEngine Integration
expected: TOML adapters appear alongside builtin adapters in SyncEngine.enabled_tools() and are included in sync plans when their tool is detected/enabled.
result: pass

## Summary

total: 7
passed: 7
issues: 0
pending: 0
skipped: 0

## Gaps

[none yet]
