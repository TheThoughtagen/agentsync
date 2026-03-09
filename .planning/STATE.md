---
gsd_state_version: 1.0
milestone: v1.1
milestone_name: Adapter Expansion & Plugin SDK
status: unknown
last_updated: "2026-03-09T03:46:54.087Z"
progress:
  total_phases: 5
  completed_phases: 5
  total_plans: 11
  completed_plans: 11
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-07)

**Core value:** Every AI tool working on a project sees the same instructions, memory, and hooks -- always in sync, zero manual copying.
**Current focus:** Phase 10 - Declarative TOML Adapters

## Current Position

Phase: 10 of 11 (Declarative TOML Adapters) -- fifth of 6 v1.1 phases
Plan: 2 of 2 complete
Status: Phase Complete
Last activity: 2026-03-09 -- Completed 10-02 (TOML Adapter Discovery & Integration)

Progress: [██████████] 95%

## Performance Metrics

**Velocity:**
- Total plans completed: 28 (20 v1.0 + 8 v1.1)
- v1.1 plans completed: 11
- Average duration: 7min
- Total execution time: 54min

## Accumulated Context

### Decisions

- Compile-time plugin registration for v1.1 (defer dynamic/WASM loading)
- Two-layer Plugin SDK: declarative TOML for simple adapters, Rust trait for complex ones
- Windsurf + Codex adapters in v1.1; Aider + Continue deferred to community/v1.2
- `add-tool` uses auto-detect + interactive selection
- Refactor ToolAdapter trait BEFORE adding new adapters (eliminates shotgun surgery)
- Custom(String) variant returns empty conditional tag names -- no tool-specific sections for custom tools yet
- tool_display_name returns String (not &'static str) to support Custom variant dynamic names
- Tool name parsing in add-tool CLI uses match statement (simpler than lookup table, avoids clippy type_complexity)
- Non-interactive add-tool lists unconfigured tools with --tool hint (no error on piped stdin)
- Custom tools use ClaudeCode adapter as fallback in init until adapter registry exists
- Plugin variant uses Arc<dyn ToolAdapter> for Clone+Send+Sync compatibility
- ToolKind::display_name() added as bridging pattern for call sites without adapter access
- todo!() defaults replaced with safe returns (Ok(None), Ok(vec![]), NotConfigured)
- BTreeMap field in ToolsConfig is private; all access via helper methods (get_tool, is_enabled, set_tool)
- Unconfigured-is-enabled semantics preserved via is_none_or in is_enabled()
- CreateSymlink made idempotent (skip if correct symlink exists) for Codex+OpenCode AGENTS.md sharing
- Windsurf uses SyncAction::CreateFile (not GenerateMdc) since output is .md not .mdc
- Codex detects only via .codex/ directory (not AGENTS.md) to avoid OpenCode detection conflict
- Deduplication uses first-adapter-wins strategy based on enabled_tools() iteration order
- Windsurf checks chars().count() for 12K char limit; Codex checks .len() for 32 KiB byte limit
- Size warnings are advisory WarnContentSize actions (no filesystem change)
- Reuse InitError for add_tool errors (ScaffoldFailed for IO, ImportFailed for serialization)
- Omit sync_strategy from ToolConfig when adapter default is Symlink (keeps TOML clean)
- plan_for_tools runs full plan then filters results (simplest correct approach preserving deduplication)
- Re-export SyncStrategy in config.rs for backward compatibility (avoids crate::config::SyncStrategy path breakage)
- Re-export all types via pub use aisync_types::* in types.rs (single re-export point)
- AdapterError expanded with Io and Other variants for community adapter ergonomics
- ToolAdapter trait methods return AdapterError (not AisyncError) to decouple SDK from core error hierarchy
- Backward compat via pub use re-exports in adapter.rs and error.rs
- Box::leak pattern for conditional_tags/watch_paths &'static str lifetime (acceptable for program-lifetime adapters)
- Custom Default impl for DetectionDef to ensure match_any defaults to true even when entire section omitted
- strip_frontmatter helper supports arbitrary delimiter strings (not just ---)
- Strategy fallback uses adapter.default_sync_strategy() instead of config.defaults when no tool_config exists
- TOML adapter detection errors are non-fatal (eprintln warning) unlike builtin adapter errors which return Err

### Pending Todos

None.

### Blockers/Concerns

- `inventory` 0.3 compatibility with Rust 2024 edition needs verification before Phase 11
- ToolKind Copy vs Custom(String) decision — DECIDED: Custom(String) variant, Clone migration

## Session Continuity

Last session: 2026-03-09
Stopped at: Completed 10-02-PLAN.md (Phase 10 complete)
Resume file: .planning/phases/10-declarative-toml-adapters/10-02-SUMMARY.md
Next: Phase 11 (Plugin Registry)
