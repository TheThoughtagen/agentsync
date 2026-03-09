# Phase 3: Memory and Hooks - Research

**Researched:** 2026-03-05
**Domain:** Memory file sync, hook translation, CLI subcommands (Rust)
**Confidence:** HIGH

## Summary

Phase 3 extends the existing sync engine to handle two new content types: memory files (`.ai/memory/`) and hook definitions (`.ai/hooks.toml`). Memory sync requires different strategies per tool -- Claude Code gets a directory symlink, while OpenCode and Cursor get managed reference blocks appended to their instruction files. Hook translation maps a single TOML schema to Claude Code's JSON format and OpenCode's plugin stubs, with explicit warnings for Cursor (unsupported).

The codebase is well-structured for this extension. The `ToolAdapter` trait needs `sync_memory()` and `translate_hook()` methods. The `SyncAction` enum needs new variants for memory symlink/references and hook translation. The managed section pattern from `gitignore.rs` is directly reusable for injecting memory references into AGENTS.md and `.cursor/rules/project.mdc`. The CLI needs two new subcommand groups (`memory` and `hooks`) following the existing clap pattern.

**Primary recommendation:** Extend the existing adapter/sync architecture incrementally. Memory sync first (simpler, builds on existing symlink/managed-section patterns), then hooks (new TOML schema, translation logic, CLI interactive builder).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Plain markdown files in flat `.ai/memory/` directory, no special schema or frontmatter
- MEMORY.md is the index file, topic files alongside it (debugging.md, patterns.md, etc.)
- `aisync memory add <topic>` creates `topic.md` with just `# Topic` as first line
- Claude Code memory sync: symlink `.ai/memory/` into `~/.claude/projects/<hash>/memory/`
- OpenCode memory sync: append managed reference block to AGENTS.md with relative paths
- Cursor memory sync: append managed reference block to `.cursor/rules/project.mdc` with relative paths
- Claude memory import: auto-detect Claude memory path, prompt per conflict, Claude only
- Single `.ai/hooks.toml` file mirroring Claude Code's event model
- Hook events: PreToolUse, PostToolUse, Notification, Stop, SubagentStop
- TOML arrays of tables per event with matcher and hooks array
- `aisync hooks add` -- interactive builder with dialoguer
- `aisync hooks list` -- shows all hooks AND all configured tools with support status
- `aisync hooks translate` -- shows translated output for ALL tools at once
- Warning in sync output for unsupported feature/tool combos (yellow, non-zero exit only on errors)
- `aisync status` extended to show memory sync state and hook translation state

### Claude's Discretion
- Claude memory path hash algorithm implementation details
- Memory reference block formatting (managed section markers consistent with Phase 2 gitignore pattern)
- Hook translation to OpenCode plugin stub format
- Interactive builder UX details (dialoguer prompts)
- Exact warning message wording and coloring

### Deferred Ideas (OUT OF SCOPE)
- Nested/hierarchical instructions
- Ongoing reverse sync for Claude memory (Phase 4)
- Memory import for tools other than Claude
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| MEM-01 | `.ai/memory/` files synced to Claude Code auto-memory path via symlink | Claude Code stores memory at `~/.claude/projects/<project-key>/memory/` where project-key is the absolute path with `/` replaced by `-`. Symlink the entire `.ai/memory/` directory there. |
| MEM-02 | Memory file references injected into AGENTS.md for OpenCode | Reuse `update_managed_section()` pattern from gitignore.rs with memory-specific markers. Append relative paths like `.ai/memory/debugging.md`. |
| MEM-03 | Memory file references injected into .mdc rules for Cursor | Same managed section pattern, appended to `.cursor/rules/project.mdc` after frontmatter+instructions content. |
| MEM-04 | `aisync memory list` shows all memory files | Scan `.ai/memory/` directory, list `.md` files with first-line heading extraction. |
| MEM-05 | `aisync memory add <topic>` creates new memory file | Create `.ai/memory/<topic>.md` with `# <Topic>` header. Validate no duplicate, sanitize filename. |
| MEM-06 | `aisync memory import claude` pulls Claude auto-memory into `.ai/memory/` | Compute Claude project key from cwd, read files from `~/.claude/projects/<key>/memory/`, copy with conflict prompting. |
| MEM-07 | `aisync memory export` writes memory to all configured tools | Trigger memory sync for all enabled tools (same as what `aisync sync` does for memory, but explicit). |
| HOOK-01 | Canonical hook definitions in `.ai/hooks.toml` with tool-agnostic schema | Single TOML file using Claude Code event model. TOML arrays of tables per event. |
| HOOK-02 | Hook translation to Claude Code `.claude/settings.json` format | Generate JSON matching Claude Code's `{ "hooks": { "PreToolUse": [{ "matcher": "...", "hooks": [{ "type": "command", "command": "...", "timeout": N }] }] } }` schema. |
| HOOK-03 | Hook translation to OpenCode `opencode.json` plugin stubs | Map events to OpenCode plugin hooks: PreToolUse -> `tool.execute.before`, PostToolUse -> `tool.execute.after`, Stop -> `session.idle`, etc. Generate JS/TS stub file. |
| HOOK-04 | `aisync hooks list` shows all hooks and per-tool translations | Parse `.ai/hooks.toml`, display table with event, matcher, command, and per-tool support status. |
| HOOK-05 | `aisync hooks add` creates canonical hook definition | Interactive builder: prompt for event type, matcher, command, timeout using dialoguer. Append to `.ai/hooks.toml`. |
| HOOK-06 | `aisync hooks translate` previews each tool's version | Show Claude Code JSON, OpenCode stub, and Cursor warning side by side. |
| HOOK-07 | Warning surfaced for tools that don't support hooks (Cursor) | Yellow warning line in sync output. Non-zero exit only on actual errors. |
</phase_requirements>

## Standard Stack

### Core (already in workspace)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| toml | 0.8 | Parse/write `.ai/hooks.toml` | Already in workspace, handles TOML round-trip |
| serde + serde_json | 1.0 | Serialize hook translations to Claude Code JSON | Already in workspace |
| clap | 4.5 (derive) | CLI subcommands for memory and hooks | Already in workspace |
| dialoguer | 0.12 | Interactive prompts for `hooks add` and `memory import` conflict resolution | Already in workspace |
| colored | 3.1 | Warning messages (yellow), success (green) | Already in workspace |
| sha2 + hex | 0.10, 0.4 | Content hashing for memory drift detection in status | Already in workspace |
| thiserror | 2.0 | Error enum variants for memory/hook errors | Already in workspace |

### No New Dependencies Needed
The existing workspace dependencies cover all Phase 3 needs. TOML parsing uses `toml 0.8`. JSON generation uses `serde_json`. Interactive prompts use `dialoguer`. No new crates required.

## Architecture Patterns

### Recommended Project Structure
```
crates/aisync-core/src/
  adapter.rs          # Add sync_memory() and translate_hook() to ToolAdapter trait
  adapters/
    claude_code.rs    # Implement memory symlink + hook JSON translation
    opencode.rs       # Implement memory references + hook plugin stubs
    cursor.rs         # Implement memory references + hook warning
  types.rs            # New SyncAction variants, HookDef types, MemoryStatus
  sync.rs             # Extend plan()/execute()/status() for memory + hooks
  memory.rs           # NEW: MemoryEngine (list, add, import, export)
  hooks.rs            # NEW: HookEngine (parse, translate, list, add)
  error.rs            # New MemoryError and HookError variants
  gitignore.rs        # Generalize managed section for reuse (or extract to managed_section.rs)
  lib.rs              # Export new modules

crates/aisync/src/
  commands/
    memory.rs         # NEW: memory list/add/import/export subcommands
    hooks.rs          # NEW: hooks list/add/translate subcommands
    mod.rs            # Register new command modules
  main.rs             # Add Memory and Hooks to Commands enum
```

### Pattern 1: Extend ToolAdapter Trait with Default Impls
**What:** Add `sync_memory()` and `translate_hook()` methods with `todo!()` defaults, same pattern used in Phase 2.
**When to use:** When adding new capability to all adapters incrementally.
**Example:**
```rust
pub trait ToolAdapter {
    // ... existing methods ...

    /// Plan memory sync actions for this tool.
    fn plan_memory_sync(
        &self,
        project_root: &Path,
        memory_files: &[PathBuf],
    ) -> Result<Vec<SyncAction>, AisyncError> {
        let _ = (project_root, memory_files);
        Ok(vec![]) // Default: no memory sync
    }

    /// Translate hooks to this tool's native format.
    fn translate_hooks(
        &self,
        hooks: &[HookDef],
    ) -> Result<HookTranslation, AisyncError> {
        let _ = hooks;
        Ok(HookTranslation::Unsupported {
            reason: "hooks not supported".into(),
        })
    }
}
```

### Pattern 2: Managed Section Reuse for Memory References
**What:** Generalize the marker-based managed section pattern from gitignore.rs for memory reference blocks.
**When to use:** When injecting aisync-managed content into AGENTS.md and .mdc files.
**Example:**
```rust
// Use different markers for memory sections vs gitignore sections
pub const MEMORY_MARKER_START: &str = "<!-- aisync:memory -->";
pub const MEMORY_MARKER_END: &str = "<!-- /aisync:memory -->";

// Content between markers:
// <!-- aisync:memory -->
// ## Memory Files
// - [debugging](.ai/memory/debugging.md)
// - [patterns](.ai/memory/patterns.md)
// <!-- /aisync:memory -->
```

### Pattern 3: New SyncAction Variants
**What:** Extend the SyncAction enum for memory and hook operations.
**When to use:** For plan/execute lifecycle consistency.
**Example:**
```rust
pub enum SyncAction {
    // ... existing variants ...

    // Memory actions
    CreateMemorySymlink { link: PathBuf, target: PathBuf },
    UpdateMemoryReferences { path: PathBuf, references: Vec<String> },

    // Hook actions
    WriteHookTranslation { path: PathBuf, content: String, tool: ToolKind },
    WarnUnsupportedHooks { tool: ToolKind, reason: String },
}
```

### Pattern 4: Claude Code Project Key Derivation
**What:** Compute the Claude Code project key from the absolute project root path.
**When to use:** For MEM-01 (symlink target) and MEM-06 (import source).
**Example:**
```rust
/// Compute the Claude Code project key from an absolute path.
/// Claude Code uses the absolute path with '/' replaced by '-'.
/// For git repos, uses the git repository root.
fn claude_project_key(project_root: &Path) -> String {
    // Canonicalize to resolve symlinks
    let canonical = project_root.canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let path_str = canonical.to_string_lossy();
    // Replace '/' with '-' (the leading '/' becomes a leading '-')
    path_str.replace('/', "-")
}

fn claude_memory_path(project_root: &Path) -> PathBuf {
    let home = dirs::home_dir().expect("home directory");
    let key = claude_project_key(project_root);
    home.join(".claude/projects").join(key).join("memory")
}
```

**IMPORTANT:** The Claude Code project key is NOT a hash. It is the absolute path with `/` replaced by `-`. Verified by inspecting `~/.claude/projects/` on a real system. Example: `/Users/pmannion/whiskeyhouse/agentsync` becomes `-Users-pmannion-whiskeyhouse-agentsync`.

### Pattern 5: Hook TOML Schema
**What:** The `.ai/hooks.toml` schema mirroring Claude Code's event model.
**Example:**
```toml
[[PreToolUse]]
matcher = "Edit"
hooks = [{ type = "command", command = "npm run lint", timeout = 10000 }]

[[PostToolUse]]
matcher = "Write|Edit"
hooks = [{ type = "command", command = "./scripts/format.sh" }]

[[Stop]]
hooks = [{ type = "command", command = "echo 'Session complete'" }]
```

**Serde model:**
```rust
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksConfig {
    #[serde(flatten)]
    pub events: BTreeMap<String, Vec<HookGroup>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookGroup {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    pub hooks: Vec<HookHandler>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookHandler {
    #[serde(rename = "type")]
    pub hook_type: String,  // "command"
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}
```

### Anti-Patterns to Avoid
- **Separate memory sync engine:** Memory sync should integrate into the existing `SyncEngine::plan()` / `execute()` flow, not be a separate engine with its own lifecycle.
- **Hashing the project path:** Claude Code does NOT hash the path. It replaces `/` with `-`. Do not use SHA-256 or any hash.
- **Modifying symlinked files:** When Claude Code memory is symlinked, writes to either side modify the same file. The symlink target should be `.ai/memory/` (canonical) so aisync remains the source of truth.
- **Blocking on Cursor hook support:** Cursor does not support hooks. The correct behavior is a warning, not an error. Do not attempt to translate hooks for Cursor.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TOML parsing/writing | Custom parser | `toml` crate with serde derive | Already in workspace, handles all edge cases |
| JSON generation for Claude hooks | Manual string building | `serde_json::to_string_pretty()` | Proper escaping, nested structure handling |
| Interactive prompts | Raw stdin reading | `dialoguer::Select`, `dialoguer::Confirm` | TTY handling, non-TTY fallback, already used in init.rs |
| Managed section injection | Ad-hoc string replacement | Generalize `update_managed_section()` from gitignore.rs | Battle-tested pattern, handles missing/existing markers |
| File conflict diff display | Custom diff | Show file content side by side or use first few lines | Full diff is complex; simple "yours vs theirs" suffices for v1 |

## Common Pitfalls

### Pitfall 1: Claude Code Memory Symlink Direction
**What goes wrong:** Symlinking the wrong direction -- making `.ai/memory/` point to Claude's location instead of the reverse.
**Why it happens:** Confusion about which is the source of truth.
**How to avoid:** `.ai/memory/` is ALWAYS the source of truth. The symlink goes FROM `~/.claude/projects/<key>/memory/` TO `.ai/memory/`. If Claude's memory directory already exists with content, that's the import case (MEM-06), not the sync case (MEM-01).
**Warning signs:** After sync, editing `.ai/memory/` doesn't update Claude's view (because the symlink is backward).

### Pitfall 2: Memory Symlink vs Directory Symlink
**What goes wrong:** Symlinking individual files instead of the directory.
**Why it happens:** Applying the same pattern as CLAUDE.md symlink (single file).
**How to avoid:** For Claude Code memory, symlink the entire `.ai/memory/` DIRECTORY. This way new files added to `.ai/memory/` are automatically visible to Claude Code without re-syncing.

### Pitfall 3: Existing Claude Memory Directory
**What goes wrong:** Creating a symlink when Claude already has a `memory/` directory with content.
**Why it happens:** Not checking for existing directory before symlinking.
**How to avoid:** Check if `~/.claude/projects/<key>/memory/` exists. If it does and has content, warn the user and suggest `aisync memory import claude` first. If it's empty or doesn't exist, safe to symlink.

### Pitfall 4: TOML Array-of-Tables Serialization
**What goes wrong:** TOML `[[Event]]` syntax not roundtripping correctly with serde.
**Why it happens:** `BTreeMap<String, Vec<T>>` with `#[serde(flatten)]` can be tricky with TOML's array-of-tables syntax.
**How to avoid:** Test round-trip serialization thoroughly. The `toml` crate handles `[[array_name]]` correctly with `Vec<T>` in a struct, but `flatten` with dynamic keys needs careful testing.

### Pitfall 5: Memory Reference Injection into MDC Files
**What goes wrong:** Memory references injected before the frontmatter, breaking Cursor's MDC parsing.
**Why it happens:** Naive append without understanding MDC structure.
**How to avoid:** For Cursor, the managed memory section must be appended AFTER the frontmatter and instruction content. Parse the existing `.mdc` to find the right insertion point. Alternatively, create a separate `.mdc` file for memory references (e.g., `.cursor/rules/memory.mdc`).

### Pitfall 6: Hook Event Name Validation
**What goes wrong:** User enters an invalid event name in `hooks add`, producing TOML that won't translate.
**Why it happens:** Free-text input without validation.
**How to avoid:** Use `dialoguer::Select` with a fixed list of supported events, not free-text input.

## Code Examples

### Claude Code Hook Translation (HOOK-02)
```rust
// Source: https://code.claude.com/docs/en/hooks
// Claude Code settings.json format:
// {
//   "hooks": {
//     "PreToolUse": [
//       {
//         "matcher": "Edit",
//         "hooks": [
//           {
//             "type": "command",
//             "command": "npm run lint",
//             "timeout": 10
//           }
//         ]
//       }
//     ]
//   }
// }

fn translate_to_claude_json(config: &HooksConfig) -> serde_json::Value {
    let mut hooks_obj = serde_json::Map::new();
    for (event, groups) in &config.events {
        let groups_json: Vec<serde_json::Value> = groups.iter().map(|g| {
            let mut obj = serde_json::Map::new();
            if let Some(matcher) = &g.matcher {
                obj.insert("matcher".into(), serde_json::Value::String(matcher.clone()));
            }
            let hooks_arr: Vec<serde_json::Value> = g.hooks.iter().map(|h| {
                let mut hook_obj = serde_json::Map::new();
                hook_obj.insert("type".into(), serde_json::Value::String(h.hook_type.clone()));
                hook_obj.insert("command".into(), serde_json::Value::String(h.command.clone()));
                if let Some(timeout) = h.timeout {
                    // Claude Code timeout is in seconds
                    hook_obj.insert("timeout".into(), serde_json::json!(timeout / 1000));
                }
                serde_json::Value::Object(hook_obj)
            }).collect();
            obj.insert("hooks".into(), serde_json::Value::Array(hooks_arr));
            serde_json::Value::Object(obj)
        }).collect();
        hooks_obj.insert(event.clone(), serde_json::Value::Array(groups_json));
    }
    serde_json::json!({ "hooks": hooks_obj })
}
```

### OpenCode Hook Event Mapping (HOOK-03)
```rust
// Source: https://opencode.ai/docs/plugins/
// OpenCode plugin hook event mapping:
// PreToolUse  -> tool.execute.before
// PostToolUse -> tool.execute.after
// Stop        -> session.idle
// Notification -> (no direct equivalent, skip with warning)
// SubagentStop -> (no equivalent)

fn opencode_event_name(aisync_event: &str) -> Option<&'static str> {
    match aisync_event {
        "PreToolUse" => Some("tool.execute.before"),
        "PostToolUse" => Some("tool.execute.after"),
        "Stop" => Some("session.idle"),
        _ => None, // Unsupported in OpenCode
    }
}
```

### Memory Reference Block for AGENTS.md (MEM-02)
```rust
// Managed section with memory-specific markers:
const MEMORY_MARKER_START: &str = "<!-- aisync:memory -->";
const MEMORY_MARKER_END: &str = "<!-- /aisync:memory -->";

fn build_memory_reference_block(memory_files: &[PathBuf]) -> String {
    let mut lines = vec![
        MEMORY_MARKER_START.to_string(),
        "".to_string(),
        "## Project Memory".to_string(),
        "".to_string(),
    ];
    for file in memory_files {
        let name = file.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        lines.push(format!("- [{}]({})", name, file.display()));
    }
    lines.push("".to_string());
    lines.push(MEMORY_MARKER_END.to_string());
    lines.join("\n")
}
```

### Claude Project Key Derivation (MEM-01, MEM-06)
```rust
// Verified by inspecting ~/.claude/projects/ on macOS.
// Claude Code uses absolute path with '/' replaced by '-'.
// Example: /Users/pmannion/project -> -Users-pmannion-project
// Confidence: HIGH (verified against real filesystem)

fn claude_project_key(project_root: &Path) -> Result<String, AisyncError> {
    let canonical = project_root.canonicalize()
        .map_err(|e| AisyncError::Memory(MemoryError::PathResolution(e)))?;
    let path_str = canonical.to_string_lossy();
    Ok(path_str.replace('/', "-"))
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Claude Code `~/.claude/projects/<hash>/` (hash-based) | `~/.claude/projects/<path-key>/` (path-based) | Current as of Claude Code 2025 | Project key is path with `/` replaced by `-`, NOT a cryptographic hash |
| Claude Code hooks `timeout` in milliseconds | `timeout` in seconds | Current Claude Code docs | Hook timeout field in settings.json is seconds, not ms |
| OpenCode plugins: simple JS exports | OpenCode plugins: typed Plugin interface with hooks object | Current OpenCode 2025-2026 | Use `@opencode-ai/plugin` types if generating TypeScript stubs |

**Claude Code hook events (current as of March 2026):**
SessionStart, UserPromptSubmit, PreToolUse, PermissionRequest, PostToolUse, PostToolUseFailure, Notification, SubagentStart, SubagentStop, Stop, TeammateIdle, TaskCompleted, InstructionsLoaded, ConfigChange, WorktreeCreate, WorktreeRemove, PreCompact, SessionEnd

**Note:** The CONTEXT.md decision limits aisync's schema to 5 events: PreToolUse, PostToolUse, Notification, Stop, SubagentStop. This is a deliberate subset -- we don't need to support all Claude Code events in v1.

## Open Questions

1. **Memory symlink direction for Claude Code**
   - What we know: `.ai/memory/` is source of truth, Claude reads from `~/.claude/projects/<key>/memory/`
   - What's unclear: Should we symlink the entire directory or individual files? Directory symlink is simpler but means if Claude auto-memory creates NEW files in its directory (which would now be `.ai/memory/`), those files appear in the project's `.ai/memory/` immediately.
   - Recommendation: Directory symlink. The auto-creation behavior is actually desirable -- when Claude learns something, it immediately appears in `.ai/memory/`. Import (MEM-06) is only needed for projects where Claude memory already existed before aisync was installed.

2. **Hook timeout units**
   - What we know: Claude Code uses seconds for timeout. The CONTEXT.md example shows `timeout = 10000` which looks like milliseconds.
   - What's unclear: Should `.ai/hooks.toml` use milliseconds (matching the CONTEXT example) or seconds (matching Claude Code)?
   - Recommendation: Use milliseconds in `.ai/hooks.toml` (matching CONTEXT decision) and convert to seconds when translating to Claude Code JSON. This avoids surprising users who set `timeout = 10000` expecting 10 seconds.

3. **OpenCode plugin stub format**
   - What we know: OpenCode plugins are JS/TS files with typed exports.
   - What's unclear: Should we generate a complete runnable plugin file or just show the code that would need to be written?
   - Recommendation: Generate a stub `.js` file in a known location (e.g., `.opencode/plugins/aisync-hooks.js`) that users can extend. For `translate`, just show the code. For `sync`, write the actual file.

## Sources

### Primary (HIGH confidence)
- Claude Code hooks docs: https://code.claude.com/docs/en/hooks -- complete hook schema, event types, matcher patterns, JSON format
- Claude Code memory docs: https://code.claude.com/docs/en/memory -- memory file structure, auto-memory behavior, project path format
- OpenCode plugins docs: https://opencode.ai/docs/plugins/ -- plugin structure, hook events, TypeScript interface
- Local filesystem inspection: `~/.claude/projects/` -- verified project key format is path-based, not hash-based

### Secondary (MEDIUM confidence)
- Claude Code blog on hooks: https://claude.com/blog/how-to-configure-hooks -- practical examples and patterns
- OpenCode plugin gists -- community examples of plugin structure

### Tertiary (LOW confidence)
- None -- all critical claims verified against primary sources

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies needed, all existing crates sufficient
- Architecture: HIGH - extends well-established patterns from Phase 1/2
- Memory sync: HIGH - Claude project key format verified against real filesystem
- Hook schema: HIGH - Claude Code JSON format verified against official docs
- OpenCode hook mapping: MEDIUM - event mapping is reasonable but OpenCode plugin API may evolve
- Pitfalls: HIGH - based on direct codebase analysis

**Research date:** 2026-03-05
**Valid until:** 2026-04-05 (30 days -- stable domain, tools change slowly)
