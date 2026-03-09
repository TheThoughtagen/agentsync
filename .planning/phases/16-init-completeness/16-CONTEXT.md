# Phase 16: Init Completeness - Context

**Gathered:** 2026-03-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Make `aisync init` produce a fully synced project — `aisync status` shows all tools OK immediately after init with zero drift. Fix ghost tools in status output, correct sync action messaging, and ensure source selection works for multiple instruction sources.

Requirements: INIT-01, INIT-02, INIT-03, INIT-04

</domain>

<decisions>
## Implementation Decisions

### Post-init auto-sync (INIT-01)
- Init calls `SyncEngine::execute()` as a final step after scaffolding `.ai/` and writing `aisync.toml`
- Full sync across all dimensions: instructions + rules + MCP + commands
- Per-tool errors are warnings, not fatal — init succeeds with partial sync and tells user to run `aisync sync` after fixing issues
- Output is summarized: one line per tool with action count (e.g., "Claude Code — 4 actions (symlink, 2 rules, 1 MCP)")
- Detailed per-action output available with `--verbose`
- Final line: "All N tools in sync" or "N/M tools synced. Run `aisync sync` after fixing issues."

### Sync action messages (INIT-04)
- Real sync uses past tense: "Created symlink", "Generated rule file", "Created MCP config"
- Dry-run uses conditional: "Would create symlink", "Would generate rule file"
- Current `Display` impl in `aisync-types/src/lib.rs` uses "Would create" for everything — needs split into real vs planned output

### Ghost tool filtering (INIT-02)
- `aisync status` shows ONLY tools explicitly listed in `aisync.toml`
- No FYI lines for detected-but-unconfigured tools
- `enabled_tools()` already filters by `config.tools.is_enabled()` — the fix is ensuring status doesn't add `NotConfigured` entries for tools that aren't in config at all

### Source selection UX (INIT-03)
- Current multi-source `Select` dialog in `commands/init.rs` already handles interactive source tool selection with 5-line previews
- Verify this works correctly with the new init flow — no UX changes needed

### Claude's Discretion
- Implementation approach for splitting Display into real vs dry-run methods
- Exact wording for each SyncAction variant's past-tense message
- How to aggregate action counts for the summarized per-tool output line
- Whether to add a `--no-sync` flag to skip the auto-sync step

</decisions>

<specifics>
## Specific Ideas

- Init output should flow naturally: scaffold section, then "Syncing..." section, then final status line
- The summarized sync output should categorize actions (symlink, rules, MCP, commands) not just count them
- Warning format for partial sync: "⚠ Cursor: .cursor/ not found, skipped"

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `SyncEngine::plan()` + `SyncEngine::execute()` in `crates/aisync-core/src/sync.rs` — full sync pipeline already exists
- `SyncEngine::enabled_tools()` at sync.rs:744 — iterates builtin + TOML + inventory adapters filtered by config
- `InitEngine::scaffold()` in `crates/aisync-core/src/init.rs` — current init scaffolding
- `run_sync` in `crates/aisync/src/commands/sync.rs` — CLI sync command (reference for output patterns)

### Established Patterns
- `SyncAction` Display impl at `crates/aisync-types/src/lib.rs:312` — the "Would create" text that needs fixing
- `ToolSyncResult` groups actions per tool — natural aggregation point for summarized output
- `DriftState::NotConfigured` used as catch-all for status errors — needs tighter scoping
- Managed files use `aisync-` prefix consistently (from Phase 13)

### Integration Points
- `run_init()` in `crates/aisync/src/commands/init.rs` — needs sync call after `InitEngine::scaffold()`
- `run_status()` reads `AisyncConfig` from `aisync.toml` — filtering happens at `enabled_tools()` level
- `SyncReport` → `ToolSyncResult` → `Vec<SyncAction>` — chain for generating summarized output

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 16-init-completeness*
*Context gathered: 2026-03-09*
