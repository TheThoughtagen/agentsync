# Phase 3: Memory and Hooks - Context

**Gathered:** 2026-03-05
**Status:** Ready for planning

<domain>
## Phase Boundary

Users can sync memory files and hook definitions across all Tier 1 tools (Claude Code, OpenCode, Cursor), with CLI subcommands for managing both. Forward sync only — reverse sync (bidirectional) is Phase 4. No watch mode, no nested/hierarchical instructions.

Requirements: MEM-01, MEM-02, MEM-03, MEM-04, MEM-05, MEM-06, MEM-07, HOOK-01, HOOK-02, HOOK-03, HOOK-04, HOOK-05, HOOK-06, HOOK-07

</domain>

<decisions>
## Implementation Decisions

### Memory file structure
- Plain markdown files in flat `.ai/memory/` directory
- No special schema or frontmatter — just human-readable `.md` files
- MEMORY.md is the index file loaded by default, topic files alongside it (debugging.md, patterns.md, etc.)
- `aisync memory add <topic>` creates `topic.md` with just `# Topic` as the first line — minimal header, no boilerplate

### Memory sync per tool
- Claude Code: symlink `.ai/memory/` into `~/.claude/projects/<hash>/memory/`
- OpenCode: append a managed reference block to AGENTS.md with relative paths from project root (e.g., "See also: .ai/memory/debugging.md")
- Cursor: append a managed reference block to `.cursor/rules/project.mdc` with relative paths

### Claude memory import
- Auto-detect Claude memory path by computing the same project-hash Claude uses (`~/.claude/projects/<hash>/memory/`)
- Prompt per conflict when `.ai/memory/` already has a file with the same name — show diff, user picks: keep aisync version, use Claude version, or merge manually
- Claude only for now — no `import` for other tools (OpenCode/Cursor don't have equivalent memory stores)
- One-time copy: import copies files into `.ai/memory/`, which then becomes the source of truth. Forward sync pushes TO tools. Reverse sync deferred to Phase 4

### Hook schema design
- Single `.ai/hooks.toml` file mirroring Claude Code's event model
- Events: PreToolUse, PostToolUse, Notification, Stop, SubagentStop (Claude Code's event set)
- Structure: TOML arrays of tables per event, each with matcher and hooks array
- Example:
  ```toml
  [[PreToolUse]]
  matcher = "Edit"
  hooks = [{ type = "command", command = "npm run lint", timeout = 10000 }]
  ```

### Hook CLI
- `aisync hooks add` — interactive builder that prompts for event type, matcher, command, timeout. Builds valid TOML and appends to `.ai/hooks.toml`
- `aisync hooks list` — shows all hooks AND all configured tools with support status (checkmark/X per tool)
- `aisync hooks translate` — shows translated output for ALL tools at once (Claude JSON, OpenCode stub, Cursor warning)

### Unsupported features
- Warning in sync output for unsupported feature/tool combos: yellow warning line (e.g., "Cursor: hooks not supported (skipped)")
- Non-zero exit only on actual errors, not warnings
- `aisync hooks list` shows all tools with support status including unsupported ones
- Memory references for OpenCode/Cursor use relative paths from project root

### Status extension
- `aisync status` extended in Phase 3 to show memory sync state and hook translation state per tool alongside instructions sync
- Full picture in one command

### Claude's Discretion
- Claude memory path hash algorithm implementation details
- Memory reference block formatting (managed section markers consistent with Phase 2 gitignore pattern)
- Hook translation to OpenCode plugin stub format
- Interactive builder UX details (dialoguer prompts)
- Exact warning message wording and coloring

</decisions>

<specifics>
## Specific Ideas

- Hook schema mirrors Claude Code's native format for near-1:1 translation; other tools do the heavier mapping
- Memory import conflict resolution follows same interactive pattern as init import from Phase 2

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ToolAdapter` trait (adapter.rs) — add `sync_memory()` and `translate_hook()` methods as planned in Phase 1
- `AnyAdapter` enum dispatch — add match arms for new methods across all three adapters
- `SyncEngine` (sync.rs) — extend `plan()` and `execute()` to handle memory sync actions alongside instructions
- `SyncAction` enum (types.rs) — add new variants for memory symlink/reference and hook translation
- `update_managed_section()` (gitignore.rs) — reuse marker-based managed section pattern for memory references in AGENTS.md and .mdc
- `InitEngine` (init.rs) — `.ai/memory/` and `.ai/hooks/` directories already scaffolded
- `content_hash()` (types.rs) — reuse for memory file drift detection in status

### Established Patterns
- Enum dispatch (`AnyAdapter`) over dyn Trait — continue for sync_memory/translate_hook
- Per-adapter error enums + thiserror -> top-level `AisyncError` — extend for memory/hook errors
- Marker-based managed sections (gitignore.rs) — reuse for memory reference blocks
- Continue-and-report on partial failure — same pattern for memory/hook sync failures
- Interactive prompting in CLI layer, core library only discovers and executes (Phase 2 decision)

### Integration Points
- `ToolAdapter` trait gains `sync_memory()` and `translate_hook()` methods
- `AnyAdapter` needs new match arms for memory/hook dispatch
- `SyncEngine::plan()` and `execute()` extended for memory actions
- New CLI subcommands: `aisync memory {list,add,import}`, `aisync hooks {list,add,translate}`
- `StatusReport` extended with memory and hook sync fields
- `.ai/hooks.toml` — new config file alongside existing `aisync.toml`

</code_context>

<deferred>
## Deferred Ideas

- Nested/hierarchical instructions (CLAUDE.md at multiple directory levels synced across tools) — new capability, possibly Phase 4 or later
- Ongoing reverse sync for Claude memory (Claude auto-memory edits flow back to .ai/) — Phase 4 bidirectional sync
- Memory import for tools other than Claude — no equivalent memory stores exist yet

</deferred>

---

*Phase: 03-memory-and-hooks*
*Context gathered: 2026-03-05*
