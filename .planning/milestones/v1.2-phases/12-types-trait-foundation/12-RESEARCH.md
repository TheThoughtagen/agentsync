# Phase 12: Types & Trait Foundation - Research

**Researched:** 2026-03-09
**Domain:** Rust type system, trait extension, enum dispatch
**Confidence:** HIGH

## Summary

Phase 12 is a foundational type/trait expansion phase. It adds new types (`RuleFile`, `RuleMetadata`, `McpConfig`, `McpServer`, `CommandFile`) to the `aisync-types` crate, extends the `ToolAdapter` trait with three new methods (`plan_rules_sync`, `plan_mcp_sync`, `plan_commands_sync`), adds new `SyncAction` variants, and updates `AnyAdapter` dispatch. This is a pure additive change with zero breaking impact on existing code.

The existing codebase has a clean, well-established pattern for this exact kind of extension. The `ToolAdapter` trait already uses default method implementations (7 of 11 methods have defaults), so adding three more with default no-ops is straightforward. The `AnyAdapter` enum dispatch uses a `dispatch_adapter!` macro that makes adding new method forwarding trivial. The `SyncAction` enum already has 13 variants with Display implementations.

**Primary recommendation:** Follow the existing pattern exactly. Add types to `aisync-types`, add trait methods with defaults to `aisync-adapter`, add SyncAction variants to `aisync-types`, update `AnyAdapter` dispatch in `aisync-core`. No new crates, no architecture changes.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TYPE-01 | `aisync-types` exports `RuleFile`, `RuleMetadata`, `McpConfig`, `McpServer`, `CommandFile` types | Types follow existing serde-derived struct pattern in `aisync-types/src/lib.rs`; TOML frontmatter for rules, TOML for MCP config |
| TYPE-02 | `ToolAdapter` trait has `plan_rules_sync()`, `plan_mcp_sync()`, `plan_commands_sync()` with default no-ops | Follows established default impl pattern in `aisync-adapter/src/lib.rs`; 7 existing methods already use this pattern |
| TYPE-03 | `SyncAction` enum has variants for rule file creation, MCP file generation, command file copying | Follows existing variant pattern in `aisync-types/src/lib.rs`; needs Display impls matching existing style |
| TYPE-04 | `AnyAdapter` dispatches new trait methods to all variants including Plugin | Uses existing `dispatch_adapter!` macro in `aisync-core/src/adapter.rs`; 3 new dispatch lines |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1.0 (workspace) | Derive Serialize/Deserialize for new types | Already used by all types in aisync-types |
| toml | 0.8 (workspace) | TOML frontmatter parsing for rules, MCP config parsing | Already a workspace dependency |
| thiserror | 2.0 (workspace) | Error types if needed | Already used by aisync-types and aisync-adapter |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde_json | 1.0 (workspace) | MCP JSON generation (Claude/Cursor formats) | Already in dev-deps, may need in regular deps for McpConfig serialization to JSON |

### Alternatives Considered
None -- this phase uses only existing workspace dependencies. No new libraries needed.

## Architecture Patterns

### Crate Dependency Chain (existing, unchanged)
```
aisync-types  (shared types, no trait)
     |
aisync-adapter  (ToolAdapter trait, depends on aisync-types)
     |
aisync-core  (AnyAdapter, adapters, sync engine)
     |
aisync  (CLI binary)
```

### Pattern 1: Default Trait Method (existing pattern)
**What:** New trait methods get default no-op implementations returning `Ok(vec![])`.
**When to use:** For all three new methods. Ensures existing adapters (ClaudeCode, Cursor, OpenCode, Windsurf, Codex, and all Plugin adapters) compile unchanged.
**Example:**
```rust
// Source: aisync-adapter/src/lib.rs (existing pattern)
fn plan_rules_sync(
    &self,
    project_root: &Path,
    rules: &[RuleFile],
) -> Result<Vec<SyncAction>, AdapterError> {
    let _ = (project_root, rules);
    Ok(vec![])
}
```

### Pattern 2: SyncAction Enum Variant (existing pattern)
**What:** Each sync dimension gets specific action variants in the SyncAction enum.
**When to use:** For rule creation, MCP generation, and command copying.
**Example:**
```rust
// Source: aisync-types/src/lib.rs (existing pattern)
// Existing: GenerateMdc, CreateFile, CreateSymlink
// New variants follow same shape:
SyncAction::CreateRuleFile {
    output: PathBuf,
    content: String,
    rule_name: String,
}
```

### Pattern 3: dispatch_adapter! Macro (existing pattern)
**What:** The macro dispatches method calls through all AnyAdapter enum variants.
**When to use:** For forwarding the three new trait methods.
**Example:**
```rust
// Source: aisync-core/src/adapter.rs (existing pattern)
// Each new method gets one dispatch line:
fn plan_rules_sync(
    &self,
    project_root: &Path,
    rules: &[RuleFile],
) -> Result<Vec<SyncAction>, AdapterError> {
    dispatch_adapter!(self, a => a.plan_rules_sync(project_root, rules))
}
```

### Pattern 4: Serde-Derived Types with YAML Frontmatter
**What:** `RuleFile` represents a `.ai/rules/*.md` file with YAML frontmatter parsed into `RuleMetadata`.
**When to use:** For the rules type system.
**Example:**
```rust
// RuleMetadata maps to YAML frontmatter fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleMetadata {
    pub description: Option<String>,
    #[serde(default)]
    pub globs: Vec<String>,
    #[serde(default = "default_true")]
    pub always_apply: bool,
}

// RuleFile combines metadata with content
#[derive(Debug, Clone)]
pub struct RuleFile {
    pub name: String,         // stem of filename
    pub metadata: RuleMetadata,
    pub content: String,      // body after frontmatter
    pub source_path: PathBuf, // .ai/rules/foo.md
}
```

### Anti-Patterns to Avoid
- **Breaking existing adapters:** Never add required (non-default) trait methods. All three new methods MUST have default implementations.
- **Putting parsing logic in types:** The types crate defines data shapes only. Parsing `.ai/rules/*.md` frontmatter or `.ai/mcp.toml` happens in aisync-core (Phase 13/14), not here.
- **Over-engineering SyncAction variants:** Use the minimum variants needed. Existing `CreateFile` and `RemoveFile` can be reused where they fit; only add new variants when semantic distinction matters (e.g., for display messages).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| YAML frontmatter parsing | Custom frontmatter parser | Defer to Phase 13 | Phase 12 defines types only; parsing is Phase 13's concern |
| TOML MCP config parsing | Custom TOML parser | `toml` crate (Phase 14) | Phase 12 defines `McpConfig`/`McpServer` types; parsing is Phase 14's concern |
| Serde derive boilerplate | Manual Serialize/Deserialize | `#[derive(Serialize, Deserialize)]` | Consistent with all existing types |

**Key insight:** Phase 12 is strictly about type definitions and trait signatures. No parsing, no file I/O, no business logic. Types compile and are importable; methods exist with no-op defaults. That is the entire scope.

## Common Pitfalls

### Pitfall 1: Adding Types That Don't Match Downstream Needs
**What goes wrong:** Types are defined in Phase 12 but turn out to be wrong shape for Phase 13/14/15.
**Why it happens:** Designing types without considering how they'll be used in adapters.
**How to avoid:** Design types by looking at what the downstream requirements actually need. RULES-01 needs frontmatter fields (description, globs, always_apply). MCP-01 needs server name, command, args, env. CMD-01 needs file path and content.
**Warning signs:** Types have fields that no requirement references, or requirements need fields the types lack.

### Pitfall 2: Making Trait Methods Too Specific
**What goes wrong:** Method signatures bake in assumptions that don't work for all adapters.
**Why it happens:** Designing for one adapter (e.g., Cursor) and not considering others (e.g., Windsurf, Plugin).
**How to avoid:** Method signatures should accept the canonical data and let each adapter decide how to transform it. Pass `&[RuleFile]` not `&str` of pre-formatted content.
**Warning signs:** An adapter implementation would need to undo transformations the caller already applied.

### Pitfall 3: Forgetting the Display Impl for New SyncAction Variants
**What goes wrong:** Code doesn't compile because `SyncAction` has a `Display` impl with a match that must be exhaustive.
**Why it happens:** Adding enum variants without updating the Display impl.
**How to avoid:** Every new SyncAction variant needs a Display arm. Follow the existing pattern of "Would [action]: [path]" messaging.
**Warning signs:** Compiler error on non-exhaustive match.

### Pitfall 4: Breaking the Plugin Variant
**What goes wrong:** New trait methods work for built-in adapters but fail for `Arc<dyn ToolAdapter>` (Plugin variant).
**Why it happens:** The Plugin variant uses dynamic dispatch via `Arc<dyn ToolAdapter>`. If the trait becomes non-object-safe, Plugin breaks.
**How to avoid:** Ensure new methods don't use generics or `Self`-returning types that prevent object safety. Use `&[RuleFile]` not `impl Iterator<Item = RuleFile>`.
**Warning signs:** "the trait `ToolAdapter` cannot be made into an object" compiler error.

### Pitfall 5: Circular Dependencies Between Crates
**What goes wrong:** `aisync-types` can't depend on `aisync-adapter` or `aisync-core`.
**Why it happens:** Putting trait-related types in the wrong crate.
**How to avoid:** Pure data types go in `aisync-types`. The trait goes in `aisync-adapter` (which depends on `aisync-types`). Adapter impls go in `aisync-core`.
**Warning signs:** `cargo build` fails with circular dependency error.

## Code Examples

### New Types in aisync-types (TYPE-01)

```rust
// Source: derived from REQUIREMENTS.md RULES-01, MCP-01, CMD-01

use std::path::PathBuf;
use std::collections::BTreeMap;
use serde::{Serialize, Deserialize};

/// Metadata from YAML frontmatter in a .ai/rules/*.md file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleMetadata {
    /// Human-readable description of what this rule covers.
    #[serde(default)]
    pub description: Option<String>,
    /// File glob patterns this rule applies to (empty = all files).
    #[serde(default)]
    pub globs: Vec<String>,
    /// Whether this rule always applies or only when globs match.
    #[serde(default = "default_true")]
    pub always_apply: bool,
}

/// A parsed rule file from .ai/rules/.
#[derive(Debug, Clone)]
pub struct RuleFile {
    /// Rule name (filename stem, e.g., "security" from "security.md").
    pub name: String,
    /// Parsed frontmatter metadata.
    pub metadata: RuleMetadata,
    /// Markdown body content (after frontmatter).
    pub content: String,
    /// Absolute path to the source file.
    pub source_path: PathBuf,
}

/// A single MCP server definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    /// Shell command to launch the server.
    pub command: String,
    /// Arguments to pass to the command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables (name -> value or ${VAR} reference).
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// Top-level MCP configuration from .ai/mcp.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// Named MCP server definitions.
    #[serde(default)]
    pub servers: BTreeMap<String, McpServer>,
}

/// A command file from .ai/commands/.
#[derive(Debug, Clone)]
pub struct CommandFile {
    /// Command name (filename stem).
    pub name: String,
    /// Full file content.
    pub content: String,
    /// Absolute path to the source file.
    pub source_path: PathBuf,
}
```

### New SyncAction Variants (TYPE-03)

```rust
// Added to existing SyncAction enum in aisync-types/src/lib.rs

// Rule sync actions
CreateRuleFile {
    output: PathBuf,
    content: String,
    rule_name: String,
},
// MCP sync actions
WriteMcpConfig {
    output: PathBuf,
    content: String,
},
// Command sync actions
CopyCommandFile {
    source: PathBuf,
    output: PathBuf,
    command_name: String,
},
// Warnings for unsupported dimensions
WarnUnsupportedDimension {
    tool: ToolKind,
    dimension: String,
    reason: String,
},
```

### New Trait Methods (TYPE-02)

```rust
// Added to ToolAdapter trait in aisync-adapter/src/lib.rs

/// Plan rule sync actions for this tool.
fn plan_rules_sync(
    &self,
    project_root: &Path,
    rules: &[RuleFile],
) -> Result<Vec<SyncAction>, AdapterError> {
    let _ = (project_root, rules);
    Ok(vec![])
}

/// Plan MCP config sync actions for this tool.
fn plan_mcp_sync(
    &self,
    project_root: &Path,
    mcp_config: &McpConfig,
) -> Result<Vec<SyncAction>, AdapterError> {
    let _ = (project_root, mcp_config);
    Ok(vec![])
}

/// Plan command sync actions for this tool.
fn plan_commands_sync(
    &self,
    project_root: &Path,
    commands: &[CommandFile],
) -> Result<Vec<SyncAction>, AdapterError> {
    let _ = (project_root, commands);
    Ok(vec![])
}
```

### AnyAdapter Dispatch (TYPE-04)

```rust
// Added to impl ToolAdapter for AnyAdapter in aisync-core/src/adapter.rs

fn plan_rules_sync(
    &self,
    project_root: &Path,
    rules: &[RuleFile],
) -> Result<Vec<SyncAction>, AdapterError> {
    dispatch_adapter!(self, a => a.plan_rules_sync(project_root, rules))
}

fn plan_mcp_sync(
    &self,
    project_root: &Path,
    mcp_config: &McpConfig,
) -> Result<Vec<SyncAction>, AdapterError> {
    dispatch_adapter!(self, a => a.plan_mcp_sync(project_root, mcp_config))
}

fn plan_commands_sync(
    &self,
    project_root: &Path,
    commands: &[CommandFile],
) -> Result<Vec<SyncAction>, AdapterError> {
    dispatch_adapter!(self, a => a.plan_commands_sync(project_root, commands))
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single `plan_sync` handles all content | Dimension-specific methods (rules, MCP, commands) | Phase 12 (now) | Each adapter can handle each sync dimension independently |
| `SyncAction` has instruction-focused variants only | `SyncAction` covers rules, MCP, and command dimensions | Phase 12 (now) | Display and execution can show dimension-appropriate messages |

**Unchanged:**
- The existing `plan_sync` method for instruction sync remains as-is. New methods are additive.
- No existing adapter code needs modification. All new methods have default no-op implementations.

## Open Questions

1. **Should `RuleFile` include raw bytes or parsed content?**
   - What we know: Phase 13 will parse YAML frontmatter from `.ai/rules/*.md`
   - What's unclear: Whether the `content` field should be the raw file or just the body after frontmatter
   - Recommendation: Store post-frontmatter body in `content` and structured metadata in `metadata`. This matches how Cursor's `read_instructions` already strips frontmatter.

2. **Should `McpServer` include a `transport` field?**
   - What we know: MCP-07 says "scopes to stdio transport only; warns when unsupported transport"
   - What's unclear: Whether to model transport in the type or assume stdio
   - Recommendation: Include an optional `transport` field defaulting to `"stdio"` for forward compatibility. Adapters can warn/skip non-stdio in Phase 14.

3. **Do we need a `CleanStaleFiles` action variant?**
   - What we know: RULES-07 and CMD-04 require cleanup of stale managed files
   - What's unclear: Whether `RemoveFile` (already exists) is sufficient or a specific variant is needed
   - Recommendation: Reuse existing `RemoveFile` variant. The sync engine can identify stale files; no special action type needed.

## Sources

### Primary (HIGH confidence)
- `aisync-types/src/lib.rs` - Full review of existing types (SyncAction, ToolKind, etc.)
- `aisync-adapter/src/lib.rs` - Full review of ToolAdapter trait and default impls
- `aisync-core/src/adapter.rs` - Full review of AnyAdapter dispatch pattern
- `aisync-core/src/adapters/cursor.rs` - Example adapter implementation
- `aisync-core/src/sync.rs` - SyncEngine orchestration showing how adapters are called
- `.planning/REQUIREMENTS.md` - TYPE-01 through TYPE-04, plus downstream RULES/MCP/CMD requirements

### Secondary (MEDIUM confidence)
- Rust trait object safety rules - well-known constraints on `dyn Trait`

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies, all existing workspace deps
- Architecture: HIGH - follows exact existing patterns (trait defaults, enum dispatch, macro)
- Pitfalls: HIGH - all pitfalls are standard Rust trait/enum patterns, verified against existing code
- Type design: MEDIUM - types are designed from requirements but may need minor adjustments when Phase 13/14/15 implement actual logic

**Research date:** 2026-03-09
**Valid until:** 2026-04-09 (stable - internal codebase, no external API changes)
