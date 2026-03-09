# Phase 6: Core Refactoring - Research

**Researched:** 2026-03-08
**Domain:** Rust trait design, enum dispatch, serde custom serialization, macro-based code generation
**Confidence:** HIGH

## Summary

Phase 6 is an internal refactoring of aisync-core to make adding new tool adapters a single-file operation. The current codebase has 5 categories of hardcoded match arms scattered across 12+ files: display names (6 duplicated `tool_display_name` functions), native file paths (in init.rs, diff.rs, watch.rs), conditional tags (conditional.rs), ToolKind-to-adapter construction (init.rs, sync.rs, watch.rs), and config field access (sync.rs `enabled_tools`). All of these must collapse into ToolAdapter trait methods and a BTreeMap-based config.

The refactoring is well-scoped: no new adapters are added, no public API changes for end users, and all existing tests provide a strong regression safety net. The main risks are the ToolKind `Copy` -> `Clone` migration (~40 usage sites) and the ToolsConfig struct field -> BTreeMap transition (which must preserve backward-compatible TOML deserialization).

**Primary recommendation:** Tackle in 3 waves: (1) ToolKind Custom(String) + Clone migration, (2) ToolAdapter trait method consolidation + dispatch_adapter! macro, (3) ToolsConfig BTreeMap migration. Each wave should compile and pass all tests before proceeding.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Extend ToolKind with `Custom(String)` variant for community/TOML-defined adapters
- Built-in tools (ClaudeCode, Cursor, OpenCode) remain as named enum variants; Windsurf and Codex will be added as named variants in Phase 7
- `Custom(String)` is reserved for Phase 10+ (declarative TOML adapters)
- ToolKind loses `Copy` derive -- migrate all ~40 usage sites to `Clone`
- Custom serialization: all variants serialize as lowercase strings ("claude-code", "cursor", "opencode"); Custom(s) serializes as the string itself
- All tool metadata moves into ToolAdapter trait methods -- this is what makes "one-file adapter" work
- New trait methods: `display_name()`, `native_instruction_path()`, `gitignore_entries()`, `conditional_tags()`, `watch_paths()`
- Eliminates hardcoded display names in 7 files (init.rs, status.rs, diff.rs, hooks.rs, sync.rs, check.rs, commands/diff.rs)
- Eliminates hardcoded native paths in init.rs, diff.rs, watch.rs
- Eliminates hardcoded conditional tags in conditional.rs
- Use a `dispatch_adapter!` macro to generate match arms for all ToolAdapter method impls on AnyAdapter
- Each new method or variant becomes one line instead of O(methods x variants) boilerplate
- Plugin variant uses `Box<dyn ToolAdapter>` for dynamic dispatch (built-in variants stay zero-cost)
- Replace ToolsConfig named fields (claude_code, cursor, opencode) with `BTreeMap<String, ToolConfig>`
- Well-known tools ("claude-code", "cursor", "opencode") are just well-known map keys -- no special treatment
- Existing aisync.toml files deserialize identically (same TOML table structure)
- Provide helper methods on ToolsConfig: `get_tool()`, `configured_tools()`, `is_enabled()` -- callers don't use raw map access

### Claude's Discretion
- Migration order within the phase (which refactor to tackle first)
- Whether to split into multiple plans or handle as one
- Exact macro syntax and error handling approach
- How to handle the `todo!()` default impls in ToolAdapter (keep, remove, or convert to proper defaults)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| REFAC-01 | ToolAdapter trait provides all tool metadata (display name, native paths, conditional tags, gitignore entries, watch paths) -- eliminating hardcoded match arms | Trait method inventory below; 12 files with hardcoded match arms identified; dispatch_adapter! macro pattern documented |
| REFAC-02 | ToolsConfig supports arbitrary tool names via BTreeMap with backward-compatible deserialization | BTreeMap<String, ToolConfig> with serde(flatten) pattern; migration path from named fields documented; helper method signatures defined |
| REFAC-03 | AnyAdapter enum includes Plugin variant for dynamic dispatch of SDK adapters | Plugin(Box<dyn ToolAdapter>) variant; requires ToolAdapter: Send + Sync for thread safety; dispatch macro handles both enum and dyn variants |
| REFAC-04 | Display name logic consolidated into single ToolAdapter method (6 duplications removed) | 6 identical `tool_display_name()` functions found in CLI commands (init.rs, status.rs, diff.rs, hooks.rs, sync.rs, check.rs); all replaced by `adapter.display_name()` |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1.0 | Custom Serialize/Deserialize for ToolKind, BTreeMap config | Already in workspace; custom impls needed for lowercase string serialization |
| toml | 0.8 | Config parsing with BTreeMap deserialization | Already in workspace; TOML tables naturally deserialize into BTreeMap |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| thiserror | 2.0 | Error types | Already in workspace; no changes needed |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| dispatch_adapter! custom macro | enum_dispatch crate | Custom macro is simpler, avoids new dependency, handles Plugin(Box<dyn>) variant which enum_dispatch doesn't support |
| BTreeMap<String, ToolConfig> | HashMap | BTreeMap gives deterministic serialization order; already used elsewhere in codebase |

**Installation:** No new dependencies needed. All changes use existing workspace crates.

## Architecture Patterns

### Recommended Migration Structure
```
crates/aisync-core/src/
├── types.rs           # ToolKind + Custom(String), custom Serialize/Deserialize
├── adapter.rs         # ToolAdapter trait (expanded), AnyAdapter + Plugin variant, dispatch_adapter! macro
├── config.rs          # ToolsConfig with BTreeMap<String, ToolConfig>, helper methods
├── adapters/
│   ├── claude_code.rs # Implements expanded ToolAdapter (display_name, native paths, etc.)
│   ├── cursor.rs      # Same
│   └── opencode.rs    # Same
├── conditional.rs     # Calls adapter.conditional_tags() instead of hardcoded match
├── diff.rs            # Calls adapter.native_instruction_path() instead of hardcoded match
├── watch.rs           # Calls adapter.watch_paths() instead of hardcoded match
├── init.rs            # Uses adapter registry instead of ToolKind match for construction
└── sync.rs            # Uses BTreeMap config iteration instead of per-field access
```

### Pattern 1: ToolAdapter Trait Expansion
**What:** Move all scattered metadata into trait methods with sensible defaults.
**When to use:** Every adapter must implement these; defaults allow incremental adoption.
```rust
pub trait ToolAdapter: Send + Sync {
    fn name(&self) -> ToolKind;
    fn display_name(&self) -> &str;
    fn native_instruction_path(&self) -> &str;  // e.g., "CLAUDE.md", ".cursor/rules/project.mdc"
    fn conditional_tags(&self) -> &[&str];       // e.g., &["claude-only", "claude-code-only"]
    fn gitignore_entries(&self) -> Vec<String> { vec![] }
    fn watch_paths(&self) -> Vec<&str> { vec![self.native_instruction_path()] }

    // Existing methods unchanged:
    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError>;
    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AisyncError>;
    fn plan_sync(&self, ...) -> Result<Vec<SyncAction>, AisyncError>;
    fn sync_status(&self, ...) -> Result<ToolSyncStatus, AisyncError>;
    fn plan_memory_sync(&self, ...) -> Result<Vec<SyncAction>, AisyncError>;
    fn translate_hooks(&self, ...) -> Result<HookTranslation, AisyncError>;
}
```

### Pattern 2: dispatch_adapter! Macro
**What:** Generates match arms for AnyAdapter's ToolAdapter impl, avoiding O(methods x variants) boilerplate.
**When to use:** In adapter.rs for the `impl ToolAdapter for AnyAdapter` block.
```rust
macro_rules! dispatch_adapter {
    ($self:expr, $inner:ident => $body:expr) => {
        match $self {
            AnyAdapter::ClaudeCode($inner) => $body,
            AnyAdapter::Cursor($inner) => $body,
            AnyAdapter::OpenCode($inner) => $body,
            AnyAdapter::Plugin($inner) => $body,
        }
    };
}

impl ToolAdapter for AnyAdapter {
    fn name(&self) -> ToolKind {
        dispatch_adapter!(self, a => a.name())
    }
    fn display_name(&self) -> &str {
        dispatch_adapter!(self, a => a.display_name())
    }
    // ... all other methods
}
```
Note: The Plugin variant holds `Box<dyn ToolAdapter>`, so `$inner` is a `&Box<dyn ToolAdapter>`. The method call `a.display_name()` works via auto-deref. The `&str` return from `display_name()` works because the trait object is borrowed, not owned.

### Pattern 3: ToolsConfig BTreeMap Migration
**What:** Replace named struct fields with a BTreeMap for arbitrary tool names.
**When to use:** In config.rs for ToolsConfig.
```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ToolsConfig {
    #[serde(flatten)]
    tools: BTreeMap<String, ToolConfig>,
}

impl ToolsConfig {
    pub fn get_tool(&self, name: &str) -> Option<&ToolConfig> {
        self.tools.get(name)
    }
    pub fn configured_tools(&self) -> impl Iterator<Item = (&str, &ToolConfig)> {
        self.tools.iter().map(|(k, v)| (k.as_str(), v))
    }
    pub fn is_enabled(&self, name: &str) -> bool {
        self.tools.get(name).is_none_or(|tc| tc.enabled)
    }
}
```
TOML `[tools.claude-code]` naturally deserializes into a BTreeMap entry with key `"claude-code"`.

### Pattern 4: ToolKind Custom Serialization
**What:** Custom Serialize/Deserialize for ToolKind to produce lowercase hyphenated strings.
```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ToolKind {
    ClaudeCode,
    Cursor,
    OpenCode,
    Custom(String),
}

impl Serialize for ToolKind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ToolKind::ClaudeCode => serializer.serialize_str("claude-code"),
            ToolKind::Cursor => serializer.serialize_str("cursor"),
            ToolKind::OpenCode => serializer.serialize_str("opencode"),
            ToolKind::Custom(s) => serializer.serialize_str(s),
        }
    }
}

impl<'de> Deserialize<'de> for ToolKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "claude-code" => Ok(ToolKind::ClaudeCode),
            "cursor" => Ok(ToolKind::Cursor),
            "opencode" => Ok(ToolKind::OpenCode),
            other => Ok(ToolKind::Custom(other.to_string())),
        }
    }
}
```

### Pattern 5: Adapter Registry
**What:** Replace ToolKind->AnyAdapter match arms with a registry function.
```rust
impl AnyAdapter {
    pub fn for_tool(kind: &ToolKind) -> Option<AnyAdapter> {
        match kind {
            ToolKind::ClaudeCode => Some(AnyAdapter::ClaudeCode(ClaudeCodeAdapter)),
            ToolKind::Cursor => Some(AnyAdapter::Cursor(CursorAdapter)),
            ToolKind::OpenCode => Some(AnyAdapter::OpenCode(OpenCodeAdapter)),
            ToolKind::Custom(_) => None, // No built-in adapter; Phase 10+ handles this
        }
    }

    pub fn all_builtin() -> Vec<AnyAdapter> { /* existing all() renamed */ }
}
```

### Anti-Patterns to Avoid
- **Matching on ToolKind for metadata outside adapter code:** After this refactoring, any `match tool_kind { ClaudeCode => "Claude Code", ... }` is a code smell. All metadata comes from the adapter.
- **Accessing ToolsConfig BTreeMap directly in callers:** Always use helper methods (`get_tool()`, `configured_tools()`, `is_enabled()`). Direct map access couples callers to internal representation.
- **Adding Plugin variant without Send + Sync bounds:** The watch engine runs adapters across thread boundaries. The Plugin variant's `Box<dyn ToolAdapter>` must be `Box<dyn ToolAdapter + Send + Sync>`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Enum dispatch for multiple methods | Copy-paste match arms for each new method/variant | `dispatch_adapter!` macro | Currently 7 match blocks x 3 variants = 21 arms; adding Plugin makes it 28; each new method adds 4 more |
| ToolKind string conversion | Manual `to_string()`/`from_str()` methods alongside serde | Custom Serialize/Deserialize impls | Single source of truth for string representation; used by both TOML config and Display |
| Config field access abstraction | Wrapper functions in every caller | Helper methods on ToolsConfig | Encapsulates BTreeMap so callers never see the internal representation |

**Key insight:** The current codebase has ~186 non-test ToolKind references. Manual migration is tractable but error-prone. The macro and helper methods ensure that adding Phase 7's Windsurf/Codex adapters requires zero changes outside their own adapter files.

## Common Pitfalls

### Pitfall 1: ToolKind Copy Removal Breaks Implicit Copies
**What goes wrong:** Removing `Copy` from ToolKind causes compile errors everywhere the enum is implicitly copied (field access from borrowed structs, pattern matching, function arguments).
**Why it happens:** `Copy` is used implicitly in ~40 places: `result.tool` in iterators, `tool_kind` passed by value to functions, struct field access in borrowed contexts.
**How to avoid:** Add `.clone()` at every usage site. Run `cargo check` after each file change. Most are trivial (add `.clone()`), but some require changing function signatures from `ToolKind` to `&ToolKind`.
**Warning signs:** The compiler will catch all of these; no silent failures possible.

### Pitfall 2: BTreeMap Deserialization Changes TOML Behavior
**What goes wrong:** Existing `[tools.claude-code]` TOML sections might not deserialize identically if the BTreeMap key format differs.
**Why it happens:** The current `ToolsConfig` uses `#[serde(rename = "claude-code")]` on the `claude_code` field. With BTreeMap + `#[serde(flatten)]`, the TOML key "claude-code" becomes the map key directly.
**How to avoid:** Write explicit round-trip tests: parse existing TOML fixtures, serialize back, verify identical structure. The BTreeMap approach should be strictly compatible since TOML tables map naturally to maps.
**Warning signs:** Test `test_parse_full_config` and `test_round_trip` will catch regressions.

### Pitfall 3: Plugin Variant Lifetime Issues with &str Returns
**What goes wrong:** `display_name() -> &str` requires the returned string to live as long as the adapter. For `Plugin(Box<dyn ToolAdapter>)`, this means the implementing type must own the string.
**Why it happens:** Built-in adapters return `&'static str` (e.g., "Claude Code"). Plugin adapters will store display names as `String` fields and return `&self.display_name`.
**How to avoid:** This is fine for Phase 6 since no Plugin adapters are instantiated yet. The trait signature `fn display_name(&self) -> &str` works because the borrow is tied to `&self`. Document this for Phase 10+ implementers.
**Warning signs:** Compile errors if a Plugin adapter tries to construct and return a String on the fly.

### Pitfall 4: enabled_tools() Refactor Must Preserve "Unconfigured = Enabled" Semantics
**What goes wrong:** The current `enabled_tools()` treats tools not present in config as enabled (they still sync). The BTreeMap version must preserve this: if a tool has a built-in adapter but no config entry, it's enabled by default.
**Why it happens:** Current logic: `config.tools.claude_code.as_ref().is_none_or(|tc| tc.enabled)`. With BTreeMap, a missing key means the tool is unconfigured but should still be iterated.
**How to avoid:** The new `enabled_tools()` iterates ALL built-in adapters, checking `config.tools.is_enabled(adapter_name)` where `is_enabled` returns `true` for missing keys. Write explicit test for this behavior.
**Warning signs:** Tests that create configs with only some tools configured would break.

### Pitfall 5: ToolKind in Struct Fields Used for Serialization
**What goes wrong:** ToolKind appears in serialized types (ToolSyncStatus, ToolDiff, HookTranslation, SyncAction, etc.). Changing serialization format could break JSON output or internal comparisons.
**Why it happens:** Types like `ToolSyncResult { tool: ToolKind }` are `#[derive(Serialize)]`. The custom serialization will change output from `"ClaudeCode"` to `"claude-code"`.
**How to avoid:** This is actually the desired behavior (cleaner output). But verify no code does string comparison against the old format. Check existing JSON fixtures or snapshot tests.
**Warning signs:** Integration tests that check JSON output format.

### Pitfall 6: dispatch_adapter! Macro and Plugin Variant Type Mismatch
**What goes wrong:** The macro generates identical match arms for all variants, but `Plugin(Box<dyn ToolAdapter>)` is a different type than `ClaudeCode(ClaudeCodeAdapter)`. The bound variable `a` is `&ClaudeCodeAdapter` vs `&Box<dyn ToolAdapter>`.
**Why it happens:** Rust match arms bind the inner value by reference. For Plugin, `a` is `&Box<dyn ToolAdapter>`, and method calls auto-deref through `Box` and `dyn` to reach the trait method.
**How to avoid:** This works naturally in Rust due to auto-deref. Test by adding a `Plugin(Box::new(ClaudeCodeAdapter))` test case.
**Warning signs:** None expected; this is a well-understood Rust pattern.

## Code Examples

### ToolKind Custom Serialization (verified pattern from serde docs)
```rust
use serde::{Serialize, Deserialize, Serializer, Deserializer};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ToolKind {
    ClaudeCode,
    Cursor,
    OpenCode,
    Custom(String),
}

impl ToolKind {
    /// Returns the canonical string key for this tool (used in config, display, serialization).
    pub fn as_str(&self) -> &str {
        match self {
            ToolKind::ClaudeCode => "claude-code",
            ToolKind::Cursor => "cursor",
            ToolKind::OpenCode => "opencode",
            ToolKind::Custom(s) => s.as_str(),
        }
    }
}

impl fmt::Display for ToolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
```

### Expanded ToolAdapter with New Methods
```rust
pub trait ToolAdapter: Send + Sync {
    fn name(&self) -> ToolKind;

    /// Human-readable display name (e.g., "Claude Code", "Cursor", "OpenCode").
    fn display_name(&self) -> &str;

    /// Relative path from project root to the tool's native instruction file.
    fn native_instruction_path(&self) -> &str;

    /// Conditional tag names that match this tool (e.g., ["claude-only", "claude-code-only"]).
    fn conditional_tags(&self) -> &[&str] { &[] }

    /// Entries to add to .gitignore when this tool is synced.
    fn gitignore_entries(&self) -> Vec<String> { vec![] }

    /// Relative paths to watch for reverse sync (defaults to native_instruction_path).
    fn watch_paths(&self) -> Vec<&str> {
        vec![self.native_instruction_path()]
    }

    /// Default sync strategy for this tool (overridable in config).
    fn default_sync_strategy(&self) -> SyncStrategy {
        SyncStrategy::Symlink
    }

    // ... existing methods remain
}
```

### ClaudeCodeAdapter with New Methods
```rust
impl ToolAdapter for ClaudeCodeAdapter {
    fn name(&self) -> ToolKind { ToolKind::ClaudeCode }
    fn display_name(&self) -> &str { "Claude Code" }
    fn native_instruction_path(&self) -> &str { "CLAUDE.md" }
    fn conditional_tags(&self) -> &[&str] { &["claude-only", "claude-code-only"] }
    // ... existing methods
}
```

### BTreeMap ToolsConfig with Helper Methods
```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ToolsConfig {
    #[serde(flatten)]
    tools: BTreeMap<String, ToolConfig>,
}

impl ToolsConfig {
    pub fn get_tool(&self, name: &str) -> Option<&ToolConfig> {
        self.tools.get(name)
    }

    pub fn configured_tools(&self) -> impl Iterator<Item = (&str, &ToolConfig)> {
        self.tools.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        self.tools.get(name).is_none_or(|tc| tc.enabled)
    }

    pub fn set_tool(&mut self, name: String, config: ToolConfig) {
        self.tools.insert(name, config);
    }
}
```

### Refactored enabled_tools()
```rust
pub(crate) fn enabled_tools(
    config: &AisyncConfig,
) -> Vec<(ToolKind, AnyAdapter, Option<&ToolConfig>)> {
    let mut tools = Vec::new();
    for adapter in AnyAdapter::all_builtin() {
        let key = adapter.name().as_str().to_string();
        if config.tools.is_enabled(&key) {
            tools.push((
                adapter.name(),
                adapter,
                config.tools.get_tool(&key),
            ));
        }
    }
    tools
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `#[derive(Copy)]` on ToolKind | `Clone`-only with `Custom(String)` | This phase | ~40 usage sites need `.clone()` |
| Named struct fields in ToolsConfig | `BTreeMap<String, ToolConfig>` with `#[serde(flatten)]` | This phase | Enables arbitrary tool names in TOML |
| Manual match arms in AnyAdapter | `dispatch_adapter!` macro | This phase | Adding a variant is one line per variant, not O(methods) |
| `tool_display_name()` functions in 6 CLI files | `adapter.display_name()` trait method | This phase | Single source of truth |

## Open Questions

1. **todo!() Default Impls in ToolAdapter**
   - What we know: `read_instructions()`, `plan_sync()`, `sync_status()` currently have `todo!()` defaults that panic at runtime.
   - What's unclear: Should these become proper error-returning defaults, or should they remain as `todo!()` to catch unimplemented adapters?
   - Recommendation: Convert to proper defaults that return `Ok(None)` / `Ok(vec![])` / `Ok(ToolSyncStatus { drift: DriftState::NotConfigured })`. This is safer for Plugin adapters that may not implement all methods. The `todo!()` pattern is a development convenience that becomes a landmine in production.

2. **AnyAdapter::all() vs all_builtin() Naming**
   - What we know: Current `all()` returns all three built-in adapters. Phase 10+ will add Plugin adapters.
   - What's unclear: Should `all()` be renamed to `all_builtin()` now, or keep `all()` and add `all_including_plugins()` later?
   - Recommendation: Rename to `all_builtin()` now. Clear naming prevents confusion when plugins arrive.

3. **ToolKind Eq/Hash with Custom(String)**
   - What we know: `Custom("claude-code")` and `ToolKind::ClaudeCode` are semantically equivalent but `!=` under derived Eq.
   - What's unclear: Should we normalize in the Deserialize impl (always return the named variant when a well-known string is parsed)?
   - Recommendation: Yes, the Deserialize impl should normalize. `Custom(String)` should only hold truly custom tool names, never well-known ones. This prevents subtle bugs in HashMap lookups.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test + tempfile 3.14 |
| Config file | None (standard `#[cfg(test)]` modules) |
| Quick run command | `cargo test -p aisync-core` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REFAC-01 | ToolAdapter provides all metadata; no hardcoded match arms outside adapters | unit | `cargo test -p aisync-core -- adapter` | Partial -- adapter.rs tests exist, new trait method tests needed |
| REFAC-02 | ToolsConfig BTreeMap deserialization with backward compat | unit | `cargo test -p aisync-core -- config` | Yes -- existing round-trip tests cover compat |
| REFAC-03 | AnyAdapter Plugin variant dispatches to dyn ToolAdapter | unit | `cargo test -p aisync-core -- adapter::plugin` | No -- Wave 0 gap |
| REFAC-04 | display_name() from single trait method, 6 duplications removed | unit + grep | `cargo test -p aisync-core -- display_name` | No -- Wave 0 gap |

### Sampling Rate
- **Per task commit:** `cargo test -p aisync-core`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before verification

### Wave 0 Gaps
- [ ] Test for Plugin variant dispatch (adapter.rs) -- covers REFAC-03
- [ ] Test for display_name() trait method on each adapter -- covers REFAC-04
- [ ] Test for ToolKind Custom(String) serialization round-trip -- covers REFAC-01
- [ ] Test for BTreeMap ToolsConfig with unknown tool names (e.g., "windsurf") -- covers REFAC-02
- [ ] Test for enabled_tools() with BTreeMap config preserving unconfigured-is-enabled semantics

## Sources

### Primary (HIGH confidence)
- Direct codebase analysis of all 16 files containing ToolKind references
- serde documentation for custom Serialize/Deserialize impls (well-established pattern)
- Rust reference for macro_rules! dispatch patterns

### Secondary (MEDIUM confidence)
- BTreeMap + `#[serde(flatten)]` for TOML table deserialization (widely used pattern, verified against toml 0.8 behavior in codebase)

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - No new dependencies; all patterns use existing workspace crates
- Architecture: HIGH - All code analyzed directly; patterns are well-established Rust idioms
- Pitfalls: HIGH - Derived from actual code analysis of all affected files; compiler enforces correctness

**Research date:** 2026-03-08
**Valid until:** 2026-04-08 (stable Rust patterns, no external dependencies changing)
