# Phase 1: Add Cursor Plugin Ecosystem Support - Research

**Researched:** 2026-03-19
**Domain:** Cursor plugin ecosystem (hooks, skills, agents, commands, plugin manifest) — Rust adapter extension
**Confidence:** HIGH (Cursor official docs fetched directly; codebase read in full)

---

## Summary

Cursor released a first-class plugin system that bundles hooks, skills, agents, commands, MCP servers, and rules into a single distributable unit defined by `.cursor-plugin/plugin.json`. The aisync Cursor adapter already handles rules (`.cursor/rules/*.mdc`), MCP (`.cursor/mcp.json`), and commands (`.cursor/commands/`). This phase adds the remaining unsupported dimensions: **hooks** (`.cursor/hooks.json`), **skills** (`.cursor/skills/` or `skills/{name}/SKILL.md`), **agents** (`.cursor/agents/*.md`), and optionally the plugin **manifest** (`.cursor-plugin/plugin.json`).

The biggest architectural clarification is hooks: the existing `translate_hooks` method on `CursorAdapter` returns `Unsupported` with the reason "Cursor does not support hooks". That is now wrong — Cursor has a rich hooks system at `.cursor/hooks.json`. Skills and agents are new sync dimensions with no existing canonical storage in aisync's `.ai/` directory, so new canonical directory structures (`.ai/skills/` and `.ai/agents/`) and corresponding engine types must be introduced alongside the trait expansion.

**Primary recommendation:** Follow the exact same pattern used for commands and rules — new canonical loader engine, new `SyncAction` variants, new `plan_*_sync` trait methods, implement on `CursorAdapter`, stub as default no-op on all other adapters. Update `SyncEngine::plan_all_internal` to call the new methods. Fix `translate_hooks` on `CursorAdapter` to actually translate hooks to Cursor's `.cursor/hooks.json` format.

---

## Standard Stack

### Core (no new crates needed)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde_json | 1.0 | Serialize Cursor hooks.json | Already in workspace deps |
| serde | 1.0 | Derive for new types | Already in workspace deps |
| toml | 0.8 | (existing) hooks.toml canonical source | Already in workspace deps |

### No New Dependencies Required

All new functionality fits within the existing workspace. Cursor hooks.json is JSON (handled by serde_json). Skills and agents are markdown files with YAML frontmatter (same hand-parsed pattern already used for rules). The canonical `.ai/` directory can be extended with `skills/` and `agents/` subdirectories.

---

## Architecture Patterns

### Existing Pattern to Follow

Every sync dimension added since v1.2 follows this identical pattern:

1. **New canonical type** in `aisync-types/src/lib.rs` (e.g., `SkillFile`, `AgentFile`)
2. **New SyncAction variants** in `aisync-types/src/lib.rs` (e.g., `WriteSkillFile`, `WriteAgentFile`, `WriteCursorHooks`)
3. **New loader engine** in `aisync-core/src/` (e.g., `skills.rs`, `agents.rs`)
4. **New trait method** in `aisync-adapter/src/lib.rs` with default no-op return
5. **Cursor adapter implementation** in `aisync-core/src/adapters/cursor.rs`
6. **Wire into SyncEngine** in `aisync-core/src/sync.rs` — load canonical, call adapter method

### Project Structure (New Files)

```
crates/
├── aisync-types/src/lib.rs          # Add SkillFile, AgentFile, CursorHooksConfig types + SyncAction variants
├── aisync-adapter/src/lib.rs        # Add plan_skills_sync, plan_agents_sync trait methods
├── aisync-core/src/
│   ├── skills.rs                    # NEW: SkillEngine::load(.ai/skills/)
│   ├── agents.rs                    # NEW: AgentEngine::load(.ai/agents/)
│   ├── adapters/cursor.rs           # Update: translate_hooks (fix Unsupported), plan_skills_sync, plan_agents_sync
│   └── sync.rs                      # Update: load skills/agents, call new adapter methods
└── .ai/
    ├── skills/                      # NEW canonical directory
    │   └── {name}/SKILL.md          # Skill files mirroring Cursor's format
    └── agents/                      # NEW canonical directory
        └── {name}.md                # Agent files
```

### Pattern 1: Cursor Hooks Translation

**What:** `translate_hooks` currently returns `Unsupported` for Cursor — this must be fixed.

**Cursor hooks.json format** (project-level at `.cursor/hooks.json`):
```json
{
  "version": 1,
  "hooks": {
    "preToolUse": [
      {
        "command": "./scripts/lint.sh",
        "timeout": 30,
        "matcher": "Shell|Write"
      }
    ],
    "postToolUse": [
      { "command": "./scripts/format.sh" }
    ]
  }
}
```

**Canonical `.ai/hooks.toml` format:**
```toml
[[PreToolUse]]
matcher = "Edit"

[[PreToolUse.hooks]]
type = "command"
command = "npm run lint"
timeout = 10000
```

**Translation mapping:**
| Canonical (HooksConfig) | Cursor hooks.json |
|------------------------|-------------------|
| Event key `PreToolUse` | `preToolUse` (camelCase) |
| Event key `PostToolUse` | `postToolUse` |
| Event key `Stop` | `stop` |
| Event key `SubagentStop` | `subagentStop` |
| Event key `Notification` | No direct equivalent (warn + skip) |
| `hooks[].command` | `command` |
| `hooks[].timeout` (ms) | `timeout` (seconds) — divide by 1000 |
| `hooks[].type` | No `type` field in Cursor format — `command` type only |
| `group.matcher` | `matcher` string |

**SyncAction:** `WriteHookTranslation` already exists. Update `SyncEngine` to route Cursor translations to `.cursor/hooks.json`.

**Important:** `translate_hooks` changes from `Unsupported` to `Supported` for Cursor. The `sync.rs` hook translation block hardcodes paths only for ClaudeCode and OpenCode — add Cursor's `.cursor/hooks.json` path.

### Pattern 2: Skills Sync

Cursor stores skills at `skills/{name}/SKILL.md` (inside a plugin directory) or at `.cursor/skills/{name}/SKILL.md` (project-level, most relevant for aisync).

**Canonical source:** `.ai/skills/{name}/SKILL.md`

**Cursor destination:** `.cursor/skills/{name}/SKILL.md`

**SKILL.md format** (both canonical and Cursor native):
```markdown
---
name: skill-identifier
description: What this skill does
---

# Skill Title

Content here.
```

**Since the format is identical**, aisync simply copies/manages the directory structure. No translation needed — just copy with aisync management prefix.

**Type:**
```rust
pub struct SkillFile {
    pub name: String,           // directory name (e.g., "my-skill")
    pub content: String,        // full SKILL.md content
    pub source_path: PathBuf,   // .ai/skills/{name}/SKILL.md
}
```

**SyncAction variants:**
```rust
WriteSkillFile { output: PathBuf, content: String, skill_name: String },
RemoveSkillDir { path: PathBuf },  // for stale cleanup
```

**Naming pattern:** Use `aisync-{name}` prefix for directories to distinguish managed from user-created: `.cursor/skills/aisync-{name}/SKILL.md`.

### Pattern 3: Agents Sync

Cursor stores agents at `.cursor/agents/*.md` (project-level).

**Canonical source:** `.ai/agents/{name}.md`

**Cursor destination:** `.cursor/agents/aisync-{name}.md`

**Agent .md format** (Cursor native and canonical match):
```markdown
---
name: agent-identifier
description: Agent focus area
---

# Agent Name

Behavioral guidelines here.
```

**Type:**
```rust
pub struct AgentFile {
    pub name: String,           // stem of file (e.g., "backend-expert")
    pub content: String,        // full .md content
    pub source_path: PathBuf,   // .ai/agents/{name}.md
}
```

**Closely mirrors `CommandFile`** — same directory copy pattern via `plan_directory_commands_sync` helper but targeting `.cursor/agents/`.

### Pattern 4: Plugin Manifest (`.cursor-plugin/plugin.json`)

**Decision: Defer or make optional.** The manifest is for publishing to the Cursor marketplace, not for local project sync. Aisync's value is syncing configs across tools, not publishing plugins. The manifest would be a generated artifact pointing to the synced files.

**Recommendation:** Do NOT sync plugin manifests in this phase. It's a publishing concern, not a project sync concern. If desired later, it can generate a manifest pointing to the aisync-managed artifacts.

### Anti-Patterns to Avoid

- **Anti-pattern: Overwriting user-created skills/agents.** Use `aisync-` prefix for managed directories/files, same as rules and commands.
- **Anti-pattern: Translating Notification hooks.** Cursor does not have a `Notification` equivalent — warn and skip, same as other unsupported events.
- **Anti-pattern: Recursive sync of `.cursor-plugin/` directory.** That is a publishing artifact, not a project config. Out of scope.
- **Anti-pattern: Syncing skills/agents to non-Cursor tools.** Only Cursor has skills/agents in this format. Other adapters return default no-op.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Hook event name mapping | Custom lookup table | Follow existing hook translation pattern in `claude_code.rs` | Established precedent |
| YAML frontmatter parsing | Write a YAML parser | Same hand-parsed pattern used for rules (strip `---` delimiters, split key: value) | No serde_yml dependency needed |
| Directory management | Custom stale file tracker | Same `aisync-` prefix + scan pattern from `plan_directory_commands_sync` | Already battle-tested |
| JSON serialization | Manual string concat | `serde_json::json!()` macro | Already used throughout codebase |

**Key insight:** Every pattern needed here already exists in the codebase. This phase is applying existing patterns to new Cursor-specific sync dimensions.

---

## Common Pitfalls

### Pitfall 1: Cursor Hook Event Name Case Mismatch

**What goes wrong:** Canonical `.ai/hooks.toml` uses `PascalCase` event names (`PreToolUse`, `PostToolUse`). Cursor's `hooks.json` uses `camelCase` (`preToolUse`, `postToolUse`). Writing PascalCase to Cursor's file causes hooks to be silently ignored.

**Why it happens:** Claude Code also uses `PascalCase` (matching canonical), so the existing translation for Claude Code doesn't do any case conversion.

**How to avoid:** Map `PreToolUse` → `preToolUse`, `PostToolUse` → `postToolUse`, `Stop` → `stop`, `SubagentStop` → `subagentStop` explicitly in the Cursor translation. Use a match arm or simple `to_lowercase_first_char()` helper.

**Warning signs:** Hook tests pass but hooks don't fire in Cursor.

### Pitfall 2: Timeout Unit Mismatch

**What goes wrong:** Canonical hooks store timeout in milliseconds (e.g., `10000`). Cursor hooks.json expects timeout in **seconds** (e.g., `30`). Writing raw milliseconds to Cursor will make hooks time out ~1000x faster than intended.

**Why it happens:** Claude Code also uses seconds — the existing `translate_hooks` already divides by 1000. Apply same conversion for Cursor.

**How to avoid:** `timeout_seconds = timeout_ms / 1000` — already done in `claude_code.rs`, must replicate in cursor translation.

### Pitfall 3: WriteHookTranslation Path Routing in sync.rs

**What goes wrong:** `sync.rs` handles `HookTranslation::Supported` with a hardcoded match on `ToolKind::ClaudeCode` and `ToolKind::OpenCode` to determine output path. When Cursor is added as supported, the `_ => continue` arm will skip it.

**Why it happens:** The routing was written before Cursor had hook support.

**How to avoid:** Add `ToolKind::Cursor => project_root.join(".cursor/hooks.json")` to the match arm in `plan_all_internal`.

### Pitfall 4: Skills Directory vs. File Layout

**What goes wrong:** Each Cursor skill is a *directory* (`skills/my-skill/SKILL.md`), not a flat file. Treating it like a command file (flat copy) will create `.cursor/skills/aisync-my-skill.md` instead of `.cursor/skills/aisync-my-skill/SKILL.md`.

**Why it happens:** Commands are flat files; skills are directory-based.

**How to avoid:** `WriteSkillFile` action must create the parent directory (`.cursor/skills/aisync-{name}/`) before writing `SKILL.md` inside it.

### Pitfall 5: Missing `version: 1` in Cursor hooks.json

**What goes wrong:** Cursor's hooks.json schema requires a `"version": 1` field. Omitting it may cause Cursor to reject or silently ignore the file.

**Why it happens:** Claude Code's settings.json has no version field.

**How to avoid:** Always include `"version": 1` as the top-level field when generating `.cursor/hooks.json`.

---

## Code Examples

### Cursor Hook Translation (reference for implementation)

```rust
// Source: official Cursor docs at cursor.com/docs/reference/hooks
// Cursor hooks.json format:
// {
//   "version": 1,
//   "hooks": {
//     "preToolUse": [{ "command": "...", "timeout": 30, "matcher": "..." }]
//   }
// }

fn event_name_to_cursor(event: &str) -> Option<&'static str> {
    match event {
        "PreToolUse" => Some("preToolUse"),
        "PostToolUse" => Some("postToolUse"),
        "Stop" => Some("stop"),
        "SubagentStop" => Some("subagentStop"),
        "Notification" => None, // no Cursor equivalent
        _ => None,
    }
}
```

### SyncAction Routing in sync.rs (existing pattern, needs Cursor arm)

```rust
// Source: crates/aisync-core/src/sync.rs (existing code to update)
Ok(HookTranslation::Supported { tool, content, .. }) => {
    let path = match &tool {
        ToolKind::ClaudeCode => project_root.join(".claude/settings.json"),
        ToolKind::OpenCode => project_root.join(".opencode/plugins/aisync-hooks.js"),
        ToolKind::Cursor => project_root.join(".cursor/hooks.json"),  // ADD THIS
        _ => continue,
    };
    actions.push(SyncAction::WriteHookTranslation { path, content, tool });
}
```

### Skill Directory Pattern

```rust
// Each skill is a directory: .cursor/skills/aisync-{name}/SKILL.md
// SyncAction::WriteSkillFile writes to:
let output_dir = project_root.join(".cursor/skills").join(format!("aisync-{}", skill.name));
let output_file = output_dir.join("SKILL.md");
```

### Agent File Pattern (mirrors command pattern)

```rust
// Agents are flat .md files: .cursor/agents/aisync-{name}.md
// Reuse plan_directory_commands_sync helper pattern
super::plan_directory_agents_sync(
    project_root.join(".cursor/agents"),
    agents,
)
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Cursor had no hooks | Cursor has `.cursor/hooks.json` with 20+ event types | Early 2026 | Must update `translate_hooks` from Unsupported to Supported |
| No Cursor skills/agents | Cursor plugin system has skills and agents | Early 2026 | New sync dimensions needed |
| Cursor rules only in `.cursorrules` | `.cursor/rules/*.mdc` is current standard | Pre-v1.0 | Already handled correctly by aisync |

**Deprecated/outdated:**
- `CursorAdapter.translate_hooks()` returning `Unsupported` — must change to `Supported` with proper JSON translation.

---

## Open Questions

1. **Skills/agents canonical directory location**
   - What we know: Cursor uses `skills/{name}/SKILL.md` per plugin; aisync needs a canonical source
   - What's unclear: Whether `.ai/skills/` should mirror Cursor's exact structure or be flat
   - Recommendation: Mirror Cursor's directory-per-skill pattern (`.ai/skills/{name}/SKILL.md`) — enables zero-transform copy

2. **Should skills/agents sync to non-Cursor tools?**
   - What we know: Only Cursor has documented skills/agents format (as of this research)
   - What's unclear: Whether Claude Code `.claude/agents/` follows the same format
   - Recommendation: Cursor-only in this phase. Claude Code agents (`.claude/agents/`) can be addressed in a follow-up phase. The `plan_skills_sync` and `plan_agents_sync` default no-op in other adapters.

3. **Claude Code `.claude/agents/` directory**
   - What we know: Claude Code has an agents directory (`~/.claude/agents/` global)
   - What's unclear: Whether project-level `.claude/agents/` is standard
   - Recommendation: Research separately; do NOT block this phase on it

4. **Plugin manifest (`.cursor-plugin/plugin.json`) scope**
   - What we know: It's for marketplace publishing, not project sync
   - What's unclear: Whether users want aisync to auto-generate it
   - Recommendation: Out of scope for this phase — publish concern, not sync concern

---

## Detailed Implementation Plan

### What changes in each file

**`crates/aisync-types/src/lib.rs`:**
- Add `SkillFile { name, content, source_path }` struct
- Add `AgentFile { name, content, source_path }` struct (or reuse `CommandFile` pattern)
- Add `SyncAction::WriteSkillFile { output, content, skill_name }` variant
- Add `SyncAction::WriteAgentFile { output, content, agent_name }` (or reuse `CopyCommandFile`)
- Add `Display` impl for new variants

**`crates/aisync-adapter/src/lib.rs`:**
- Add `plan_skills_sync(&self, project_root: &Path, skills: &[SkillFile]) -> Result<Vec<SyncAction>, AdapterError>` with default no-op
- Add `plan_agents_sync(&self, project_root: &Path, agents: &[AgentFile]) -> Result<Vec<SyncAction>, AdapterError>` with default no-op

**`crates/aisync-core/src/lib.rs`:**
- Re-export new `SkillEngine` and `AgentEngine`

**`crates/aisync-core/src/skills.rs`** (new file):
- `SkillEngine::load(project_root)` — scans `.ai/skills/*/SKILL.md`

**`crates/aisync-core/src/agents.rs`** (new file):
- `AgentEngine::load(project_root)` — scans `.ai/agents/*.md`

**`crates/aisync-core/src/adapters/cursor.rs`:**
- Fix `translate_hooks` — return `Supported` with Cursor `hooks.json` JSON format
- Add `plan_skills_sync` — generate `.cursor/skills/aisync-{name}/SKILL.md` per skill
- Add `plan_agents_sync` — generate `.cursor/agents/aisync-{name}.md` per agent (via `plan_directory_agents_sync` or inline)

**`crates/aisync-core/src/adapters/mod.rs`:**
- Add `plan_directory_agents_sync` shared helper (mirrors `plan_directory_commands_sync`)

**`crates/aisync-core/src/adapter.rs`** (AnyAdapter dispatch):
- Add dispatch arms for `plan_skills_sync` and `plan_agents_sync`

**`crates/aisync-core/src/sync.rs`:**
- Load skills: `let skills = SkillEngine::load(project_root)?;`
- Load agents: `let agents = AgentEngine::load(project_root)?;`
- Call `adapter.plan_skills_sync` and `adapter.plan_agents_sync`
- Fix hook path routing: add `ToolKind::Cursor => project_root.join(".cursor/hooks.json")`

---

## Sources

### Primary (HIGH confidence)
- `https://cursor.com/docs/reference/plugins` — fetched 2026-03-19 — plugin system structure, skills/agents/commands/hooks/manifest format
- `https://cursor.com/docs/reference/hooks` — fetched 2026-03-19 — hooks.json format, all event names, timeout in seconds, version field, matcher field, Cursor vs Claude Code differences
- `/Users/pmannion/whiskeyhouse/agentsync/crates/aisync-core/src/adapters/cursor.rs` — full existing Cursor adapter implementation
- `/Users/pmannion/whiskeyhouse/agentsync/crates/aisync-adapter/src/lib.rs` — ToolAdapter trait with all existing methods
- `/Users/pmannion/whiskeyhouse/agentsync/crates/aisync-types/src/lib.rs` — all existing types and SyncAction variants
- `/Users/pmannion/whiskeyhouse/agentsync/crates/aisync-core/src/sync.rs` — SyncEngine orchestration, hook path routing

### Secondary (MEDIUM confidence)
- Existing `translate_hooks` in `claude_code.rs` — reference implementation showing timeout /1000 conversion and JSON format
- Existing `plan_directory_commands_sync` in `adapters/mod.rs` — definitive pattern for agent file sync
- Existing `plan_rules_sync` in `cursor.rs` — definitive pattern for skills directory sync

---

## Metadata

**Confidence breakdown:**
- Cursor hooks format: HIGH — fetched from official docs, exact schema documented
- Cursor skills/agents format: HIGH — fetched from official docs
- Adapter extension pattern: HIGH — read full codebase, identical pattern already used 3x
- Skills canonical directory design: MEDIUM — design decision, no prior art in codebase
- Agent file format compatibility: MEDIUM — assumes same YAML frontmatter pattern as existing types

**Research date:** 2026-03-19
**Valid until:** 2026-06-19 (Cursor plugin API may evolve, but hooks/skills/agents format is stable as of official docs)
