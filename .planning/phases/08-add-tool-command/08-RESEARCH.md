# Phase 8: Add-Tool Command - Research

**Researched:** 2026-03-08
**Domain:** CLI command design, config mutation, partial sync
**Confidence:** HIGH

## Summary

Phase 8 adds an `aisync add-tool` command that lets users adopt new AI tools mid-project without manual config editing or full re-initialization. The implementation sits cleanly on top of existing infrastructure: `DetectionEngine::scan()` already discovers all built-in tools, `ToolsConfig::set_tool()` already mutates the config map, `AisyncConfig::to_string_pretty()` already serializes to TOML, and `SyncEngine::plan()`/`execute()` already handle per-tool sync. The main new work is: (1) a core-layer `AddToolEngine` that computes the delta between detected tools and configured tools, (2) a CLI command with `dialoguer::MultiSelect` for interactive selection, and (3) a partial-sync path that plans/executes only for the newly added tools.

The codebase already uses `dialoguer` 0.12 (Confirm, Select, Input) so MultiSelect is available without new dependencies. The `SyncEngine::enabled_tools()` method iterates all built-in adapters filtered by config -- the partial sync just needs a filtered variant that accepts a specific set of `ToolKind` values.

**Primary recommendation:** Add `AddToolEngine` to `aisync-core` with `discover_unconfigured()` and `add_tools()` methods, then wire a new `AddTool` CLI subcommand using existing patterns from `init.rs`.

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TOOL-01 | `aisync add-tool` auto-detects tools not yet configured in aisync.toml | `DetectionEngine::scan()` already returns all detected tools; diff against `ToolsConfig::configured_tools()` to find unconfigured ones |
| TOOL-02 | User interactively selects which detected tools to add | `dialoguer::MultiSelect` (already in workspace deps) provides checkbox-style selection |
| TOOL-03 | Selected tools are added to aisync.toml and synced immediately | `ToolsConfig::set_tool()` + `AisyncConfig::to_string_pretty()` + `std::fs::write()` for config update; then `SyncEngine::plan()`/`execute()` for sync |
| TOOL-04 | Partial sync runs only for newly added tools (not full re-sync) | New `SyncEngine::plan_for_tools()` method that filters `enabled_tools()` to a specific set, reusing all existing per-tool logic |

</phase_requirements>

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| dialoguer | 0.12 | Interactive multi-select prompts | Already in workspace, used by init/hooks/sync commands |
| clap | workspace | CLI subcommand definition | Already used for all commands |
| toml | workspace | Config serialization/deserialization | Already used by AisyncConfig |
| colored | workspace | Terminal output formatting | Already used by all CLI commands |

### Supporting

No new dependencies needed. Everything required is already in the workspace.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| dialoguer::MultiSelect | Plain numbered list + manual input | MultiSelect is better UX, already available |
| Core-layer AddToolEngine | All logic in CLI command | Violates existing core/CLI separation pattern |
| Partial sync via filtered plan | Re-run full sync after config update | Full sync works but violates TOOL-04 requirement |

## Architecture Patterns

### Recommended Project Structure

```
crates/aisync-core/src/
  add_tool.rs         # AddToolEngine (discover_unconfigured, add_tools)
  sync.rs             # Add plan_for_tools() method

crates/aisync/src/commands/
  add_tool.rs          # CLI handler with interactive prompts
  mod.rs               # Add pub mod add_tool

crates/aisync/src/main.rs  # Add AddTool variant to Commands enum
```

### Pattern 1: Core Engine + CLI Handler (existing pattern)

**What:** Business logic in `aisync-core` engine struct, user interaction in `aisync/commands/` handler.
**When to use:** Every command follows this pattern. InitEngine/SyncEngine/MemoryEngine are precedents.
**Example:**

```rust
// crates/aisync-core/src/add_tool.rs
pub struct AddToolEngine;

impl AddToolEngine {
    /// Discover tools detected on disk but not configured in aisync.toml.
    pub fn discover_unconfigured(
        config: &AisyncConfig,
        project_root: &Path,
    ) -> Result<Vec<DetectionResult>, AisyncError> {
        let detected = DetectionEngine::scan(project_root)?;
        let unconfigured: Vec<DetectionResult> = detected
            .into_iter()
            .filter(|d| config.tools.get_tool(d.tool.as_str()).is_none())
            .collect();
        Ok(unconfigured)
    }

    /// Add tools to config and write updated aisync.toml.
    pub fn add_tools(
        config: &mut AisyncConfig,
        tools: &[ToolKind],
        project_root: &Path,
    ) -> Result<(), AisyncError> {
        for tool in tools {
            let adapter = AnyAdapter::for_tool(tool);
            let strategy = adapter.as_ref().map(|a| a.default_sync_strategy());
            let tool_config = ToolConfig {
                enabled: true,
                sync_strategy: strategy.filter(|s| *s != SyncStrategy::Symlink),
            };
            config.tools.set_tool(tool.as_str().to_string(), tool_config);
        }
        let toml_str = config.to_string_pretty()
            .map_err(|e| InitError::ImportFailed(format!("serialize: {e}")))?;
        std::fs::write(project_root.join("aisync.toml"), toml_str)
            .map_err(InitError::ScaffoldFailed)?;
        Ok(())
    }
}
```

### Pattern 2: Partial Sync via Filtered Plan

**What:** `SyncEngine::plan_for_tools()` that accepts a filter set, reuses all existing per-tool plan logic.
**When to use:** When only specific tools need syncing (add-tool, future selective sync).
**Example:**

```rust
// In SyncEngine
pub fn plan_for_tools(
    config: &AisyncConfig,
    project_root: &Path,
    only_tools: &[ToolKind],
) -> Result<SyncReport, AisyncError> {
    // Same as plan(), but enabled_tools() is further filtered to only_tools
    // Reuses canonical content loading, conditional processing, deduplication
}
```

### Pattern 3: Non-interactive Fallback

**What:** When stdin is not a terminal, either show available tools and exit, or accept `--tool` flag.
**When to use:** CI/scripting environments. Follows existing pattern in `init.rs`.
**Example:**

```rust
// CLI handler
if !interactive {
    // List unconfigured tools and exit with hint
    for result in &unconfigured {
        println!("  {}", result.tool.display_name());
    }
    eprintln!("Use `aisync add-tool --tool <name>` in non-interactive mode.");
    return Ok(());
}
```

### Anti-Patterns to Avoid

- **Modifying aisync.toml by string manipulation:** Always deserialize, mutate, re-serialize. The `AisyncConfig` round-trip is already tested.
- **Running full sync after add-tool:** Violates TOOL-04. Only sync newly added tools.
- **Duplicating sync logic:** Reuse `SyncEngine::plan()`/`execute()` internals, don't copy them.
- **Ignoring deduplication:** The partial sync must still run through `deduplicate_actions()` since the new tool may share paths with existing tools (e.g., Codex + OpenCode both target AGENTS.md).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Interactive multi-select | Custom terminal input loop | `dialoguer::MultiSelect` | Handles arrow keys, space toggle, enter confirm, terminal state |
| TOML config mutation | String find-and-replace on TOML | `AisyncConfig` serde round-trip | Already tested, handles edge cases |
| Tool detection | Separate detection for add-tool | `DetectionEngine::scan()` | Already handles all built-in tools with confidence levels |
| Sync planning | Custom file operations | `SyncEngine::plan_for_tools()` filtering to `SyncEngine::plan()` | Handles conditionals, memory, hooks, deduplication |

**Key insight:** 90% of the infrastructure already exists. The add-tool command is primarily a composition of existing capabilities with a new discovery step and filtered sync path.

## Common Pitfalls

### Pitfall 1: Config Serialization Order Instability

**What goes wrong:** Adding a tool and re-serializing aisync.toml could reorder existing sections or change formatting.
**Why it happens:** `toml::to_string_pretty()` serializes BTreeMap in alphabetical order, which is stable but may differ from the user's manual ordering.
**How to avoid:** `ToolsConfig` uses `BTreeMap` which is already alphabetically ordered. This is consistent. Document that tool sections will be alphabetized.
**Warning signs:** User complaints about reformatted config files.

### Pitfall 2: Unconfigured-Is-Enabled Semantics Confusion

**What goes wrong:** A tool not in aisync.toml is treated as "enabled" by `is_enabled()`, so `discover_unconfigured()` must check `get_tool().is_none()`, not `!is_enabled()`.
**Why it happens:** The "unconfigured-is-enabled" design (from Phase 6 decisions) means absent tools pass through sync.
**How to avoid:** Use `config.tools.get_tool(name).is_none()` as the unconfigured check, never `!is_enabled()`.
**Warning signs:** Already-syncing tools appearing as "available to add".

### Pitfall 3: Deduplication with Partial Sync

**What goes wrong:** A newly added tool (e.g., Codex) targets AGENTS.md which is already managed by OpenCode. The partial sync creates a duplicate symlink or conflicts.
**Why it happens:** `deduplicate_actions()` only deduplicates within a single `SyncReport`. If existing tools aren't in the report, there's no deduplication.
**How to avoid:** The partial sync `plan_for_tools()` should still include existing tools' claimed paths in deduplication, or check filesystem state. Simplest: include existing configured tools in the plan but only execute actions for new tools.
**Warning signs:** Conflicting symlinks or "file already exists" errors on add-tool.

### Pitfall 4: Missing aisync.toml or .ai/ Directory

**What goes wrong:** User runs `add-tool` before `init`.
**Why it happens:** Normal user error.
**How to avoid:** Check for `aisync.toml` existence at start of command (same as `sync` command pattern). Return clear error: "Run `aisync init` first."
**Warning signs:** Confusing "file not found" errors.

### Pitfall 5: Tool Already Configured

**What goes wrong:** All detected tools are already in aisync.toml, leaving nothing to add.
**Why it happens:** User runs add-tool when they don't need to.
**How to avoid:** When `discover_unconfigured()` returns empty, print friendly "All detected tools are already configured" message and exit cleanly.
**Warning signs:** Empty multi-select list confusing users.

## Code Examples

### Multi-Select with dialoguer

```rust
// dialoguer::MultiSelect for tool selection (TOOL-02)
use dialoguer::MultiSelect;

let items: Vec<String> = unconfigured
    .iter()
    .map(|d| format!("{} ({:?})", d.tool.display_name(), d.confidence))
    .collect();

let selections = MultiSelect::new()
    .with_prompt("Select tools to add")
    .items(&items)
    .interact()?;

let selected_tools: Vec<ToolKind> = selections
    .iter()
    .map(|&i| unconfigured[i].tool.clone())
    .collect();
```

### Config Mutation Pattern

```rust
// Already supported by existing API
let mut config = AisyncConfig::from_file(Path::new("aisync.toml"))?;
config.tools.set_tool("windsurf".to_string(), ToolConfig {
    enabled: true,
    sync_strategy: Some(SyncStrategy::Copy),
});
let toml = config.to_string_pretty()?;
std::fs::write("aisync.toml", toml)?;
```

### Partial Sync Filtering

```rust
// Filter enabled_tools to only the newly added set
fn plan_for_tools(
    config: &AisyncConfig,
    project_root: &Path,
    only_tools: &[ToolKind],
) -> Result<SyncReport, AisyncError> {
    // Reuse plan() but filter to only_tools
    let all = Self::enabled_tools(config);
    let filtered: Vec<_> = all
        .into_iter()
        .filter(|(kind, _, _)| only_tools.contains(kind))
        .collect();
    // ... rest follows plan() exactly
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual aisync.toml editing | `add-tool` command with auto-detect | This phase | Users no longer need to know TOML section names or sync strategies |
| Full re-init to add tools | Partial config update + selective sync | This phase | Preserves existing config, only syncs what's new |

## Open Questions

1. **Should `add-tool` also support adding tools NOT detected on disk?**
   - What we know: Requirements say "auto-detects tools not yet configured" (TOOL-01)
   - What's unclear: Should there be a `--tool windsurf` flag to add a tool even if its markers aren't detected yet?
   - Recommendation: Support `--tool <name>` as escape hatch for non-interactive mode and for pre-configuring tools. Detection is the primary path but not the only one. This also satisfies non-interactive use.

2. **Should disabled tools (enabled = false) appear in the unconfigured list?**
   - What we know: A tool with `enabled = false` is explicitly configured but disabled
   - What's unclear: Should `add-tool` offer to re-enable disabled tools?
   - Recommendation: No. `discover_unconfigured()` should only return tools with no config entry at all. Re-enabling is a manual config edit or a future `aisync enable-tool` command.

3. **Deduplication strategy for partial sync**
   - What we know: Codex and OpenCode both target AGENTS.md; `deduplicate_actions()` handles this within a full sync
   - What's unclear: How to handle deduplication when only syncing new tools
   - Recommendation: `plan_for_tools()` should plan all enabled tools (to get full deduplication) but then filter the final `SyncReport.results` to only include the new tools' results before executing. This is the simplest correct approach.

## Sources

### Primary (HIGH confidence)

- Codebase analysis of `crates/aisync-core/src/` -- adapter.rs, config.rs, detection.rs, init.rs, sync.rs, types.rs, error.rs, lib.rs
- Codebase analysis of `crates/aisync/src/` -- main.rs, commands/init.rs, commands/sync.rs, commands/mod.rs
- Cargo.toml workspace dependencies -- dialoguer 0.12, clap, toml, colored

### Secondary (MEDIUM confidence)

- dialoguer crate MultiSelect API -- based on training data for dialoguer 0.12; API is stable and consistent with observed Select/Confirm usage in codebase

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all dependencies already in workspace, no new crates needed
- Architecture: HIGH -- follows established core/CLI separation pattern with clear precedents
- Pitfalls: HIGH -- derived from direct codebase analysis of existing semantics (unconfigured-is-enabled, deduplication, TOML round-trip)

**Research date:** 2026-03-08
**Valid until:** 2026-04-08 (stable -- internal codebase patterns, no external API dependencies)
