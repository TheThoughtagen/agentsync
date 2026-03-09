# Phase 11: Compile-Time Registration - Research

**Researched:** 2026-03-08
**Domain:** Rust compile-time plugin registration, community adapter SDK, documentation
**Confidence:** HIGH

## Summary

Phase 11 adds compile-time adapter registration via the `inventory` crate so that community Rust adapter crates can be picked up by the `aisync` binary without modifying any source in the main repository. The second deliverable is documentation for both TOML and Rust adapter authoring paths.

The `inventory` crate (v0.3.22, maintained by dtolnay) provides exactly the distributed plugin registration pattern needed. It uses linker-section tricks to collect static registrations across crate boundaries at program startup. The key constraint is that `inventory::Collect` requires `Sized + Sync + 'static`, which means we cannot collect `dyn ToolAdapter` directly. The standard pattern is to collect a concrete wrapper type (e.g., `AdapterFactory`) that produces `Box<dyn ToolAdapter>` or `Arc<dyn ToolAdapter>`. The `aisync-adapter` crate already defines the `ToolAdapter` trait with `Send + Sync` bounds, making it compatible.

**Primary recommendation:** Add `inventory` as a dependency to `aisync-adapter`, define an `AdapterFactory` struct that wraps a constructor function, call `inventory::collect!` in `aisync-adapter`, and have `aisync-core` iterate `inventory::iter::<AdapterFactory>` alongside builtins and TOML adapters in `enabled_tools()` and `DetectionEngine::scan()`.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SDK-06 | Compile-time registration via `inventory` crate for community Rust adapter crates | `inventory` 0.3.22 supports this exact pattern. `AdapterFactory` wrapper struct collected via `inventory::collect!`, iterated via `inventory::iter`. Community crates use `inventory::submit!` macro. |
| SDK-07 | Documentation for community adapter authoring (both TOML and Rust paths) | Two authoring paths well-defined: TOML (drop `.toml` in `.ai/adapters/`) and Rust (implement `ToolAdapter`, use `inventory::submit!`). Working examples from existing code. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| inventory | 0.3.22 | Typed distributed plugin registration | dtolnay crate, zero dependencies, battle-tested, supports Linux/macOS/Windows/WASM |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| aisync-adapter | 0.1.0 (workspace) | ToolAdapter trait + AdapterFactory type | Where `inventory::collect!` is called and community crates depend |
| aisync-types | 0.1.0 (workspace) | Shared types (ToolKind, SyncStrategy, etc.) | Re-exported through aisync-adapter |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| inventory | linkme | linkme uses `#[distributed_slice]` attribute -- similar mechanism but less widely adopted; inventory has simpler API for this use case |
| inventory | Manual static registration | Requires modifying main repo for each adapter -- exactly what we want to avoid |
| Wrapper struct (AdapterFactory) | Collect Box\<dyn ToolAdapter\> directly | Not possible: `inventory::Collect` requires `Sized`, trait objects are `!Sized` |

**Installation:**
```toml
# In crates/aisync-adapter/Cargo.toml
[dependencies]
inventory = "0.3"
```

## Architecture Patterns

### Recommended Integration Points
```
crates/
├── aisync-adapter/       # inventory::collect!(AdapterFactory) lives here
│   └── src/lib.rs        # AdapterFactory struct + ToolAdapter trait
├── aisync-core/          # inventory::iter::<AdapterFactory> consumed here
│   ├── src/sync.rs       # enabled_tools() extended
│   └── src/detection.rs  # scan() extended
├── aisync-types/         # No changes needed
└── aisync/               # Binary crate -- community crates linked here via Cargo.toml
```

### Pattern 1: AdapterFactory Wrapper
**What:** A `Sized` struct that wraps a constructor function, collected by inventory
**When to use:** Required because `inventory::Collect` needs `Sized` but `dyn ToolAdapter` is `!Sized`

```rust
// In aisync-adapter/src/lib.rs

/// Factory for creating adapter instances at runtime.
/// Community crates submit these via `inventory::submit!`.
pub struct AdapterFactory {
    /// Human-readable name for diagnostics/logging.
    pub name: &'static str,
    /// Constructor that produces a boxed adapter.
    pub create: fn() -> Box<dyn ToolAdapter>,
}

inventory::collect!(AdapterFactory);
```

### Pattern 2: Community Crate Registration
**What:** A community adapter crate depends on `aisync-adapter` and registers via `inventory::submit!`
**When to use:** Any Rust adapter that needs logic beyond what TOML can express

```rust
// In a community crate (e.g., aisync-adapter-aider/src/lib.rs)

use aisync_adapter::{AdapterFactory, ToolAdapter, /* ... */};

pub struct AiderAdapter;

impl ToolAdapter for AiderAdapter {
    // ... full implementation
}

inventory::submit! {
    AdapterFactory {
        name: "aider",
        create: || Box::new(AiderAdapter),
    }
}
```

### Pattern 3: Integration in enabled_tools() and scan()
**What:** The binary collects registered adapters alongside builtins and TOML adapters
**When to use:** In `SyncEngine::enabled_tools()` and `DetectionEngine::scan()`

```rust
// In aisync-core -- extend enabled_tools()
for factory in inventory::iter::<aisync_adapter::AdapterFactory> {
    let adapter = (factory.create)();
    let key = adapter.name().as_str().to_string();
    if config.tools.is_enabled(&key) {
        let tool_config = config.tools.get_tool(&key);
        tools.push((
            adapter.name(),
            AnyAdapter::Plugin(Arc::from(adapter)),
            tool_config,
        ));
    }
}
```

### Anti-Patterns to Avoid
- **Collecting `dyn ToolAdapter` directly:** Will not compile -- `Collect` requires `Sized`
- **Putting `inventory::collect!` in aisync-core:** The `collect!` call must be in the same crate that defines the collected type. Since community crates depend on `aisync-adapter` (not `aisync-core`), the factory type and `collect!` must live in `aisync-adapter`
- **Using `Arc<dyn ToolAdapter>` as the collected type:** `Arc` is `Sized` so this technically works, but `fn() -> Box<dyn ToolAdapter>` is more ergonomic and avoids forcing community crates to import `Arc`
- **Forgetting to link the community crate:** `inventory::submit!` only works if the crate is linked into the binary. The community adapter crate must appear in the `aisync` binary's `Cargo.toml` dependencies

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Distributed plugin registration | Custom linker tricks or static mut registries | `inventory` crate | Cross-platform linker section management is subtle; inventory handles Linux, macOS, Windows, WASM |
| Adapter factory pattern | Ad-hoc function pointers with unsafe | `AdapterFactory` struct with `inventory::collect!` | Type-safe, no unsafe needed, compile-time guaranteed |
| Plugin discovery | Runtime scanning of dylibs | `inventory::iter` at startup | Compile-time linking is simpler and sufficient for v1.1 |

**Key insight:** The `inventory` crate eliminates the need for any central registry code. Community crates just `submit!` and the binary's startup logic picks them up automatically via `iter`.

## Common Pitfalls

### Pitfall 1: collect! Placement
**What goes wrong:** Putting `inventory::collect!(AdapterFactory)` in `aisync-core` instead of `aisync-adapter` causes "trait bound not satisfied" errors
**Why it happens:** The `collect!` macro must be in the same crate that defines the type being collected
**How to avoid:** `AdapterFactory` struct definition AND `inventory::collect!(AdapterFactory)` must both live in `aisync-adapter/src/lib.rs`
**Warning signs:** Compile error mentioning `Collect` trait not implemented

### Pitfall 2: Community Crate Not Linked
**What goes wrong:** `inventory::submit!` in a community crate has no effect -- adapter not discovered
**Why it happens:** The community crate must be a dependency of the final binary crate (`aisync`). If it is only a transitive dependency of a library crate, the linker may strip it
**How to avoid:** Document that community adapter crates must be added to `aisync/Cargo.toml` (or the user's binary). Consider an `extern crate` in `main.rs` to force linking
**Warning signs:** Adapter silently missing at runtime despite `submit!` call

### Pitfall 3: Name Collisions with Builtins or TOML Adapters
**What goes wrong:** A community Rust adapter registers with a name that matches a builtin or a TOML adapter, causing duplicate detection results or sync conflicts
**Why it happens:** No deduplication between registration sources
**How to avoid:** Reuse the existing `BUILTIN_NAMES` collision guard from `declarative.rs`. Add deduplication in `enabled_tools()` that prefers builtin > TOML > inventory order
**Warning signs:** Duplicate tool entries in status/sync output

### Pitfall 4: Box\<dyn ToolAdapter\> to Arc Conversion
**What goes wrong:** `AnyAdapter::Plugin` takes `Arc<dyn ToolAdapter>` but factory returns `Box<dyn ToolAdapter>`
**Why it happens:** Different smart pointer types
**How to avoid:** Use `Arc::from(box_value)` which converts `Box<dyn T>` to `Arc<dyn T>` without extra allocation (stabilized in Rust 1.21)
**Warning signs:** Compile error on type mismatch

### Pitfall 5: inventory and Rust 2024 Edition
**What goes wrong:** Concern flagged in STATE.md about `inventory` 0.3 compatibility with `edition = "2024"`
**Why it happens:** `inventory` itself uses `edition = "2021"` but this is fine -- Rust editions are per-crate. A crate using `edition = "2024"` can depend on a crate using `edition = "2021"` with no issues
**How to avoid:** No action needed. Verified: inventory 0.3.22 works with rustc 1.92.0 (the version installed). No open issues about edition 2024 compatibility
**Warning signs:** None expected

## Code Examples

### AdapterFactory Definition (in aisync-adapter)
```rust
// Source: inventory 0.3.22 README + project architecture

/// A registration entry for compile-time adapter discovery.
///
/// Community adapter crates submit instances via `inventory::submit!`.
/// The aisync binary iterates all submissions via `inventory::iter::<AdapterFactory>`.
pub struct AdapterFactory {
    /// Identifier for deduplication and logging.
    pub name: &'static str,
    /// Constructor function. Called once during adapter collection.
    pub create: fn() -> Box<dyn ToolAdapter>,
}

// This macro MUST be in the same crate that defines AdapterFactory.
inventory::collect!(AdapterFactory);
```

### Community Adapter Crate (example: aisync-adapter-aider)
```rust
// Cargo.toml:
// [dependencies]
// aisync-adapter = "0.1"
// inventory = "0.3"

use aisync_adapter::{AdapterFactory, ToolAdapter, AdapterError, DetectionResult};
use aisync_adapter::aisync_types::{ToolKind, Confidence, SyncStrategy};
use std::path::Path;

pub struct AiderAdapter;

impl ToolAdapter for AiderAdapter {
    fn name(&self) -> ToolKind { ToolKind::Custom("aider".into()) }
    fn display_name(&self) -> &str { "Aider" }
    fn native_instruction_path(&self) -> &str { ".aider.conf.yml" }
    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AdapterError> {
        let detected = project_root.join(".aider").is_dir();
        Ok(DetectionResult {
            tool: self.name(),
            detected,
            confidence: Confidence::High,
            markers_found: if detected { vec![project_root.join(".aider")] } else { vec![] },
            version_hint: None,
        })
    }
}

inventory::submit! {
    AdapterFactory {
        name: "aider",
        create: || Box::new(AiderAdapter),
    }
}
```

### Consuming Registered Adapters (in aisync-core)
```rust
// In SyncEngine::enabled_tools() -- after builtins and TOML adapters:

for factory in inventory::iter::<aisync_adapter::AdapterFactory> {
    let adapter = (factory.create)();
    let name_str = adapter.name().as_str().to_string();

    // Skip if name collides with builtin or already-seen adapter
    if seen_names.contains(&name_str) {
        eprintln!("Warning: inventory adapter '{}' skipped (name collision)", name_str);
        continue;
    }
    seen_names.insert(name_str.clone());

    if config.tools.is_enabled(&name_str) {
        let tool_config = config.tools.get_tool(&name_str);
        tools.push((
            adapter.name(),
            AnyAdapter::Plugin(Arc::from(adapter)),
            tool_config,
        ));
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Central enum (AnyAdapter variants) | Plugin(Arc\<dyn ToolAdapter\>) variant | Phase 6 (REFAC-03) | Enabled dynamic dispatch for non-builtin adapters |
| No external adapters | TOML declarative adapters (.ai/adapters/*.toml) | Phase 10 (SDK-03/04/05) | Simple adapters without Rust code |
| No compile-time registration | inventory::submit! (this phase) | Phase 11 (SDK-06) | Community Rust adapters without modifying main repo |

**Deprecated/outdated:**
- Manual `AnyAdapter` enum modification for new adapters -- replaced by Plugin variant + inventory

## Open Questions

1. **Should `aisync-adapter` re-export `inventory::submit!`?**
   - What we know: Community crates need both `aisync-adapter` and `inventory` as dependencies
   - What's unclear: Whether re-exporting the macro reduces friction enough to justify the coupling
   - Recommendation: Re-export `inventory::submit!` as `aisync_adapter::submit!` for ergonomics. Community crates then only need `aisync-adapter` as a dependency (inventory becomes transitive). If this creates macro re-export issues, fall back to requiring explicit `inventory` dependency.

2. **Documentation format: in-repo docs vs published book?**
   - What we know: SDK-07 requires documentation for both TOML and Rust adapter authoring
   - What's unclear: Whether a `docs/` directory guide or `README.md` sections are sufficient
   - Recommendation: Add `docs/ADAPTER-AUTHORING.md` with two sections (TOML path, Rust path) plus a working example crate. This is sufficient for v1.1; a mdbook can come later.

3. **Example adapter crate: in-repo or separate?**
   - What we know: Success criteria requires "working examples"
   - What's unclear: Whether to create a full example crate in the workspace
   - Recommendation: Create `examples/adapter-example/` as a standalone Cargo project (NOT a workspace member) that demonstrates the pattern. This avoids workspace coupling while providing a copy-pasteable template.

## Sources

### Primary (HIGH confidence)
- inventory crate README (github.com/dtolnay/inventory) -- API, usage patterns, platform support
- inventory crate source (lib.rs) -- `Collect` trait constraints: `Sync + Sized + 'static`
- inventory Cargo.toml -- version 0.3.22, edition 2021, MSRV 1.68
- Codebase: `crates/aisync-adapter/src/lib.rs` -- ToolAdapter trait definition, Send + Sync bounds
- Codebase: `crates/aisync-core/src/adapter.rs` -- AnyAdapter enum, Plugin(Arc<dyn ToolAdapter>) variant
- Codebase: `crates/aisync-core/src/sync.rs` -- enabled_tools() integration point
- Codebase: `crates/aisync-core/src/detection.rs` -- scan() integration point
- Codebase: `crates/aisync-core/src/declarative.rs` -- TOML adapter pattern, BUILTIN_NAMES guard

### Secondary (MEDIUM confidence)
- crates.io API -- inventory 0.3.22 latest release date (2026-02-19)
- GitHub issues -- no open issues about Rust 2024 edition compatibility

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- inventory is the canonical solution for this pattern in Rust, maintained by dtolnay
- Architecture: HIGH -- integration points are clear from codebase analysis; Plugin variant and enabled_tools() already exist
- Pitfalls: HIGH -- constraints verified in source code (Sized requirement, collect! placement rule)

**Research date:** 2026-03-08
**Valid until:** 2026-04-08 (stable domain, inventory API unlikely to change)
