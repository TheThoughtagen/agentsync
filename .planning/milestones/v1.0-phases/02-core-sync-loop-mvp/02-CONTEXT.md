# Phase 2: Core Sync Loop (MVP) - Context

**Gathered:** 2026-03-05
**Status:** Ready for planning

<domain>
## Phase Boundary

Users can scaffold a canonical `.ai/` directory, import existing configs with interactive merge, and forward-sync instructions to Claude Code, OpenCode, and Cursor with `aisync init`, `aisync sync`, and `aisync status`. No watch mode, no bidirectional sync, no memory/hooks — just one-shot forward sync.

Requirements: CLI-01, CLI-02, CLI-03, CLI-04, CLI-09, CLI-11, INST-01, INST-02, INST-03, INST-04, INST-05, INST-06, INST-07, INST-10, ADPT-01, ADPT-02, ADPT-03

</domain>

<decisions>
## Implementation Decisions

### Init workflow
- **Import strategy**: Interactive merge — when multiple existing configs exist (CLAUDE.md + .cursor/rules/), show a diff of all found configs side-by-side and let user pick sections from each
- **Scaffold contents**: Full scaffold — create `.ai/instructions.md`, `.ai/memory/`, `.ai/hooks/`, `.ai/commands/`, and `aisync.toml` even if subdirectories are empty. Ready for Phase 3+
- **Tool detection**: Auto-detect + confirm — run detection engine, show results ("Found: Claude Code, Cursor"), ask user to confirm before writing config
- **Re-init behavior**: Offer re-init — warn and ask "Re-initialize? This will overwrite aisync.toml and re-import instructions." Allows user to reset
- **No tools detected**: Proceed anyway — create `.ai/` with empty tool config. User can add tools later. Low friction

### Sync output shape
- **Symlink direction**: Tool file → .ai/ — CLAUDE.md and AGENTS.md are symlinks pointing to `.ai/instructions.md`. Canonical file is the real file, tool files are symlinks
- **Existing file handling**: Prompt interactively — when sync finds an existing non-symlink tool file, ask the user: "CLAUDE.md exists. Replace with symlink? [y/N]"
- **Cursor .mdc frontmatter**: Minimal — just `description` and `globs: '**'`, enough for Cursor to load the rule
- **.gitignore management**: Auto-add with marker — append a managed section (`# aisync-managed` ... `# /aisync-managed`) with entries for symlinked/generated files
- **Idempotency**: Running `aisync sync` twice produces identical results — symlinks verified, .mdc regenerated only if content changed

### Status reporting
- **Drift detection**: Content hash (SHA-256) — compare file contents regardless of filesystem. For symlinks: verify target + hash. For generated files: compare content hash
- **Default output**: Colored table — Tool | Strategy | Status (checkmark/X) | Details. Green/red for quick scanning
- **All-in-sync output**: Single summary line — "✓ All 3 tools in sync" when everything is clean. Full table only when drift detected
- **JSON flag**: Yes, from day one — `--json` outputs structured JSON for scripts and CI
- **Symlink validation**: Detect dangling symlinks and report in status

### Error handling
- **Partial sync failure**: Continue + report — sync all tools, collect errors, report failures at the end. Exit code reflects failures (non-zero if any tool failed)
- **--dry-run output**: Action list — one line per planned action: "Would create symlink: CLAUDE.md → .ai/instructions.md"
- **--verbose**: Claude's Discretion — pick whatever format is clearest for debugging

### Claude's Discretion
- Verbose output format (structured debug vs narrative)
- Interactive merge UI implementation details (dialoguer prompts, diff rendering)
- Exact .mdc template beyond minimal frontmatter
- Internal module organization for CLI commands
- Temp file handling during sync operations

</decisions>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AnyAdapter` enum dispatch (adapter.rs) — extend with read/write/sync methods for Phase 2
- `ToolAdapter` trait — add `read_instructions()`, `write_instructions()`, `sync_status()` methods
- `DetectionEngine::scan()` — reuse in `aisync init` for auto-detection
- `AisyncConfig` + `ToolConfig` (config.rs) — parse `aisync.toml` for sync strategy per tool
- `SyncStrategy` enum (Symlink/Copy/Generate) — drives per-tool sync behavior
- `DetectionResult` struct — reuse confidence/markers in status reporting

### Established Patterns
- Enum dispatch (`AnyAdapter`) over dyn Trait — continue for new adapter methods
- Per-adapter error enums + thiserror → top-level `AisyncError` — extend for sync/init errors
- Fixtures at `fixtures/` — add multi-tool test scenarios with existing configs

### Integration Points
- `aisync` binary crate (`crates/aisync/src/main.rs`) — needs clap CLI with init/sync/status subcommands
- `aisync-core` library — new modules for sync engine, init logic, status checker
- Adapter structs gain read/write methods that Phase 3+ will extend further

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 02-core-sync-loop-mvp*
*Context gathered: 2026-03-05*
