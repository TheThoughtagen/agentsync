# Phase 6: Core Refactoring - Context

**Gathered:** 2026-03-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Refactor core types and ToolAdapter trait so that adding a new adapter is a single-file addition. No new adapters are added in this phase — the goal is eliminating scattered match arms, hardcoded metadata, and rigid config structures that block extensibility.

Requirements: REFAC-01, REFAC-02, REFAC-03, REFAC-04.

</domain>

<decisions>
## Implementation Decisions

### ToolKind extensibility
- Extend ToolKind with `Custom(String)` variant for community/TOML-defined adapters
- Built-in tools (ClaudeCode, Cursor, OpenCode) remain as named enum variants; Windsurf and Codex will be added as named variants in Phase 7
- `Custom(String)` is reserved for Phase 10+ (declarative TOML adapters)
- ToolKind loses `Copy` derive — migrate all ~40 usage sites to `Clone`
- Custom serialization: all variants serialize as lowercase strings ("claude-code", "cursor", "opencode"); Custom(s) serializes as the string itself

### Metadata consolidation
- All tool metadata moves into ToolAdapter trait methods — this is what makes "one-file adapter" work
- New trait methods: `display_name()`, `native_instruction_path()`, `gitignore_entries()`, `conditional_tags()`, `watch_paths()`
- Eliminates hardcoded display names in 7 files (init.rs, status.rs, diff.rs, hooks.rs, sync.rs, check.rs, commands/diff.rs)
- Eliminates hardcoded native paths in init.rs, diff.rs, watch.rs
- Eliminates hardcoded conditional tags in conditional.rs

### AnyAdapter dispatch
- Use a `dispatch_adapter!` macro to generate match arms for all ToolAdapter method impls on AnyAdapter
- Each new method or variant becomes one line instead of O(methods × variants) boilerplate
- Plugin variant uses `Box<dyn ToolAdapter>` for dynamic dispatch (built-in variants stay zero-cost)

### Config deserialization
- Replace ToolsConfig named fields (claude_code, cursor, opencode) with `BTreeMap<String, ToolConfig>`
- Well-known tools ("claude-code", "cursor", "opencode") are just well-known map keys — no special treatment
- Existing aisync.toml files deserialize identically (same TOML table structure)
- Provide helper methods on ToolsConfig: `get_tool()`, `configured_tools()`, `is_enabled()` — callers don't use raw map access

### Claude's Discretion
- Migration order within the phase (which refactor to tackle first)
- Whether to split into multiple plans or handle as one
- Exact macro syntax and error handling approach
- How to handle the `todo!()` default impls in ToolAdapter (keep, remove, or convert to proper defaults)

</decisions>

<specifics>
## Specific Ideas

- Plugin variant in AnyAdapter: `Plugin(Box<dyn ToolAdapter>)` — dyn dispatch only for plugins, enum dispatch for built-in
- ToolKind serialization should be custom (not default serde derive) to produce clean lowercase strings
- The dispatch macro pattern: `dispatch_adapter!(self, a => a.method(args))`

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `adapter.rs`: ToolAdapter trait and AnyAdapter enum — the primary refactoring target
- `types.rs`: ToolKind enum — needs Custom(String) variant and custom Serialize/Deserialize
- `config.rs`: ToolsConfig, ToolConfig, AisyncConfig — needs BTreeMap migration

### Established Patterns
- Enum-dispatch: AnyAdapter delegates to concrete adapter structs (ClaudeCodeAdapter, CursorAdapter, OpenCodeAdapter)
- Zero-sized adapter structs: each adapter is `#[derive(Debug, Clone)]` with no fields
- Default trait method impls: some ToolAdapter methods have `todo!()` defaults

### Integration Points
- CLI commands (init, status, diff, sync, check, hooks) all reference ToolKind display names — need to call adapter.display_name() instead
- `watch.rs`: hardcoded native paths for reverse-sync detection — need to call adapter.watch_paths()
- `conditional.rs`: hardcoded tag lists — need to call adapter.conditional_tags()
- `init.rs`: ToolKind-to-native-path mapping and ToolKind-to-AnyAdapter construction — need adapter registry

### Key Files Affected
- `crates/aisync-core/src/adapter.rs` (trait + enum)
- `crates/aisync-core/src/types.rs` (ToolKind)
- `crates/aisync-core/src/config.rs` (ToolsConfig)
- `crates/aisync-core/src/init.rs` (match arms)
- `crates/aisync-core/src/diff.rs` (match arms)
- `crates/aisync-core/src/watch.rs` (match arms)
- `crates/aisync-core/src/conditional.rs` (match arms)
- `crates/aisync/src/commands/` (display name match arms in 5+ files)

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 06-core-refactoring*
*Context gathered: 2026-03-08*
