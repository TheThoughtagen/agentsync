---
phase: 09-plugin-sdk-crate-extraction
plan: 02
subsystem: api
tags: [rust, trait-extraction, sdk, adapter-pattern, crate-splitting]

# Dependency graph
requires:
  - phase: 09-01
    provides: aisync-types crate with ToolKind, SyncStrategy, SyncAction, etc.
provides:
  - aisync-adapter crate with ToolAdapter trait, DetectionResult, AdapterError
  - Inverted dependency chain: types <- adapter <- core
  - Lightweight SDK for community adapter development
affects: [10-plugin-registration, 11-declarative-toml-adapters]

# Tech tracking
tech-stack:
  added: [aisync-adapter crate]
  patterns: [trait-in-sdk-crate, re-export-for-backward-compat, error-type-per-crate]

key-files:
  created:
    - crates/aisync-adapter/Cargo.toml
    - crates/aisync-adapter/src/lib.rs
  modified:
    - Cargo.toml
    - crates/aisync-core/Cargo.toml
    - crates/aisync-core/src/adapter.rs
    - crates/aisync-core/src/error.rs
    - crates/aisync-core/src/adapters/claude_code.rs
    - crates/aisync-core/src/adapters/cursor.rs
    - crates/aisync-core/src/adapters/opencode.rs
    - crates/aisync-core/src/adapters/windsurf.rs
    - crates/aisync-core/src/adapters/codex.rs

key-decisions:
  - "AdapterError expanded with Io and Other variants for community adapter ergonomics"
  - "ToolAdapter trait methods return AdapterError (not AisyncError) to decouple SDK from core error hierarchy"
  - "Backward compat via pub use re-exports in adapter.rs and error.rs"

patterns-established:
  - "SDK crate re-exports: pub use aisync_types in adapter crate for convenience"
  - "Error bridging: core wraps AdapterError in AisyncError::Adapter variant"

requirements-completed: [SDK-02]

# Metrics
duration: 7min
completed: 2026-03-09
---

# Phase 09 Plan 02: Adapter Trait Extraction Summary

**ToolAdapter trait and AdapterError extracted to aisync-adapter crate with all 5 adapters rewired and backward-compatible re-exports**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-09T02:49:03Z
- **Completed:** 2026-03-09T02:56:05Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments
- Created aisync-adapter crate with ToolAdapter trait, DetectionResult, and expanded AdapterError
- Rewired aisync-core to depend on aisync-adapter (inverted dependency for SDK architecture)
- All 5 concrete adapter implementations updated to return AdapterError
- Full backward compatibility maintained via re-exports (285 tests, 0 failures)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create aisync-adapter crate** - `bf58d0b` (feat)
2. **Task 2: Rewire aisync-core with backward-compatible re-exports** - `f56ecfe` (feat)

## Files Created/Modified
- `crates/aisync-adapter/Cargo.toml` - Package metadata, depends on aisync-types + thiserror only
- `crates/aisync-adapter/src/lib.rs` - ToolAdapter trait, DetectionResult, AdapterError (3 variants)
- `Cargo.toml` - Added aisync-adapter to workspace dependencies
- `crates/aisync-core/Cargo.toml` - Added aisync-adapter dependency
- `crates/aisync-core/src/adapter.rs` - Removed trait/struct definitions, added re-exports from aisync-adapter
- `crates/aisync-core/src/error.rs` - Replaced local AdapterError with re-export, updated AisyncError::Adapter source type
- `crates/aisync-core/src/adapters/claude_code.rs` - Returns AdapterError instead of AisyncError
- `crates/aisync-core/src/adapters/cursor.rs` - Returns AdapterError instead of AisyncError
- `crates/aisync-core/src/adapters/opencode.rs` - Returns AdapterError instead of AisyncError
- `crates/aisync-core/src/adapters/windsurf.rs` - Returns AdapterError instead of AisyncError
- `crates/aisync-core/src/adapters/codex.rs` - Returns AdapterError instead of AisyncError

## Decisions Made
- AdapterError expanded from 1 variant (DetectionFailed) to 3 (DetectionFailed, Io, Other) for community adapter ergonomics
- ToolAdapter trait methods return AdapterError to fully decouple SDK from core error hierarchy
- Backward compatibility maintained via `pub use` re-exports in adapter.rs and error.rs

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- SDK crate layer complete: types <- adapter <- core dependency chain
- Community developers can now implement ToolAdapter by depending only on aisync-adapter
- Ready for Phase 10 (Plugin Registration) and Phase 11 (Declarative TOML Adapters)

---
*Phase: 09-plugin-sdk-crate-extraction*
*Completed: 2026-03-09*

## Self-Check: PASSED
