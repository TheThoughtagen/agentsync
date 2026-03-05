# Phase 1: Foundation and Data Model - Context

**Gathered:** 2026-03-05
**Status:** Ready for planning

<domain>
## Phase Boundary

The canonical data model, config schema, adapter trait, tool detection engine, and error types exist as a compilable Rust library that all future phases build on. No CLI commands, no sync logic — just the foundation.

Requirements: CLI-08, ADPT-04, ADPT-05

</domain>

<decisions>
## Implementation Decisions

### Tool Detection Markers
- Claude Code: detect via both CLAUDE.md and .claude/ directory — either triggers detection
- Cursor: detect via both .cursor/rules/ (current) and .cursorrules (legacy). Flag legacy format in status output
- OpenCode: detect via both AGENTS.md and opencode.json
- Ambiguous markers (e.g., AGENTS.md could be OpenCode or Copilot): report with confidence level (High/Medium), let user confirm during init

### Config Schema (aisync.toml)
- schema_version as top-level integer (schema_version = 1)
- Per-tool config via nested TOML tables: [tools.claude-code], [tools.cursor], etc.
- Each tool section has enabled, sync_strategy, and tool-specific fields
- Three sync strategies: symlink (default for macOS/Linux), copy (Windows fallback), generate (for tools needing transformation like Cursor .mdc)
- Global [defaults] section that tools inherit from — tools override only when they differ

### Adapter Trait Contract
- detect() returns structured DetectionResult { detected, confidence: High/Medium, markers_found: Vec<PathBuf>, version_hint: Option<String> }
- Error modeling: per-adapter error enum + thiserror derives, converting into top-level AisyncError
- Lean trait in Phase 1: detect() and name() only. read/write added in Phase 2, sync_memory/translate_hook in Phase 3, watch_paths in Phase 4
- Adapter dispatch strategy: Claude's Discretion (compile-time enum vs trait objects)

### Workspace Organization
- Cargo workspace with two crates: aisync-core (library) and aisync (binary)
- crates/ directory: crates/aisync-core/ and crates/aisync/
- Test fixtures at workspace root: fixtures/ with subdirectories simulating tool setups (claude-only/, multi-tool/, no-tools/)
- Binary name: aisync (matches CLI command)
- Rust 2024 edition (requires 1.85+)

### Claude's Discretion
- Adapter dispatch model (compile-time enum vs dyn trait objects)
- Internal module organization within aisync-core
- Specific thiserror message wording
- Test organization within crates

</decisions>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- None — greenfield project, only PRD.md exists

### Established Patterns
- None yet — Phase 1 establishes all patterns

### Integration Points
- aisync-core lib will be the dependency for the aisync binary crate
- Config parsing (aisync.toml) feeds into detection engine and all future sync operations
- DetectionResult struct will be consumed by aisync status, aisync init, and aisync sync in later phases

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 01-foundation-and-data-model*
*Context gathered: 2026-03-05*
