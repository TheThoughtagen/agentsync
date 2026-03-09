# Phase 9: Plugin SDK Crate Extraction - Research

**Researched:** 2026-03-08
**Domain:** Rust workspace crate extraction and public API design
**Confidence:** HIGH

## Summary

Phase 9 extracts shared types and the `ToolAdapter` trait from `aisync-core` into two new independently-publishable crates: `aisync-types` and `aisync-adapter`. This is a pure refactoring phase -- no new features, no behavior changes. The goal is to invert the dependency so `aisync-core` depends on the SDK crates, enabling community developers to build custom adapters by depending only on the lightweight SDK crates without pulling in all of `aisync-core`.

The codebase is well-structured for this extraction. Types live cleanly in `types.rs`, the trait lives in `adapter.rs`, and the dependency graph between modules is already relatively clean. The main complexity is identifying which types belong in `aisync-types` (pure data) vs `aisync-adapter` (trait + trait-adjacent types), and updating all import paths across the workspace without breaking anything.

**Primary recommendation:** Extract types first (`aisync-types`), then extract the trait (`aisync-adapter` depending on `aisync-types`), then rewire `aisync-core` to depend on both. Re-export all public types from `aisync-core` to avoid breaking the CLI crate.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SDK-01 | `aisync-types` crate extracted with shared types (ToolKind, SyncStrategy, etc.) | Types are cleanly isolated in `types.rs` and `config.rs` (SyncStrategy). Dependencies are minimal: serde, thiserror, sha2, hex, serde_json. Can be reduced to just serde + thiserror for the types crate. |
| SDK-02 | `aisync-adapter` crate published with ToolAdapter trait and supporting types | Trait is cleanly defined in `adapter.rs` with well-defined type dependencies. Needs `aisync-types` + a subset of error types. The `AnyAdapter` enum and built-in adapter structs should stay in `aisync-core` (they are implementation, not SDK). |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1.0 (workspace) | Serialization for types | Already used, required for ToolKind/SyncStrategy serde derives |
| thiserror | 2.0 (workspace) | Error derive macros | Already used, needed for AisyncError/AdapterError in SDK |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde_json | 1.0 (workspace) | JSON serialization | Only if aisync-types needs json support (currently used in tests) |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Re-exporting from aisync-core | Breaking the public API | Re-exports preserve backward compatibility -- strongly preferred |
| Moving AnyAdapter to aisync-adapter | Keeping it in aisync-core | AnyAdapter is implementation dispatch, not public SDK surface. Keep in core. |

## Architecture Patterns

### Recommended Crate Structure
```
crates/
  aisync-types/        # Pure data types, no behavior beyond serde
    src/lib.rs         # Re-exports all public types
  aisync-adapter/      # ToolAdapter trait + supporting types
    src/lib.rs         # Trait definition, DetectionResult, re-exports aisync-types
  aisync-core/         # Depends on aisync-types + aisync-adapter
    src/lib.rs         # Re-exports SDK types for backward compat
  aisync/              # CLI binary, depends on aisync-core (unchanged)
```

### Pattern 1: Type Ownership Boundaries

**What:** Clear rules for which types belong where.

**aisync-types owns (pure data, serde-derivable):**
- `ToolKind` (enum with Custom variant)
- `SyncStrategy` (enum: Symlink, Copy, Generate)
- `Confidence` (enum: High, Medium)
- `SyncAction` (enum with all variants)
- `ToolSyncResult`, `SyncReport`
- `DriftState`, `ToolSyncStatus`, `StatusReport`
- `HooksConfig`, `HookGroup`, `HookHandler`, `HookTranslation`
- `ToolDiff`
- `WatchEvent`
- `MemoryStatusReport`, `ToolMemoryStatus`
- `HookStatusReport`, `ToolHookStatus`
- `content_hash()` function (uses sha2+hex -- consider whether to include or leave in core)

**aisync-adapter owns (trait + closely coupled types):**
- `ToolAdapter` trait
- `DetectionResult` struct (returned by `ToolAdapter::detect`)
- Re-exports `aisync-types` for convenience

**aisync-core keeps (implementation):**
- `AnyAdapter` enum and dispatch macro
- All concrete adapter structs (`ClaudeCodeAdapter`, `CursorAdapter`, etc.)
- All adapter implementations (`adapters/*.rs`)
- `AisyncConfig`, `ToolConfig`, `ToolsConfig`, `DefaultsConfig` (config parsing)
- All engines (`SyncEngine`, `DetectionEngine`, `WatchEngine`, etc.)
- All error types (most are core-internal)

**When to use:** This is the primary decision framework for the entire phase.

### Pattern 2: Re-export for Backward Compatibility

**What:** `aisync-core/src/lib.rs` re-exports everything from `aisync-types` and `aisync-adapter` so existing `use aisync_core::ToolKind` paths continue to work.

**Example:**
```rust
// aisync-core/src/lib.rs
pub use aisync_types::*;
pub use aisync_adapter::{ToolAdapter, DetectionResult};
```

**When to use:** Always. The CLI crate (`aisync`) and all internal code should not need import path changes.

### Pattern 3: Workspace Dependencies for Version Consistency

**What:** Declare new crates as workspace dependencies with path references.

**Example in workspace Cargo.toml:**
```toml
[workspace.dependencies]
aisync-types = { path = "crates/aisync-types", version = "0.1.0" }
aisync-adapter = { path = "crates/aisync-adapter", version = "0.1.0" }
```

### Anti-Patterns to Avoid
- **Circular dependencies:** aisync-types MUST NOT depend on aisync-adapter or aisync-core. aisync-adapter MUST NOT depend on aisync-core.
- **Leaking internal types into SDK:** Config types (`AisyncConfig`, `ToolsConfig`) are internal to core, not SDK surface.
- **Moving too much into SDK:** Only types that a community adapter author actually needs belong in the SDK crates. Engine types, config parsing, etc. stay in core.
- **Breaking re-exports:** If `aisync-core` stops re-exporting a moved type, the CLI crate and any downstream code breaks.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Serde compatibility across crate boundaries | Custom serialization wrappers | Standard serde derives with `#[serde(rename_all)]` | serde works transparently across crate boundaries |
| Trait object compatibility | Custom vtable tricks | `Arc<dyn ToolAdapter>` (already in place) | The Plugin variant already uses Arc for dyn dispatch |

**Key insight:** This is a mechanical refactoring. The types and trait already exist and work. The challenge is purely about moving code between crates without breaking anything.

## Common Pitfalls

### Pitfall 1: ToolSyncStatus References SyncStrategy from config.rs
**What goes wrong:** `ToolSyncStatus` in `types.rs` has a `strategy: crate::config::SyncStrategy` field. If `ToolSyncStatus` moves to `aisync-types` but `SyncStrategy` stays in `aisync-core::config`, you get a circular dependency.
**Why it happens:** `SyncStrategy` is defined in `config.rs`, not `types.rs`, but it is used by types in `types.rs`.
**How to avoid:** `SyncStrategy` MUST move to `aisync-types` alongside `ToolSyncStatus`. It is a pure data enum with serde derives -- it belongs in types.
**Warning signs:** Compilation error about `crate::config::SyncStrategy` not found.

### Pitfall 2: ToolAdapter Methods Reference SyncStrategy and AisyncError
**What goes wrong:** The `ToolAdapter` trait methods take `SyncStrategy` parameters and return `Result<_, AisyncError>`. If these types are in different crates, the trait can't be defined cleanly.
**Why it happens:** The trait bridges types (from aisync-types) with error handling (from aisync-core).
**How to avoid:** Create a minimal `AdapterError` or re-use a subset of error types in `aisync-adapter`. The full `AisyncError` hierarchy should stay in core, but the trait needs at least a `Box<dyn Error>` or a dedicated adapter-level error type. Alternatively, `aisync-adapter` can define its own error type that `AisyncError` wraps via `From`.
**Warning signs:** The trait returning `AisyncError` forces `aisync-adapter` to depend on the full error module.

### Pitfall 3: content_hash Pulls in sha2+hex Dependencies
**What goes wrong:** `content_hash()` is in `types.rs` but uses `sha2` and `hex` crates. Moving all of `types.rs` to `aisync-types` would add these as dependencies of the types crate.
**Why it happens:** `content_hash()` was placed in types.rs for convenience but is really a utility function.
**How to avoid:** Leave `content_hash()` in `aisync-core` (it is a utility, not a type). Or move it but accept the deps. The success criteria says "only serde/thiserror dependencies" -- so leave it in core.
**Warning signs:** `aisync-types` Cargo.toml listing sha2 and hex.

### Pitfall 4: Test-Only Dependencies Inflating SDK Crate
**What goes wrong:** Types tests use `serde_json` for roundtrip testing. This could become a runtime dependency of `aisync-types`.
**Why it happens:** Tests in `types.rs` call `serde_json::to_string` and `serde_json::from_str`.
**How to avoid:** Add `serde_json` as a dev-dependency only in `aisync-types`. Move or duplicate the serde roundtrip tests.
**Warning signs:** `serde_json` appearing in `[dependencies]` instead of `[dev-dependencies]`.

### Pitfall 5: Forgetting publish = false While Iterating
**What goes wrong:** Accidentally publishing incomplete SDK crates to crates.io.
**Why it happens:** New crates default to publishable.
**How to avoid:** Set `publish = false` initially (matching existing crates). Remove when ready to publish.
**Warning signs:** `cargo publish` succeeding unexpectedly.

## Code Examples

### aisync-types Cargo.toml
```toml
[package]
name = "aisync-types"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
description = "Shared types for the aisync ecosystem"
license = "MIT"
repository = "https://github.com/pmannion/agentsync"
publish = false

[dependencies]
serde = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
serde_json = { workspace = true }
```

### aisync-adapter Cargo.toml
```toml
[package]
name = "aisync-adapter"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
description = "ToolAdapter trait for building aisync adapters"
license = "MIT"
repository = "https://github.com/pmannion/agentsync"
publish = false

[dependencies]
aisync-types = { workspace = true }
thiserror = { workspace = true }
```

### aisync-types lib.rs (sketch)
```rust
// Re-export everything for convenience
mod types;
mod sync_strategy;

pub use types::*;
pub use sync_strategy::SyncStrategy;
```

### aisync-adapter lib.rs (sketch)
```rust
pub use aisync_types;  // Re-export types crate

use std::path::{Path, PathBuf};
use aisync_types::{Confidence, DriftState, HooksConfig, HookTranslation, SyncAction, SyncStrategy, ToolKind, ToolSyncStatus};

/// Error type for adapter operations.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("detection failed: {0}")]
    DetectionFailed(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

/// Result of detecting a specific AI tool in a project directory.
#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub tool: ToolKind,
    pub detected: bool,
    pub confidence: Confidence,
    pub markers_found: Vec<PathBuf>,
    pub version_hint: Option<String>,
}

pub trait ToolAdapter: Send + Sync {
    fn name(&self) -> ToolKind;
    fn display_name(&self) -> &str;
    fn native_instruction_path(&self) -> &str;
    // ... rest of trait methods using AdapterError instead of AisyncError
}
```

### Re-export Pattern in aisync-core lib.rs
```rust
// Backward-compatible re-exports
pub use aisync_types::{
    Confidence, DriftState, HookGroup, HookHandler, HookStatusReport,
    HookTranslation, HooksConfig, MemoryStatusReport, StatusReport,
    SyncAction, SyncReport, ToolDiff, ToolHookStatus, ToolKind,
    ToolMemoryStatus, ToolSyncResult, ToolSyncStatus, WatchEvent,
    SyncStrategy,
};
pub use aisync_adapter::{ToolAdapter, DetectionResult, AdapterError};
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| All types in aisync-core | Types in aisync-core (Phase 9 changes this) | Phase 9 | Community can depend on SDK without pulling core |
| AisyncError as trait return type | AdapterError (dedicated SDK error) | Phase 9 | Decouples adapter from core error hierarchy |

## Open Questions

1. **Error Type Strategy for ToolAdapter**
   - What we know: The trait currently returns `Result<_, AisyncError>`. `AisyncError` is a large enum wrapping config, detection, sync, init, memory, hook, and watch errors. Community adapters should not need to construct most of these.
   - What's unclear: Should `aisync-adapter` define its own `AdapterError` type, or use `Box<dyn std::error::Error + Send + Sync>`?
   - Recommendation: Define a dedicated `AdapterError` in `aisync-adapter` with variants for IO, detection, and generic errors. Have `AisyncError` in core wrap `AdapterError` via `From`. This keeps the SDK error surface small and focused.

2. **Whether content_hash Belongs in aisync-types**
   - What we know: Success criteria says "only serde/thiserror dependencies." `content_hash` uses sha2+hex.
   - What's unclear: Is content_hash used by adapter implementations? (Yes -- `claude_code.rs` and others use it.)
   - Recommendation: Leave `content_hash` in `aisync-core`. Adapter implementations that need hashing can depend on aisync-core or compute hashes themselves. The function is a utility, not a type.

3. **SyncAction Enum Size in aisync-types**
   - What we know: SyncAction has 15 variants, many referencing PathBuf. It is used in ToolAdapter::plan_sync return type.
   - What's unclear: Should all SyncAction variants be in the types crate, or should some stay in core?
   - Recommendation: Move the entire SyncAction enum to aisync-types. All variants are pure data (PathBuf + String). Adapter authors need to construct these variants.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml per crate |
| Quick run command | `cargo test -p aisync-types && cargo test -p aisync-adapter` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SDK-01 | aisync-types compiles independently with only serde/thiserror deps | unit | `cargo test -p aisync-types` | Wave 0 |
| SDK-01 | ToolKind, SyncStrategy, related types exported | unit | `cargo test -p aisync-types` | Wave 0 |
| SDK-02 | aisync-adapter exports ToolAdapter trait | unit | `cargo test -p aisync-adapter` | Wave 0 |
| SDK-02 | External crate can depend on aisync-adapter | integration | `cargo test -p aisync-core` (re-exports work) | Wave 0 |
| SDK-01+02 | aisync-core depends on SDK crates (inverted dependency) | build | `cargo check -p aisync-core` | Existing |
| SDK-01+02 | Full workspace still compiles and passes all tests | integration | `cargo test --workspace` | Existing (188+ tests) |

### Sampling Rate
- **Per task commit:** `cargo test -p aisync-types -p aisync-adapter -p aisync-core`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before verify

### Wave 0 Gaps
- [ ] `crates/aisync-types/` -- new crate directory, Cargo.toml, src/lib.rs
- [ ] `crates/aisync-adapter/` -- new crate directory, Cargo.toml, src/lib.rs
- [ ] Workspace Cargo.toml must include new crate members

## Sources

### Primary (HIGH confidence)
- Direct codebase analysis of `crates/aisync-core/src/types.rs` (406 lines, 14 public types)
- Direct codebase analysis of `crates/aisync-core/src/adapter.rs` (441 lines, trait + AnyAdapter)
- Direct codebase analysis of `crates/aisync-core/src/config.rs` (SyncStrategy enum)
- Direct codebase analysis of `crates/aisync-core/src/error.rs` (error hierarchy)
- Direct codebase analysis of `crates/aisync-core/src/lib.rs` (public re-exports)
- Cargo workspace configuration analysis

### Secondary (MEDIUM confidence)
- Rust crate extraction patterns are well-established in the ecosystem (e.g., tokio/tokio-util, axum/axum-core)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Only uses existing workspace dependencies (serde, thiserror)
- Architecture: HIGH - Direct analysis of all source files, clear type boundaries identified
- Pitfalls: HIGH - All pitfalls identified from concrete code analysis (SyncStrategy location, content_hash deps, error type coupling)

**Research date:** 2026-03-08
**Valid until:** 2026-04-07 (stable domain, no external dependency concerns)
