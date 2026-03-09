# Phase 10: Declarative TOML Adapters - Research

**Researched:** 2026-03-08
**Domain:** TOML-driven adapter schema, deserialization, template interpolation, plugin discovery
**Confidence:** HIGH

## Summary

Phase 10 enables users to define new tool adapters via TOML files in `.ai/adapters/` without writing Rust. This requires three components: (1) a TOML schema that captures adapter metadata (detection rules, file mappings, sync strategy, templates), (2) a `DeclarativeAdapter` struct that implements `ToolAdapter` from parsed TOML, and (3) auto-discovery of `.ai/adapters/*.toml` files during sync/status operations.

The existing codebase is well-prepared for this. The `ToolAdapter` trait (in `aisync-adapter`) has sensible defaults for all optional methods. The `AnyAdapter::Plugin(Arc<dyn ToolAdapter>)` variant already exists for dynamic dispatch. The `enabled_tools()` method in `SyncEngine` currently only iterates `AnyAdapter::all_builtin()` -- this needs extension to include discovered TOML adapters. The `toml` crate (0.8) already in workspace dependencies handles all deserialization needs. For template interpolation, a simple `{{content}}` string replacement suffices (matching the Windsurf/Cursor pattern of frontmatter + canonical content).

**Primary recommendation:** Build `DeclarativeAdapter` in `aisync-core` (not `aisync-adapter`) since it depends on filesystem discovery and template rendering. Use `toml` 0.8 for deserialization. Use simple string interpolation (`{{content}}`) -- no need for a full template engine.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SDK-03 | Declarative TOML adapter schema supports detection rules, file mappings, sync strategy, and templates | TOML schema design section; all ToolAdapter trait methods map to TOML fields |
| SDK-04 | DeclarativeAdapter struct implements ToolAdapter from parsed TOML definitions | Architecture pattern: DeclarativeAdapter wraps DeclarativeAdapterDef, implements ToolAdapter via delegation |
| SDK-05 | `.ai/adapters/*.toml` files auto-discovered and loaded as plugin adapters | Discovery pattern: scan directory, parse TOML, wrap in AnyAdapter::Plugin, inject into enabled_tools |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| toml | 0.8 | Parse adapter TOML definitions | Already in workspace, serde-based, handles all TOML features |
| serde | 1.0 | Deserialize TOML into Rust structs | Already in workspace, derive macros for clean schema definition |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| (none needed) | - | Template interpolation | Simple `str::replace` with `{{content}}` placeholder suffices |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Simple `{{content}}` | tera/handlebars | Full template engine is overkill; only need content interpolation. Adds dependency for one feature. |
| DeclarativeAdapter in aisync-core | DeclarativeAdapter in aisync-adapter | aisync-adapter is the public SDK crate; adding filesystem discovery and toml dependency there pollutes the minimal SDK surface |

## Architecture Patterns

### Recommended Project Structure
```
crates/aisync-core/src/
  declarative.rs        # DeclarativeAdapterDef (TOML schema), DeclarativeAdapter (ToolAdapter impl), discovery fn
  adapter.rs            # Existing -- no changes to AnyAdapter enum needed (uses Plugin variant)
  sync.rs               # Extend enabled_tools() to include discovered TOML adapters
  detection.rs          # Extend scan() to include TOML adapter detection results
```

### Pattern 1: TOML Adapter Definition Schema

**What:** A serde-deserializable struct that captures all adapter metadata from a TOML file.
**When to use:** Every `.ai/adapters/*.toml` file is parsed into this struct.

Example TOML file (`.ai/adapters/aider.toml`):
```toml
# Adapter metadata
name = "aider"
display_name = "Aider"

# Detection rules -- what filesystem markers indicate this tool is present
[detection]
directories = [".aider"]
files = [".aider.conf.yml"]
match_any = true  # true = any marker sufficient, false = all required

# Sync configuration
[sync]
strategy = "symlink"                      # symlink | copy | generate
instruction_path = ".aider.conf.yml"      # relative path for native instruction file
conditional_tags = ["aider-only"]         # optional conditional section tags
gitignore_entries = [".aider.conf.yml"]   # optional gitignore entries

# Template for generate strategy (optional, only used when strategy = "generate")
[template]
content = """
---
description: Project instructions synced by aisync
---

{{content}}
"""
# frontmatter_strip = "---"  # optional: how to strip frontmatter when reading back
```

Corresponding Rust struct:
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct DeclarativeAdapterDef {
    pub name: String,
    pub display_name: String,
    #[serde(default)]
    pub detection: DetectionDef,
    #[serde(default)]
    pub sync: SyncDef,
    #[serde(default)]
    pub template: Option<TemplateDef>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DetectionDef {
    #[serde(default)]
    pub directories: Vec<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default = "default_true")]
    pub match_any: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyncDef {
    #[serde(default)]
    pub strategy: SyncStrategy,
    pub instruction_path: String,
    #[serde(default)]
    pub conditional_tags: Vec<String>,
    #[serde(default)]
    pub gitignore_entries: Vec<String>,
    #[serde(default)]
    pub watch_paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TemplateDef {
    pub content: String,
    pub frontmatter_strip: Option<String>,
}
```

### Pattern 2: DeclarativeAdapter Implements ToolAdapter

**What:** A struct that holds a `DeclarativeAdapterDef` and implements `ToolAdapter` by delegating to the parsed definition.
**When to use:** After parsing TOML, wrap in DeclarativeAdapter for trait dispatch.

Key implementation considerations:
- `name()` returns `ToolKind::Custom(def.name.clone())`
- `display_name()` returns `&self.display_name` (stored on struct, not def, for lifetime reasons)
- `detect()` checks `detection.directories` and `detection.files` against project_root
- `plan_sync()` for Generate strategy uses `template.content.replace("{{content}}", canonical_content)`
- `plan_sync()` for Symlink/Copy strategies reuse the same logic as ClaudeCode/Codex adapters
- `conditional_tags()` returns references to stored Vec -- needs `Vec<String>` stored on struct, return `&[&str]` via a cached field or `Vec<&str>` computed on demand

**Lifetime challenge:** `ToolAdapter::conditional_tags()` returns `&[&str]`, but DeclarativeAdapter has `Vec<String>`. Solutions:
1. Store a `Vec<&'static str>` by leaking strings (unacceptable for user-defined adapters)
2. Change trait signature to return `Vec<String>` (breaking change to SDK trait)
3. Store a precomputed `Vec<String>` and a corresponding `Vec<&str>` that borrows from it (self-referential, unsafe)
4. **Recommended:** Store both the `Vec<String>` and a leaked `Box<[&str]>` that is computed once at construction. Since TOML adapters are loaded once and live for the program lifetime, the small leak is acceptable.
5. **Alternative (simpler):** Change `conditional_tags()` trait method return type from `&[&str]` to `Vec<String>`. This is a breaking change to `aisync-adapter` but since it's pre-1.0 and the only external consumers would be v1.1 SDK users, it's acceptable. This is the cleanest approach.

### Pattern 3: Auto-Discovery of TOML Adapter Files

**What:** On `aisync sync` and `aisync status`, scan `.ai/adapters/` for `*.toml` files, parse each, and register as plugin adapters.
**When to use:** Every sync/status invocation.

Discovery flow:
1. `discover_toml_adapters(project_root: &Path) -> Vec<DeclarativeAdapter>` scans `.ai/adapters/*.toml`
2. Each parsed adapter is wrapped in `AnyAdapter::Plugin(Arc::new(adapter))`
3. `SyncEngine::enabled_tools()` is extended: after builtin adapters, append discovered TOML adapters (filtered by `config.tools.is_enabled()`)
4. `DetectionEngine::scan()` is extended similarly for detection

Integration points:
- `SyncEngine::enabled_tools()` -- add TOML adapters after builtins
- `DetectionEngine::scan()` -- include TOML adapters in scanning
- Both need access to `project_root` (currently `enabled_tools` only takes `config`)

### Anti-Patterns to Avoid
- **Over-engineering the template system:** Do not add Tera/Handlebars. `{{content}}` replacement is sufficient. If users need more, they should write a Rust adapter.
- **Storing DeclarativeAdapter in aisync-adapter crate:** Keep the TOML parsing and filesystem discovery in aisync-core. The SDK crate should remain minimal.
- **Making TOML adapters override builtin adapters:** TOML adapters should only create Custom tool kinds. If a TOML file tries to define `name = "cursor"`, it should be rejected or ignored.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TOML parsing | Custom parser | `toml` 0.8 with serde derives | Already in workspace, handles all edge cases |
| Template rendering | Custom template engine | `str::replace("{{content}}", ...)` | Only one variable needed; full engine is 10x more code for zero benefit |
| Glob matching for adapter discovery | Custom directory walker | `std::fs::read_dir` + `.toml` extension filter | Simple flat directory scan, no recursion needed |

**Key insight:** The declarative adapter is essentially a data-driven implementation of an existing trait. All the complexity is already handled by the trait's method signatures and the sync engine's orchestration. The TOML layer is just configuration.

## Common Pitfalls

### Pitfall 1: Lifetime Mismatch on conditional_tags()
**What goes wrong:** `ToolAdapter::conditional_tags()` returns `&[&str]` but DeclarativeAdapter stores `Vec<String>`. Cannot return references to temporary `Vec<&str>`.
**Why it happens:** The trait was designed for static adapters where tags are compile-time constants.
**How to avoid:** Either change the trait signature (recommended since pre-1.0) or use the construction-time leak pattern. Decide before implementation.
**Warning signs:** Compiler errors about returning references to temporaries.

### Pitfall 2: enabled_tools() Signature Change
**What goes wrong:** `SyncEngine::enabled_tools()` currently takes only `&AisyncConfig` and doesn't know about project_root for TOML discovery.
**Why it happens:** TOML adapters live in the project directory, not in the config.
**How to avoid:** Add `project_root: &Path` parameter to `enabled_tools()`, or pass discovered adapters as a separate parameter. The cleanest approach is to discover adapters at the call site and pass them in.
**Warning signs:** Awkward parameter threading.

### Pitfall 3: Name Collision with Builtins
**What goes wrong:** A TOML adapter defines `name = "claude-code"`, causing conflicts with the builtin ClaudeCode adapter.
**Why it happens:** No validation that TOML adapter names don't collide with builtins.
**How to avoid:** During discovery, reject (with warning) any TOML adapter whose name matches a known ToolKind variant (`claude-code`, `cursor`, `opencode`, `windsurf`, `codex`).
**Warning signs:** Duplicate tools in status output, panics in deduplication.

### Pitfall 4: Template Without Generate Strategy
**What goes wrong:** User defines a `[template]` section but sets `strategy = "symlink"`. Template is silently ignored.
**Why it happens:** No validation that template section requires generate strategy.
**How to avoid:** Log a warning during discovery if `template` is present but strategy is not `generate`. Or auto-upgrade strategy to `generate` when template is present.

### Pitfall 5: Instruction Path Must Be Unique Per Adapter
**What goes wrong:** Two TOML adapters target the same `instruction_path`, causing file conflicts.
**Why it happens:** No cross-adapter validation during discovery.
**How to avoid:** The existing `deduplicate_actions()` in SyncEngine already handles path conflicts at the action level. No additional code needed -- first-adapter-wins semantics apply.

## Code Examples

### Example 1: TOML Adapter Definition File
```toml
# .ai/adapters/continue.toml
name = "continue"
display_name = "Continue"

[detection]
directories = [".continue"]
files = [".continue/config.json"]
match_any = true

[sync]
strategy = "symlink"
instruction_path = ".continue/rules/project.md"
conditional_tags = ["continue-only"]
gitignore_entries = [".continue/rules/project.md"]
```

### Example 2: TOML Adapter with Template (Generate Strategy)
```toml
# .ai/adapters/pearai.toml
name = "pearai"
display_name = "PearAI"

[detection]
directories = [".pearai"]

[sync]
strategy = "generate"
instruction_path = ".pearai/rules/project.md"
conditional_tags = ["pearai-only"]
gitignore_entries = [".pearai/rules/"]

[template]
content = """---
type: project_rules
description: Project instructions synced by aisync
---

{{content}}
"""
frontmatter_strip = "---"
```

### Example 3: Discovery Function
```rust
pub fn discover_toml_adapters(project_root: &Path) -> Vec<DeclarativeAdapter> {
    let adapters_dir = project_root.join(".ai/adapters");
    if !adapters_dir.is_dir() {
        return vec![];
    }

    let mut adapters = Vec::new();
    let entries = match std::fs::read_dir(&adapters_dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "toml") {
            match load_toml_adapter(&path) {
                Ok(adapter) => adapters.push(adapter),
                Err(e) => eprintln!("Warning: failed to load adapter {}: {e}", path.display()),
            }
        }
    }
    adapters
}
```

### Example 4: DeclarativeAdapter::detect() Implementation
```rust
fn detect(&self, project_root: &Path) -> Result<DetectionResult, AdapterError> {
    let mut markers = Vec::new();

    for dir in &self.def.detection.directories {
        let path = project_root.join(dir);
        if path.is_dir() {
            markers.push(path);
        }
    }
    for file in &self.def.detection.files {
        let path = project_root.join(file);
        if path.exists() {
            markers.push(path);
        }
    }

    let detected = if self.def.detection.match_any {
        !markers.is_empty()
    } else {
        let total = self.def.detection.directories.len() + self.def.detection.files.len();
        markers.len() == total
    };

    Ok(DetectionResult {
        tool: self.name(),
        detected,
        confidence: Confidence::Medium,  // TOML adapters are Medium confidence
        markers_found: markers,
        version_hint: None,
    })
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Hardcoded adapters only | Plugin variant for dynamic dispatch | Phase 6 (REFAC-03) | AnyAdapter::Plugin already exists |
| Static conditional tags | Adapter-provided tags | Phase 6 (REFAC-01) | Tags come from ToolAdapter trait, not hardcoded |
| Only builtin tool kinds | Custom(String) variant | Phase 6 (REFAC-02) | ToolKind supports arbitrary tool names |

**Deprecated/outdated:**
- None relevant. The codebase is modern and well-factored for this extension.

## Open Questions

1. **conditional_tags() return type**
   - What we know: Current signature is `&[&str]`, which works for static adapters but not for dynamic ones with `Vec<String>` storage.
   - What's unclear: Whether changing the trait signature is acceptable vs. using a leak pattern.
   - Recommendation: Change the trait to return `Vec<&str>` (computed on demand). Pre-1.0 crate, minimal breaking change. The 5 built-in adapters need a trivial update (`.to_vec()`). Alternatively, since this returns only a few strings, leaking at construction time is also fine.

2. **Where to trigger discovery**
   - What we know: Discovery must happen before `enabled_tools()` and `DetectionEngine::scan()`.
   - What's unclear: Whether to discover once and cache, or re-discover on each invocation.
   - Recommendation: Discover per-invocation (TOML files are tiny, scan is fast). No caching needed for v1.1. Pass discovered adapters as a parameter to `enabled_tools()`.

3. **Error handling for malformed TOML adapter files**
   - What we know: Malformed files should not crash the program.
   - What's unclear: Whether to warn and skip, or fail the entire sync.
   - Recommendation: Warn and skip. Log a clear message with the file path and error. Other adapters (both builtin and TOML) continue normally.

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `crates/aisync-adapter/src/lib.rs` -- ToolAdapter trait definition (13 methods, 6 with defaults)
- Codebase analysis: `crates/aisync-core/src/adapter.rs` -- AnyAdapter enum with Plugin variant, dispatch macro
- Codebase analysis: `crates/aisync-core/src/sync.rs` -- SyncEngine::enabled_tools(), plan_all_internal(), deduplicate_actions()
- Codebase analysis: `crates/aisync-core/src/adapters/windsurf.rs` -- Generate strategy pattern with frontmatter template
- Codebase analysis: `crates/aisync-core/src/adapters/codex.rs` -- Symlink strategy pattern
- Codebase analysis: `crates/aisync-types/src/lib.rs` -- SyncStrategy, ToolKind::Custom, SyncAction variants
- Codebase analysis: `Cargo.toml` workspace -- toml 0.8, serde 1.0 already available

### Secondary (MEDIUM confidence)
- toml crate documentation -- serde integration for nested table deserialization with defaults (verified via existing usage in config.rs)

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all libraries already in workspace, no new dependencies needed
- Architecture: HIGH - existing Plugin variant, dispatch macro, and adapter patterns provide clear blueprint
- Pitfalls: HIGH - derived from direct codebase analysis of trait signatures and method contracts

**Research date:** 2026-03-08
**Valid until:** 2026-04-08 (stable domain, no external dependencies changing)
