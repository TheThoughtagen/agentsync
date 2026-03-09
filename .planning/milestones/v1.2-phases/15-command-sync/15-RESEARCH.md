# Phase 15: Command Sync - Research

**Researched:** 2026-03-09
**Domain:** Command file synchronization across AI coding tools
**Confidence:** HIGH

## Summary

Command sync is the simplest of the three new sync dimensions (rules, MCP, commands). Commands are plain `.md` files in `.ai/commands/` that get copied to tool-specific command directories. Unlike rules (which require frontmatter translation) and MCP (which requires config format conversion), commands are format-identical across Claude Code and Cursor -- both use `.md` files in a `commands/` subdirectory.

The codebase already has all the foundation in place from Phase 12: `CommandFile` type in `aisync-types`, `plan_commands_sync()` trait method with default no-op, `CopyCommandFile` and `RemoveFile` sync action variants, and `AnyAdapter` dispatch. The work is implementing the adapter methods, creating a `CommandEngine` loader (analogous to `RuleEngine`), wiring commands into `SyncEngine::plan_all_internal()`, and adding command import to `InitEngine`.

**Primary recommendation:** Follow the exact pattern established by rules sync in Phase 13 -- create a `CommandEngine` loader, implement `plan_commands_sync()` for Claude Code and Cursor adapters using `CopyCommandFile` actions with `aisync-` prefix for managed files and stale file cleanup, emit `WarnUnsupportedDimension` for tools without command support (Windsurf, OpenCode, Codex), and add `import_commands()` to `InitEngine`.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CMD-01 | `aisync sync` copies `.ai/commands/*.md` to `.claude/commands/` for Claude Code | CommandEngine loader + ClaudeCodeAdapter.plan_commands_sync() using CopyCommandFile actions |
| CMD-02 | `aisync sync` copies `.ai/commands/*.md` to `.cursor/commands/` for Cursor | CursorAdapter.plan_commands_sync() using CopyCommandFile actions |
| CMD-03 | `aisync init` imports existing `.claude/commands/` into `.ai/commands/` | InitEngine.import_commands() method scanning .claude/commands/*.md |
| CMD-04 | Stale aisync-managed command files are cleaned up when canonical source is removed | RemoveFile actions for aisync-* prefixed files not in expected set (same pattern as rules) |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| aisync-types | workspace | CommandFile type already defined | Phase 12 foundation |
| aisync-adapter | workspace | plan_commands_sync() trait method already defined | Phase 12 foundation |
| aisync-core | workspace | SyncEngine, InitEngine, adapter implementations | Main implementation crate |
| std::fs | stdlib | File copy, directory scanning, file removal | No external deps needed for file copy |

### Supporting
No new dependencies required. All needed types, actions, and infrastructure exist.

## Architecture Patterns

### Recommended Project Structure
```
crates/aisync-core/src/
  commands.rs           # NEW: CommandEngine (load .ai/commands/*.md)
  sync.rs               # MODIFY: Wire command loading + plan_commands_sync calls
  init.rs               # MODIFY: Add import_commands()
  adapters/
    claude_code.rs      # MODIFY: Implement plan_commands_sync()
    cursor.rs           # MODIFY: Implement plan_commands_sync()
    windsurf.rs         # No change (WarnUnsupportedDimension default)
    opencode.rs         # No change (WarnUnsupportedDimension default)
    codex.rs            # No change (WarnUnsupportedDimension default)
```

### Pattern 1: CommandEngine Loader (analogous to RuleEngine)
**What:** A `CommandEngine::load()` function that scans `.ai/commands/*.md`, returns `Vec<CommandFile>`
**When to use:** Called from `SyncEngine::plan_all_internal()` before the tool loop
**Example:**
```rust
// Follows exact pattern of crates/aisync-core/src/rules.rs
pub struct CommandEngine;

impl CommandEngine {
    pub fn load(project_root: &Path) -> Result<Vec<CommandFile>, AisyncError> {
        let commands_dir = project_root.join(".ai/commands");
        if !commands_dir.is_dir() {
            return Ok(vec![]);
        }
        let mut commands = Vec::new();
        let entries = std::fs::read_dir(&commands_dir)
            .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
        for entry in entries {
            let entry = entry.map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                let content = std::fs::read_to_string(&path)
                    .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
                let name = path.file_stem().unwrap().to_string_lossy().to_string();
                commands.push(CommandFile { name, content, source_path: path });
            }
        }
        commands.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(commands)
    }
}
```

### Pattern 2: Adapter plan_commands_sync (Claude Code + Cursor)
**What:** Each adapter generates CopyCommandFile actions for each command, plus RemoveFile for stale aisync-* files
**When to use:** Both Claude Code and Cursor have the same directory-based command format
**Example:**
```rust
// For ClaudeCodeAdapter
fn plan_commands_sync(
    &self,
    project_root: &Path,
    commands: &[CommandFile],
) -> Result<Vec<SyncAction>, AdapterError> {
    if commands.is_empty() {
        return Ok(vec![]);
    }
    let mut actions = Vec::new();
    let commands_dir = project_root.join(".claude/commands");

    // Ensure directory exists
    if !commands_dir.is_dir() {
        actions.push(SyncAction::CreateDirectory { path: commands_dir.clone() });
    }

    // Build expected filenames
    let expected: HashSet<String> = commands.iter()
        .map(|c| format!("aisync-{}.md", c.name))
        .collect();

    // Copy each command file
    for cmd in commands {
        let filename = format!("aisync-{}.md", cmd.name);
        let output = commands_dir.join(&filename);

        // Idempotent: skip if content matches
        if output.exists() {
            if let Ok(existing) = std::fs::read_to_string(&output) {
                if existing == cmd.content {
                    continue;
                }
            }
        }

        actions.push(SyncAction::CopyCommandFile {
            source: cmd.source_path.clone(),
            output,
            command_name: cmd.name.clone(),
        });
    }

    // Remove stale aisync-* command files
    if commands_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&commands_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("aisync-") && name.ends_with(".md") && !expected.contains(&name) {
                    actions.push(SyncAction::RemoveFile { path: entry.path() });
                }
            }
        }
    }

    Ok(actions)
}
```

### Pattern 3: Shared helper for Claude Code + Cursor
**What:** Since both adapters have identical logic (only the target directory differs), a shared helper in `adapters/mod.rs` avoids duplication
**When to use:** This mirrors the `plan_single_file_rules_sync` pattern already in `adapters/mod.rs`
**Example:**
```rust
// In adapters/mod.rs
pub(crate) fn plan_directory_commands_sync(
    commands_dir: PathBuf,
    commands: &[CommandFile],
) -> Result<Vec<SyncAction>, AdapterError> {
    // ... shared logic for both .claude/commands/ and .cursor/commands/
}
```

### Pattern 4: SyncEngine integration
**What:** Wire CommandEngine::load() into plan_all_internal(), call plan_commands_sync() for each adapter
**When to use:** Follows the exact pattern of rules sync integration
**Example:**
```rust
// In sync.rs plan_all_internal(), after rules loading:
let commands = crate::commands::CommandEngine::load(project_root)?;

// In the tool loop, after rule sync block:
if !commands.is_empty() {
    match adapter.plan_commands_sync(project_root, &commands) {
        Ok(cmd_actions) => actions.extend(cmd_actions),
        Err(e) => {
            actions.push(SyncAction::WarnUnsupportedDimension {
                tool: tool_kind.clone(),
                dimension: "commands".into(),
                reason: format!("command sync failed: {e}"),
            });
        }
    }
}
```

### Pattern 5: Init command import
**What:** `InitEngine::import_commands()` copies `.claude/commands/*.md` into `.ai/commands/`
**When to use:** Called during `aisync init` scaffold
**Example:**
```rust
pub fn import_commands(project_root: &Path) -> Result<usize, AisyncError> {
    let commands_dir = project_root.join(".ai/commands");
    std::fs::create_dir_all(&commands_dir)
        .map_err(|e| InitError::ScaffoldFailed(e))?;
    let mut count = 0;

    let claude_commands = project_root.join(".claude/commands");
    if claude_commands.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&claude_commands) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "md") {
                    let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                    // Skip aisync-managed files
                    if stem.starts_with("aisync-") { continue; }
                    let content = std::fs::read_to_string(&path)
                        .map_err(|e| InitError::ImportFailed(format!("read {}: {e}", path.display())))?;
                    let output = commands_dir.join(format!("{stem}.md"));
                    std::fs::write(&output, &content)
                        .map_err(|e| InitError::ScaffoldFailed(e))?;
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}
```

### Anti-Patterns to Avoid
- **Symlinks for commands:** Commands should use file copy (via CopyCommandFile), not symlinks. The aisync-prefix convention requires writing new files with different names.
- **No aisync- prefix:** Without prefix, aisync would overwrite user-created native command files. Always use `aisync-{name}.md` naming.
- **Skipping idempotency check:** Always compare content before generating CopyCommandFile actions to avoid unnecessary writes.
- **Importing from Cursor commands too:** STATE.md notes "Cursor command format documentation is sparse -- validate during Phase 15." Only import from `.claude/commands/` per CMD-03. Cursor commands use the same `.md` format so import from `.cursor/commands/` could be added, but is not required by the spec.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| File loading | Custom directory walker | Follow RuleEngine::load() pattern exactly | Consistent error handling, sorting, extension filtering |
| Stale file cleanup | Custom tracking mechanism | Scan for aisync-* prefix, compare against expected set | Same pattern works for rules, proven in Phase 13 |
| Action execution | Custom copy logic | Existing CopyCommandFile executor in sync.rs | Already handles parent dir creation + fs::copy |

**Key insight:** Nearly every pattern needed for command sync already exists in the codebase from Phase 13 (rules sync). The implementation is copy-adapt, not design-from-scratch.

## Common Pitfalls

### Pitfall 1: Command naming conflicts
**What goes wrong:** An aisync-managed command named `aisync-review.md` maps to `/aisync-review` in Claude Code, not `/review`
**Why it happens:** The aisync- prefix is needed to avoid overwriting user files, but it changes the command name
**How to avoid:** This is an accepted tradeoff documented in the project decisions. The alternative (no prefix) risks overwriting user commands. Users who want clean names can use the canonical `.ai/commands/` directly. Note: Claude Code and Cursor both derive command names from filenames.
**Warning signs:** Users confused about why their slash command is `/aisync-build` instead of `/build`

### Pitfall 2: Forgetting to wire into SyncEngine
**What goes wrong:** CommandEngine exists, adapter methods work, but `aisync sync` never calls them
**Why it happens:** Missing the integration point in `plan_all_internal()`
**How to avoid:** Follow the exact pattern of rules integration: load before the tool loop, call in the tool loop

### Pitfall 3: Not registering commands.rs in lib.rs
**What goes wrong:** Compilation error -- module not found
**Why it happens:** New file created but not declared in `crates/aisync-core/src/lib.rs`
**How to avoid:** Add `pub mod commands;` to lib.rs

### Pitfall 4: Subdirectory commands
**What goes wrong:** Commands in `.ai/commands/subdir/` are not synced
**Why it happens:** CommandEngine only scans top-level `.md` files
**How to avoid:** For v1.2, only top-level `.md` files are in scope. Claude Code supports subdirectories (creating namespaced commands like `/subdir:name`), but this adds complexity. If needed, add recursive scanning in a future version.

### Pitfall 5: Missing export in lib.rs
**What goes wrong:** Other crates can't use CommandEngine
**Why it happens:** Module declared but not re-exported
**How to avoid:** Check existing lib.rs re-export pattern for RuleEngine

## Code Examples

### Existing CopyCommandFile execution (already in sync.rs)
```rust
// Source: crates/aisync-core/src/sync.rs line 672
SyncAction::CopyCommandFile { output, source, .. } => {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
    }
    std::fs::copy(source, output)
        .map_err(|e| AisyncError::Sync(SyncError::WriteFailed(e)))?;
    Ok(())
}
```

### Existing CommandFile type (already in aisync-types)
```rust
// Source: crates/aisync-types/src/lib.rs line 210
pub struct CommandFile {
    pub name: String,
    pub content: String,
    pub source_path: PathBuf,
}
```

### Existing CopyCommandFile display (already in aisync-types)
```rust
// Source: crates/aisync-types/src/lib.rs line 408
SyncAction::CopyCommandFile { output, command_name, .. } => {
    write!(f, "Would copy command '{}': {}", command_name, output.display())
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Claude Code `.claude/commands/` only | Merged into skills system (`.claude/skills/`) | Late 2025 | `.claude/commands/` still works, skills preferred for new commands |
| No Cursor command support | `.cursor/commands/` directory | Cursor 1.6 (Sep 2025) | Format identical to Claude Code -- plain .md files |

**Key findings on tool command formats:**
- **Claude Code:** `.claude/commands/*.md` -- filename becomes `/command-name`. Supports `$ARGUMENTS` placeholder. Supports subdirectories for namespacing (`subdir/cmd.md` becomes `/subdir:cmd`). Also supports YAML frontmatter (name, description, etc.) but it's optional. Files still work without frontmatter.
- **Cursor:** `.cursor/commands/*.md` -- filename becomes the command identifier. Same basic format as Claude Code. Released in Cursor 1.6 (Sep 2025).
- **OpenCode:** No documented command format. STATE.md notes: "OpenCode command format undocumented -- may skip OpenCode command sync"
- **Windsurf:** No project-level command support documented.
- **Codex:** No command format documented.

**Decision from STATE.md:** "Cursor command format documentation is sparse -- validate during Phase 15" -- validated above, format confirmed as `.cursor/commands/*.md`.

## Open Questions

1. **Should import also scan `.cursor/commands/`?**
   - What we know: CMD-03 only specifies `.claude/commands/` import
   - What's unclear: Whether Cursor commands should also be imported
   - Recommendation: Stick to spec (only `.claude/commands/`). Can add Cursor import later. Cursor commands are identical format so the code change would be trivial.

2. **Should subdirectory commands be supported?**
   - What we know: Claude Code supports subdirectories for namespaced commands
   - What's unclear: Whether Cursor also supports subdirectories
   - Recommendation: v1.2 scope is top-level only. Recursive scanning is a natural v1.3 enhancement.

3. **Idempotency via content comparison vs. CopyCommandFile**
   - What we know: `CopyCommandFile` uses `fs::copy` which copies file content. We need to compare content to determine if a copy is needed.
   - What's unclear: Whether to compare via content string or file hash
   - Recommendation: Use `std::fs::read_to_string` content comparison (same pattern as rules sync). Simple and reliable.

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `crates/aisync-types/src/lib.rs` -- CommandFile, CopyCommandFile types
- Codebase analysis: `crates/aisync-adapter/src/lib.rs` -- plan_commands_sync() trait method
- Codebase analysis: `crates/aisync-core/src/sync.rs` -- CopyCommandFile executor
- Codebase analysis: `crates/aisync-core/src/rules.rs` -- RuleEngine pattern to follow
- Codebase analysis: `crates/aisync-core/src/adapters/cursor.rs` -- plan_rules_sync() pattern with stale cleanup
- Codebase analysis: `crates/aisync-core/src/init.rs` -- import_rules() pattern to follow
- [Claude Code Skills Documentation](https://code.claude.com/docs/en/skills) -- `.claude/commands/` format confirmed
- [Cursor Commands Documentation](https://cursor.com/docs/context/commands) -- `.cursor/commands/` format confirmed

### Secondary (MEDIUM confidence)
- STATE.md project decisions on command sync scope and constraints

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all types and infrastructure already exist from Phase 12
- Architecture: HIGH - follows exact pattern of Phase 13 rules sync, which is proven
- Pitfalls: HIGH - pitfalls are well-understood from rules sync experience

**Research date:** 2026-03-09
**Valid until:** 2026-04-09 (stable -- command format unlikely to change)
