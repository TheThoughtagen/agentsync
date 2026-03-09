---
phase: 10-declarative-toml-adapters
plan: 01
subsystem: adapters
tags: [toml, declarative, plugin-sdk, serde, tooladapter]

requires:
  - phase: 09-plugin-sdk-crate-extraction
    provides: ToolAdapter trait in aisync-adapter crate, AdapterError with Io/Other variants
provides:
  - DeclarativeAdapterDef TOML schema for adapter authoring without Rust
  - DeclarativeAdapter implementing all 13 ToolAdapter trait methods
  - load_toml_adapter() function for parsing TOML files into working adapters
  - Builtin name collision guard
affects: [10-02-PLAN, phase-11]

tech-stack:
  added: []
  patterns: [Box::leak for static str lifetime in trait returns, declarative TOML adapter schema]

key-files:
  created:
    - crates/aisync-core/src/declarative.rs
  modified:
    - crates/aisync-core/src/lib.rs

key-decisions:
  - "Box::leak pattern for conditional_tags/watch_paths &'static str lifetime (acceptable for program-lifetime adapters)"
  - "Custom Default impl for DetectionDef to ensure match_any defaults to true even when entire section omitted"
  - "strip_frontmatter helper supports arbitrary delimiter strings (not just ---)"

patterns-established:
  - "TOML adapter schema: name, display_name, detection, sync, template sections"
  - "DeclarativeAdapter constructor validates name against BUILTIN_NAMES before accepting"

requirements-completed: [SDK-03, SDK-04]

duration: 5min
completed: 2026-03-09
---

# Phase 10 Plan 01: TOML Adapter Schema and DeclarativeAdapter Summary

**DeclarativeAdapterDef TOML schema with full ToolAdapter implementation supporting Generate/Symlink/Copy strategies, detection with match_any/match_all semantics, and builtin name collision guard**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-09T03:32:33Z
- **Completed:** 2026-03-09T03:37:33Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- DeclarativeAdapterDef struct deserializes from TOML with name, display_name, detection, sync, and template sections
- DeclarativeAdapter implements all 13 ToolAdapter trait methods via delegation to parsed TOML definition
- detect() supports match_any (OR) and match_all (AND) semantics with Medium confidence
- plan_sync() handles Generate (template with {{content}} interpolation), Symlink, and Copy strategies
- Builtin name collision guard rejects adapter names matching claude-code/cursor/opencode/windsurf/codex
- 40 unit tests covering all behaviors

## Task Commits

Each task was committed atomically:

1. **Task 1: Define TOML schema structs and DeclarativeAdapter with ToolAdapter implementation** - `f240433` (feat)

**Plan metadata:** (pending docs commit)

## Files Created/Modified
- `crates/aisync-core/src/declarative.rs` - DeclarativeAdapterDef, DeclarativeAdapter, load_toml_adapter (1013 lines)
- `crates/aisync-core/src/lib.rs` - Added pub mod declarative and re-exports

## Decisions Made
- Used Box::leak pattern for conditional_tags() and watch_paths() to satisfy &[&str] and Vec<&str> trait return types without changing the SDK trait signature
- Implemented custom Default for DetectionDef so match_any defaults to true even when entire [detection] section is omitted from TOML
- strip_frontmatter helper accepts arbitrary delimiter strings, not just "---", enabling adapters with different frontmatter formats (e.g., "+++")

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed DetectionDef Default implementation**
- **Found during:** Task 1
- **Issue:** derive(Default) set match_any to false, but TOML spec requires true as default
- **Fix:** Replaced derive(Default) with manual Default impl that sets match_any: true
- **Files modified:** crates/aisync-core/src/declarative.rs
- **Verification:** test_deserialize_minimal_toml passes

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Necessary for correctness. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- DeclarativeAdapter ready for integration into adapter registry (10-02)
- load_toml_adapter() ready for use by adapter discovery/loading system
- TOML schema stable for community adapter authoring

---
*Phase: 10-declarative-toml-adapters*
*Completed: 2026-03-09*
