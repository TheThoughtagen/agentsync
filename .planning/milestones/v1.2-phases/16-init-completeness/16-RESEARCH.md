# Phase 16: Init Completeness - Research

**Researched:** 2026-03-09
**Domain:** Rust CLI init/status/sync workflow integration
**Confidence:** HIGH

## Summary

Phase 16 addresses four distinct bugs/gaps in the init-to-status workflow. After extensive code review, all four issues are well-understood with clear root causes and surgical fixes.

**INIT-01** (zero drift after init): `run_init()` in `crates/aisync/src/commands/init.rs` calls `InitEngine::scaffold()` which creates `.ai/` structure and `aisync.toml` but never calls `SyncEngine::plan/execute`. The fix is to run a sync at the end of init.

**INIT-02** (ghost tools): `ToolsConfig::is_enabled()` in `crates/aisync-core/src/config.rs:51` returns `true` for tools NOT in the config (`is_none_or(|tc| tc.enabled)`). This means all 5 builtin adapters appear in status even if only 2 are configured in `aisync.toml`. The fix is to change `is_enabled` to only return true for tools explicitly listed in the config.

**INIT-03** (interactive source selection): Already partially implemented in `resolve_import()` in `crates/aisync/src/commands/init.rs:136-220`. The current code handles instruction source selection but does NOT handle tool-level source selection (e.g., "which tool's rules to import from"). The requirement says "source tool selection" -- the existing multi-source picker with `dialoguer::Select` already satisfies this for instructions. Need to verify if the requirement extends to rules/MCP/commands source selection too.

**INIT-04** (action messages): `SyncAction`'s `Display` impl in `crates/aisync-types/src/lib.rs:306-434` uses "Would create" phrasing for ALL variants. This is correct for dry-run but wrong during actual sync. The fix is to either change Display to use present tense, or use a different formatting path in the sync command's `print_results`.

**Primary recommendation:** Fix all four issues with targeted changes across 3-4 files. No new dependencies needed.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| INIT-01 | `aisync init` completes with zero drift -- `aisync status` shows all tools OK immediately after init | Root cause: `run_init()` never calls sync after scaffold. Fix: call `SyncEngine::plan` + `SyncEngine::execute` at end of init |
| INIT-02 | `aisync status` only shows tools configured in `aisync.toml` or detected | Root cause: `is_enabled()` returns true for unconfigured tools. Fix: change to require explicit presence in config |
| INIT-03 | Interactive source tool selection when multiple instruction sources exist | Already implemented in `resolve_import()` with `dialoguer::Select`. May need extension for rules/MCP if desired |
| INIT-04 | `aisync sync` output uses correct messages (no "Would create" during real sync) | Root cause: `SyncAction::Display` uses "Would" prefix. Fix: change Display to present tense, use dry-run wrapper for "Would" prefix |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| dialoguer | (existing) | Interactive prompts (Select, Confirm) | Already used in init.rs for user prompts |
| colored | (existing) | Terminal output coloring | Already used throughout CLI commands |
| aisync-types | workspace | SyncAction Display impl | Where INIT-04 fix lives |
| aisync-core | workspace | InitEngine, SyncEngine, config | Where INIT-01 and INIT-02 fixes live |

### Supporting
No new libraries needed. All fixes use existing workspace crates.

## Architecture Patterns

### Pattern 1: Init-then-Sync
**What:** After scaffolding `.ai/` and writing `aisync.toml`, run a full sync cycle
**When to use:** INIT-01 fix
**Implementation approach:**

```rust
// In crates/aisync/src/commands/init.rs, after InitEngine::scaffold():
// 1. Load the config we just wrote
let config = AisyncConfig::from_file(&project_root.join("aisync.toml"))?;
// 2. Plan and execute sync
let planned = SyncEngine::plan(&config, &project_root)?;
let result = SyncEngine::execute(&planned, &project_root)?;
```

Key consideration: The sync should be non-interactive (no SkipExistingFile prompts) since init just created everything fresh. The SkipExistingFile case shouldn't arise because init creates symlinks/generates files from scratch.

### Pattern 2: Explicit Tool Enablement
**What:** Change `is_enabled` to require tools be explicitly listed in config
**When to use:** INIT-02 fix
**Implementation approach:**

```rust
// In crates/aisync-core/src/config.rs
pub fn is_enabled(&self, name: &str) -> bool {
    self.tools.get(name).is_some_and(|tc| tc.enabled)
}
```

This changes from "enabled unless explicitly disabled" to "only enabled if explicitly listed". The `build_config` in init.rs already adds detected tools to `aisync.toml`, so after init only detected tools appear.

**Impact analysis:** This changes behavior for ALL commands (sync, status, diff, watch). Any tool not in `aisync.toml` will be excluded. This is the correct behavior per the requirement, but need to verify `add-tool` still works (it does -- it adds tools to config before syncing).

### Pattern 3: Action Message Context
**What:** SyncAction Display should use present/past tense ("Created", "Synced") not future tense ("Would create")
**When to use:** INIT-04 fix
**Two approaches:**

**Approach A (recommended): Change Display to present tense, wrap for dry-run.**
Change `SyncAction::Display` to use action-oriented language ("Create symlink", "Generate MDC file"). In `print_dry_run`, prefix with "Would: " or use a dedicated dry-run formatter.

**Approach B: Add a display method that takes a `dry_run: bool` parameter.**
Less clean since Display trait doesn't accept params, would need a wrapper struct or separate method.

### Anti-Patterns to Avoid
- **Running sync in init before aisync.toml is written:** The config must exist before SyncEngine can plan
- **Changing is_enabled without updating build_config:** Init must still add all detected tools to config
- **Using Display for both dry-run and execution output:** These need different messaging

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Interactive selection | Custom menu | `dialoguer::Select` | Already used, handles edge cases |
| Config serialization | Manual TOML writing | `toml::to_string_pretty` | Already used in build_config |

## Common Pitfalls

### Pitfall 1: is_enabled backward compatibility
**What goes wrong:** Changing `is_enabled` to require explicit listing breaks projects with existing aisync.toml that don't list all their tools
**Why it happens:** Old configs may not have all tools listed -- they relied on the "enabled by default" behavior
**How to avoid:** The `aisync init` and `aisync add-tool` commands already add tools explicitly to aisync.toml. Any project that ran init will have tools listed. Projects that manually created aisync.toml with only `schema_version = 1` would lose all tool sync, but this is the correct behavior (no tools configured = no tools synced).
**Warning signs:** Integration tests that create minimal aisync.toml without tool entries

### Pitfall 2: Init sync creating SkipExistingFile actions
**What goes wrong:** If a tool already has a native instruction file (e.g., CLAUDE.md exists), init's auto-sync might encounter SkipExistingFile
**Why it happens:** The plan_sync method checks if native files exist and were not created by aisync
**How to avoid:** Init already imports content into `.ai/instructions.md`. The sync after init should use the same strategy as a normal sync. For symlink tools, if the native file exists as a regular file, the user was already prompted about this during the import step. Consider using force mode or handling SkipExistingFile as RemoveAndRelink during init's auto-sync.

### Pitfall 3: Display impl used in tests
**What goes wrong:** Changing SyncAction::Display may break test assertions that match on "Would create"
**Why it happens:** Tests may assert on Display output strings
**How to avoid:** Search for "Would create" and "Would " in test files before changing. Update test assertions accordingly.

### Pitfall 4: Sync errors during init
**What goes wrong:** If the auto-sync fails (e.g., permission error creating symlink), init appears to fail even though scaffolding succeeded
**Why it happens:** sync errors propagate up
**How to avoid:** Catch sync errors gracefully during init -- warn but don't fail init. The user can always run `aisync sync` manually afterward.

## Code Examples

### Current is_enabled (buggy for INIT-02)
```rust
// crates/aisync-core/src/config.rs:51
pub fn is_enabled(&self, name: &str) -> bool {
    self.tools.get(name).is_none_or(|tc| tc.enabled)
    // Returns true for tools NOT in config!
}
```

### Current Display (buggy for INIT-04)
```rust
// crates/aisync-types/src/lib.rs:309-316
SyncAction::CreateSymlink { link, target } => {
    write!(f, "Would create symlink: {} -> {}", link.display(), target.display())
}
```

### Current init flow (missing sync for INIT-01)
```rust
// crates/aisync/src/commands/init.rs:96-101
InitEngine::scaffold(&project_root, &detected, import_content.as_deref(), &options)?;
// No SyncEngine call follows -- init ends with print messages
```

### Files to modify
1. `crates/aisync-core/src/config.rs` -- `is_enabled()` method (INIT-02)
2. `crates/aisync/src/commands/init.rs` -- add sync after scaffold (INIT-01)
3. `crates/aisync-types/src/lib.rs` -- `Display for SyncAction` (INIT-04)
4. `crates/aisync/src/commands/sync.rs` -- `print_dry_run` to add "Would" prefix (INIT-04)

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `is_none_or` (default enabled) | Should be `is_some_and` (explicit only) | Phase 16 | Ghost tools removed from status |
| Init without sync | Init + auto-sync | Phase 16 | Zero drift after init |
| "Would create" always | Context-appropriate messages | Phase 16 | Clear sync output |

## Open Questions

1. **INIT-03 scope: Does "interactive source tool selection" extend beyond instructions?**
   - What we know: `resolve_import()` already handles interactive selection for instruction sources with `dialoguer::Select`
   - What's unclear: The requirement says "when multiple instruction sources exist" -- this is already implemented. Does it also mean choosing which tool's rules/MCP/commands to prioritize?
   - Recommendation: The current implementation satisfies the literal requirement. Rules/MCP/commands import from ALL detected sources with first-seen-wins merge. No additional interactive prompt needed unless explicitly requested.

2. **Should init auto-sync handle SkipExistingFile?**
   - What we know: Init creates `.ai/` fresh, so symlink targets should be new. But native files (CLAUDE.md, .cursor/rules/project.mdc) may pre-exist.
   - What's unclear: Should init's auto-sync force-replace existing native files?
   - Recommendation: For init specifically, treat SkipExistingFile as RemoveAndRelink since the user already chose to initialize. This ensures zero drift.

## Sources

### Primary (HIGH confidence)
- Direct code review of `crates/aisync/src/commands/init.rs` -- init workflow
- Direct code review of `crates/aisync-core/src/config.rs:51` -- is_enabled bug
- Direct code review of `crates/aisync-types/src/lib.rs:306-434` -- Display impl
- Direct code review of `crates/aisync-core/src/sync.rs` -- SyncEngine::status and enabled_tools

### Secondary (MEDIUM confidence)
- Integration tests in `crates/aisync/tests/integration/` -- existing test patterns

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies, all fixes in existing code
- Architecture: HIGH - root causes clearly identified in code review
- Pitfalls: HIGH - based on direct analysis of current behavior and test structure

**Research date:** 2026-03-09
**Valid until:** 2026-04-09 (stable Rust codebase, no external dependencies changing)
