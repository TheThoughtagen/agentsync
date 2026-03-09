---
phase: 10-declarative-toml-adapters
verified: 2026-03-08T22:00:00Z
status: passed
score: 9/9 must-haves verified
re_verification: false
---

# Phase 10: Declarative TOML Adapters Verification Report

**Phase Goal:** Users can define new tool adapters via TOML files in `.ai/adapters/` without writing Rust
**Verified:** 2026-03-08T22:00:00Z
**Status:** passed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | DeclarativeAdapterDef deserializes from TOML with name, display_name, detection, sync, template sections | VERIFIED | Full and minimal TOML deserialization tests pass (test_deserialize_full_toml, test_deserialize_minimal_toml); struct at line 17 with all fields |
| 2 | DeclarativeAdapter implements all ToolAdapter trait methods | VERIFIED | `impl ToolAdapter for DeclarativeAdapter` at line 160, covers name/display_name/native_instruction_path/conditional_tags/gitignore_entries/watch_paths/default_sync_strategy/detect/read_instructions/plan_sync/sync_status/plan_memory_sync/translate_hooks |
| 3 | detect() checks directories/files with match_any/match_all semantics | VERIFIED | Tests: test_detect_match_any_dir, test_detect_match_any_file, test_detect_match_any_none_found, test_detect_match_all_all_present, test_detect_match_all_partial, test_detect_empty_markers |
| 4 | plan_sync() Generate strategy uses template with {{content}} interpolation | VERIFIED | test_plan_sync_generate_creates_file verifies template interpolation; test_plan_sync_generate_creates_directory verifies parent dir creation; test_plan_sync_generate_idempotent verifies no-op on unchanged |
| 5 | plan_sync() Symlink strategy creates symlink to .ai/instructions.md | VERIFIED | test_plan_sync_symlink_creates_symlink checks link/target; test_plan_sync_symlink_idempotent checks no-op |
| 6 | Builtin name collision guard rejects claude-code/cursor/opencode/windsurf/codex | VERIFIED | test_rejects_builtin_names iterates all 5 builtin names; BUILTIN_NAMES const at line 79 |
| 7 | discover_toml_adapters() scans .ai/adapters/*.toml and returns parsed adapters | VERIFIED | Function at line 448; tests: test_discover_valid_toml_files (2 files), test_discover_no_adapters_dir, test_discover_empty_adapters_dir, test_discover_skips_malformed_toml, test_discover_skips_builtin_name_collisions, test_discover_skips_non_toml_files |
| 8 | SyncEngine::enabled_tools() includes TOML adapters alongside builtins | VERIFIED | sync.rs line 658 calls discover_toml_adapters; wraps in AnyAdapter::Plugin(Arc) at line 664; test_status_includes_toml_adapter and test_toml_adapter_disabled_by_config pass |
| 9 | DetectionEngine::scan() includes TOML adapters in detection results | VERIFIED | detection.rs line 42 calls discover_toml_adapters; test_scan_includes_toml_adapters passes |

**Score:** 9/9 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/aisync-core/src/declarative.rs` | DeclarativeAdapterDef, DeclarativeAdapter, load_toml_adapter, discover_toml_adapters | VERIFIED | 1155 lines; all structs, impls, helpers, and 46 tests present |
| `crates/aisync-core/src/lib.rs` | pub mod declarative + re-exports | VERIFIED | Line 6: `pub mod declarative`; Line 22: `pub use declarative::{DeclarativeAdapter, DeclarativeAdapterDef, discover_toml_adapters}` |
| `crates/aisync-core/src/sync.rs` | enabled_tools() extended for TOML adapters | VERIFIED | Line 658: discover_toml_adapters call; line 664: AnyAdapter::Plugin wrapping |
| `crates/aisync-core/src/detection.rs` | scan() extended for TOML adapters | VERIFIED | Line 4: import; line 42: discover_toml_adapters call with non-fatal error handling |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| declarative.rs | aisync-adapter/src/lib.rs | impl ToolAdapter for DeclarativeAdapter | WIRED | Line 160 |
| declarative.rs | aisync-types/src/lib.rs | ToolKind::Custom usage | WIRED | Lines 162, 216 |
| sync.rs | declarative.rs | calls discover_toml_adapters | WIRED | Line 658 |
| detection.rs | declarative.rs | calls discover_toml_adapters | WIRED | Line 42 |
| sync.rs | adapter.rs | AnyAdapter::Plugin(Arc::new(...)) | WIRED | Line 664 |
| diff.rs | sync.rs | enabled_tools(config, project_root) | WIRED | Line 31 |
| watch.rs | sync.rs | enabled_tools(config, project_root) | WIRED | Line 200 |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| SDK-03 | 10-01 | Declarative TOML adapter schema supports detection rules, file mappings, sync strategy, and templates | SATISFIED | DeclarativeAdapterDef with DetectionDef, SyncDef, TemplateDef structs; full TOML deserialization verified |
| SDK-04 | 10-01 | DeclarativeAdapter struct implements ToolAdapter from parsed TOML definitions | SATISFIED | impl ToolAdapter for DeclarativeAdapter covering all 13 trait methods; 46 tests pass |
| SDK-05 | 10-02 | .ai/adapters/*.toml files auto-discovered and loaded as plugin adapters | SATISFIED | discover_toml_adapters() + SyncEngine/DetectionEngine integration; 12 detection tests + sync integration tests pass |

No orphaned requirements found. All 3 requirement IDs from ROADMAP.md Phase 10 (SDK-03, SDK-04, SDK-05) are covered by plans and verified.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | - | - | - | No anti-patterns found |

No TODOs, FIXMEs, placeholders, empty implementations, or console-log-only handlers detected.

### Success Criteria from ROADMAP.md

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | A .ai/adapters/mytool.toml with detection rules, file mappings, and sync strategy is auto-discovered on aisync sync | VERIFIED | discover_toml_adapters() in sync.rs enabled_tools(); test_status_includes_toml_adapter passes |
| 2 | The TOML-defined adapter appears in aisync status output alongside built-in adapters | VERIFIED | enabled_tools() includes TOML adapters in status flow; test_status_includes_toml_adapter confirms |
| 3 | A TOML adapter can generate output files using template syntax with instruction content interpolation | VERIFIED | plan_sync_generate() replaces {{content}} in template; test_plan_sync_generate_creates_file confirms interpolation |

### Human Verification Required

### 1. End-to-End CLI Flow

**Test:** Create a project with `.ai/adapters/aider.toml` containing detection rules and Generate strategy, then run `aisync sync` and `aisync status`
**Expected:** aider adapter appears in status output; sync generates the expected output file with template interpolation
**Why human:** CLI output formatting and real filesystem behavior with actual binary

### 2. Malformed TOML Warning Output

**Test:** Place a malformed `.toml` file in `.ai/adapters/` and run `aisync sync`
**Expected:** Warning printed to stderr, sync completes normally for other tools
**Why human:** stderr output formatting verification

### Gaps Summary

No gaps found. All 9 observable truths verified. All 3 requirements satisfied. All key links wired. All ROADMAP success criteria met. 46 declarative tests + detection and sync integration tests pass.

---

_Verified: 2026-03-08T22:00:00Z_
_Verifier: Claude (gsd-verifier)_
