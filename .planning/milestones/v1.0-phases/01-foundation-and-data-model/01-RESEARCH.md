# Phase 1: Foundation and Data Model - Research

**Researched:** 2026-03-05
**Domain:** Rust workspace setup, config parsing, trait design, filesystem detection
**Confidence:** HIGH

## Summary

Phase 1 establishes the foundational Rust library for aisync: a Cargo workspace with two crates, a TOML config schema with serde-based parsing, an adapter trait for tool detection, and a detection engine that scans for AI tool markers. This is a greenfield Rust project using the 2024 edition (requires Rust 1.85+; host has 1.92).

The Rust ecosystem for this phase is mature and well-documented. Config parsing via `serde` + `toml`, error handling via `thiserror`, and trait-based adapter patterns are all standard, stable approaches. The main design decision in Claude's discretion -- enum dispatch vs dyn trait objects -- should use compile-time enum dispatch since we have a closed, known set of tools (Claude Code, Cursor, OpenCode).

**Primary recommendation:** Use `serde` + `toml` for config, `thiserror` for errors, compile-time enum dispatch for adapters, and `std::fs`/`std::path` for tool detection. No async needed in Phase 1.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Tool Detection Markers:
  - Claude Code: detect via both CLAUDE.md and .claude/ directory -- either triggers detection
  - Cursor: detect via both .cursor/rules/ (current) and .cursorrules (legacy). Flag legacy format in status output
  - OpenCode: detect via both AGENTS.md and opencode.json
  - Ambiguous markers (e.g., AGENTS.md could be OpenCode or Copilot): report with confidence level (High/Medium), let user confirm during init
- Config Schema (aisync.toml):
  - schema_version as top-level integer (schema_version = 1)
  - Per-tool config via nested TOML tables: [tools.claude-code], [tools.cursor], etc.
  - Each tool section has enabled, sync_strategy, and tool-specific fields
  - Three sync strategies: symlink (default for macOS/Linux), copy (Windows fallback), generate (for tools needing transformation like Cursor .mdc)
  - Global [defaults] section that tools inherit from -- tools override only when they differ
- Adapter Trait Contract:
  - detect() returns structured DetectionResult { detected, confidence: High/Medium, markers_found: Vec<PathBuf>, version_hint: Option<String> }
  - Error modeling: per-adapter error enum + thiserror derives, converting into top-level AisyncError
  - Lean trait in Phase 1: detect() and name() only. read/write added in Phase 2, sync_memory/translate_hook in Phase 3, watch_paths in Phase 4
- Workspace Organization:
  - Cargo workspace with two crates: aisync-core (library) and aisync (binary)
  - crates/ directory: crates/aisync-core/ and crates/aisync/
  - Test fixtures at workspace root: fixtures/ with subdirectories simulating tool setups (claude-only/, multi-tool/, no-tools/)
  - Binary name: aisync (matches CLI command)
  - Rust 2024 edition (requires 1.85+)

### Claude's Discretion
- Adapter dispatch model (compile-time enum vs dyn trait objects)
- Internal module organization within aisync-core
- Specific thiserror message wording
- Test organization within crates

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CLI-08 | `aisync.toml` config file with `schema_version`, per-tool settings, sync strategy | serde + toml crate for parsing/serialization; struct design with Default/Serialize/Deserialize derives; nested TOML tables map to nested Rust structs |
| ADPT-04 | Tool detection engine scans project root for AI tool config markers | std::fs + std::path for marker scanning; detection markers documented per tool; confidence levels modeled as enum |
| ADPT-05 | Adapter trait with detect, read, write, sync_memory, translate_hook, watch_paths | Phase 1 scope is lean: detect() + name() only; trait defined with full signature but remaining methods deferred; thiserror for error types |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1.0.228 | Serialization/deserialization framework | De facto standard for all Rust data serialization |
| toml | 1.0.4 | TOML parsing and serialization | Native Rust TOML encoder/decoder, serde-compatible |
| thiserror | 2.0.18 | Derive macros for Error trait | Standard for library error types; does not leak into public API |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde (derive feature) | 1.0.228 | #[derive(Serialize, Deserialize)] | Every struct that touches config |

### Not Needed in Phase 1
| Library | Why Not Yet |
|---------|-------------|
| clap | No CLI commands until Phase 2 |
| tokio/async-std | No async operations; filesystem scanning is synchronous |
| anyhow | Library crate uses thiserror; anyhow for binary crate in Phase 2 |
| notify | File watching is Phase 4 |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| toml | toml_edit | toml_edit preserves formatting/comments but adds complexity; not needed for Phase 1 structured parsing |
| thiserror | manual Error impl | thiserror generates identical code but eliminates boilerplate |

**Installation (workspace Cargo.toml):**
```toml
[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
toml = "1.0"
thiserror = "2.0"
```

## Architecture Patterns

### Recommended Project Structure
```
agentsync/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── aisync-core/            # Library crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs          # Public API re-exports
│   │       ├── config.rs       # Config types + parsing
│   │       ├── adapter.rs      # ToolAdapter trait + DetectionResult
│   │       ├── detection.rs    # Detection engine (scans filesystem)
│   │       ├── error.rs        # AisyncError + per-module errors
│   │       └── types.rs        # Shared types (SyncStrategy, Confidence, ToolKind)
│   └── aisync/                 # Binary crate (placeholder in Phase 1)
│       ├── Cargo.toml
│       └── src/
│           └── main.rs         # Minimal main() that compiles
├── fixtures/                   # Test fixtures
│   ├── claude-only/            # CLAUDE.md + .claude/
│   ├── cursor-only/            # .cursor/rules/
│   ├── cursor-legacy/          # .cursorrules (legacy)
│   ├── opencode-only/          # AGENTS.md + opencode.json
│   ├── multi-tool/             # Multiple tools present
│   ├── ambiguous/              # AGENTS.md only (could be OpenCode or other)
│   └── no-tools/               # Empty project, no markers
└── PRD.md
```

### Pattern 1: Enum Dispatch for Adapters (Recommended)

**What:** Use a compile-time enum wrapping all adapter implementations instead of `dyn ToolAdapter`.

**When to use:** When the set of types is closed and known at compile time (we have exactly 3 tools in v1).

**Why recommended:** The tool set is closed (Claude Code, Cursor, OpenCode). Enum dispatch gives:
- No vtable overhead
- Compiler can inline match arms
- Exhaustive matching ensures all tools are handled
- No object-safety constraints on the trait
- Easy to enumerate all adapters for detection scanning

**Example:**
```rust
pub enum ToolKind {
    ClaudeCode,
    Cursor,
    OpenCode,
}

// Each adapter is a zero-sized struct
pub struct ClaudeCodeAdapter;
pub struct CursorAdapter;
pub struct OpenCodeAdapter;

pub enum AnyAdapter {
    ClaudeCode(ClaudeCodeAdapter),
    Cursor(CursorAdapter),
    OpenCode(OpenCodeAdapter),
}

impl AnyAdapter {
    pub fn all() -> Vec<AnyAdapter> {
        vec![
            AnyAdapter::ClaudeCode(ClaudeCodeAdapter),
            AnyAdapter::Cursor(CursorAdapter),
            AnyAdapter::OpenCode(OpenCodeAdapter),
        ]
    }

    pub fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError> {
        match self {
            AnyAdapter::ClaudeCode(a) => a.detect(project_root),
            AnyAdapter::Cursor(a) => a.detect(project_root),
            AnyAdapter::OpenCode(a) => a.detect(project_root),
        }
    }
}
```

### Pattern 2: Config Schema with Defaults Inheritance

**What:** Global defaults section with per-tool overrides using Option fields.

**Example:**
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AisyncConfig {
    pub schema_version: u32,
    #[serde(default)]
    pub defaults: DefaultsConfig,
    #[serde(default)]
    pub tools: ToolsConfig,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DefaultsConfig {
    #[serde(default = "default_sync_strategy")]
    pub sync_strategy: SyncStrategy,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ToolsConfig {
    #[serde(rename = "claude-code")]
    pub claude_code: Option<ToolConfig>,
    pub cursor: Option<ToolConfig>,
    pub opencode: Option<ToolConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub sync_strategy: Option<SyncStrategy>,
    // Tool-specific fields via flatten or additional struct
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SyncStrategy {
    #[default]
    Symlink,
    Copy,
    Generate,
}
```

### Pattern 3: Structured Error Hierarchy

**What:** Per-module error enums that convert into a top-level AisyncError.

**Example:**
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AisyncError {
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
    #[error("detection error: {0}")]
    Detection(#[from] DetectionError),
    #[error("adapter error for {tool}: {source}")]
    Adapter { tool: String, source: AdapterError },
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    ReadFile(#[from] std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("unsupported schema version {version}, expected {expected}")]
    UnsupportedVersion { version: u32, expected: u32 },
}

#[derive(Debug, Error)]
pub enum DetectionError {
    #[error("failed to scan directory {path}: {source}")]
    ScanFailed { path: String, source: std::io::Error },
}

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("detection failed: {0}")]
    DetectionFailed(String),
}
```

### Anti-Patterns to Avoid
- **Premature async:** Phase 1 is pure filesystem scanning -- do not pull in tokio/async. Keep everything synchronous.
- **Stringly-typed tool names:** Use `ToolKind` enum, not `String`, for tool identification.
- **Monolithic error enum:** Don't put all error variants in one flat enum. Use nested per-module errors with `#[from]` conversion.
- **Exposing serde in public API:** Config structs can derive Serialize/Deserialize, but don't require callers to depend on serde directly.
- **Over-engineering the trait:** Phase 1 trait has detect() + name() only. Don't add unimplemented methods that return `todo!()`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TOML parsing | Custom parser | `toml` crate with serde derives | TOML spec is complex; crate handles all edge cases |
| Error boilerplate | Manual Display/Error impls | `thiserror` derives | Identical output, zero runtime cost, less code |
| Config defaults | Manual default logic | `serde(default)` + Default trait | Handles missing fields, nesting, and inheritance cleanly |
| TOML serialization | Manual string building | `toml::to_string_pretty()` | Handles escaping, nested tables, array formatting |

**Key insight:** The core logic in Phase 1 is data modeling and filesystem scanning -- both are well-served by standard library patterns. Custom code should focus on the detection heuristics and confidence scoring, not infrastructure.

## Common Pitfalls

### Pitfall 1: AGENTS.md Ambiguity
**What goes wrong:** AGENTS.md is used by both OpenCode and potentially other tools (Copilot). Treating any AGENTS.md as definitively OpenCode leads to false positives.
**Why it happens:** Multiple tools adopted the same filename convention.
**How to avoid:** When AGENTS.md is found without opencode.json, report confidence as Medium. When both AGENTS.md and opencode.json exist, report High.
**Warning signs:** Tests that assume AGENTS.md = OpenCode without checking for corroborating markers.

### Pitfall 2: Cursor Legacy Format Detection
**What goes wrong:** Only checking for `.cursor/rules/` and missing projects using the legacy `.cursorrules` file.
**Why it happens:** Cursor migrated from .cursorrules to .cursor/rules/ directory with .mdc files, but both formats remain in use.
**How to avoid:** Check both locations. When `.cursorrules` is found, set a flag (`legacy_format: true`) so later phases can warn the user.
**Warning signs:** Detection misses in projects that have not migrated to the new Cursor format.

### Pitfall 3: serde rename for TOML Keys
**What goes wrong:** Rust struct field `claude_code` doesn't match TOML key `claude-code` (kebab-case).
**Why it happens:** Rust identifiers can't contain hyphens; TOML conventionally uses kebab-case.
**How to avoid:** Use `#[serde(rename = "claude-code")]` on the field, or use `#[serde(rename_all = "kebab-case")]` on the struct.
**Warning signs:** Deserialization silently produces None for fields that should have values.

### Pitfall 4: Edition 2024 Resolver Change
**What goes wrong:** Not specifying the resolver in workspace Cargo.toml, getting unexpected dependency resolution.
**Why it happens:** Edition 2024 implies `resolver = "3"` (Rust-version-aware resolver). This is generally fine but changes dependency resolution behavior.
**How to avoid:** Set `edition = "2024"` in workspace members; the workspace root inherits resolver automatically. Be aware that resolver 3 respects `rust-version` fields in dependencies.
**Warning signs:** Dependency conflicts or unexpected version selections.

### Pitfall 5: Path Handling Cross-Platform
**What goes wrong:** Using string-based path comparisons or hardcoded `/` separators.
**Why it happens:** macOS and Linux use `/`, Windows uses `\`.
**How to avoid:** Always use `std::path::Path` and `std::path::PathBuf`. Use `path.join()` instead of format strings. Use `path.exists()` and `path.is_dir()` for detection.
**Warning signs:** Tests pass on macOS but fail on Windows CI.

## Code Examples

### Workspace Root Cargo.toml
```toml
[workspace]
members = ["crates/*"]

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
toml = "1.0"
thiserror = "2.0"

[workspace.package]
edition = "2024"
rust-version = "1.85"
```

### aisync-core Cargo.toml
```toml
[package]
name = "aisync-core"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true

[dependencies]
serde = { workspace = true }
toml = { workspace = true }
thiserror = { workspace = true }
```

### aisync Binary Cargo.toml
```toml
[package]
name = "aisync"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true

[[bin]]
name = "aisync"
path = "src/main.rs"

[dependencies]
aisync-core = { path = "../aisync-core" }
```

### Config Parsing
```rust
use std::path::Path;

impl AisyncConfig {
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: AisyncConfig = toml::from_str(&content)?;
        if config.schema_version != 1 {
            return Err(ConfigError::UnsupportedVersion {
                version: config.schema_version,
                expected: 1,
            });
        }
        Ok(config)
    }

    pub fn to_string_pretty(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }
}
```

### Tool Detection Engine
```rust
use std::path::Path;

pub struct DetectionEngine;

#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub tool: ToolKind,
    pub detected: bool,
    pub confidence: Confidence,
    pub markers_found: Vec<std::path::PathBuf>,
    pub version_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Confidence {
    High,
    Medium,
}

impl DetectionEngine {
    pub fn scan(project_root: &Path) -> Result<Vec<DetectionResult>, DetectionError> {
        let adapters = AnyAdapter::all();
        let mut results = Vec::new();
        for adapter in &adapters {
            let result = adapter.detect(project_root)
                .map_err(|e| DetectionError::ScanFailed {
                    path: project_root.display().to_string(),
                    source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
                })?;
            if result.detected {
                results.push(result);
            }
        }
        Ok(results)
    }
}
```

### Claude Code Detection Logic
```rust
impl ClaudeCodeAdapter {
    pub fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError> {
        let claude_md = project_root.join("CLAUDE.md");
        let claude_dir = project_root.join(".claude");

        let mut markers = Vec::new();
        if claude_md.exists() {
            markers.push(claude_md);
        }
        if claude_dir.is_dir() {
            markers.push(claude_dir);
        }

        Ok(DetectionResult {
            tool: ToolKind::ClaudeCode,
            detected: !markers.is_empty(),
            confidence: Confidence::High, // Both markers are unambiguous
            markers_found: markers,
            version_hint: None,
        })
    }
}
```

### Test Fixture Pattern
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir; // Or use fixtures/ directory

    #[test]
    fn detects_claude_code_from_claude_md() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Project").unwrap();

        let result = ClaudeCodeAdapter.detect(dir.path()).unwrap();
        assert!(result.detected);
        assert_eq!(result.confidence, Confidence::High);
        assert_eq!(result.markers_found.len(), 1);
    }

    #[test]
    fn no_detection_in_empty_directory() {
        let dir = TempDir::new().unwrap();
        let result = ClaudeCodeAdapter.detect(dir.path()).unwrap();
        assert!(!result.detected);
        assert!(result.markers_found.is_empty());
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Rust 2021 edition | Rust 2024 edition | Rust 1.85 (Feb 2025) | Resolver v3, minor syntax changes |
| thiserror 1.x | thiserror 2.x | Late 2024 | MSRV bump, derive syntax unchanged |
| .cursorrules (single file) | .cursor/rules/*.mdc (directory) | 2024 | Both still work; legacy should be flagged |
| toml 0.5.x | toml 1.0.x (TOML spec 1.1.0) | 2024 | Full TOML 1.0 spec support |

**Deprecated/outdated:**
- `.cursorrules` single file: Deprecated by Cursor in favor of `.cursor/rules/` directory, but still supported
- thiserror 1.x: Superseded by 2.x with cleaner derive syntax

## Tool Detection Marker Reference

| Tool | Marker | Type | Confidence | Notes |
|------|--------|------|------------|-------|
| Claude Code | `CLAUDE.md` | File | High | Unambiguous marker |
| Claude Code | `.claude/` | Directory | High | Contains settings.json, rules, etc. |
| Cursor | `.cursor/rules/` | Directory | High | Contains .mdc rule files |
| Cursor | `.cursorrules` | File | High (legacy) | Deprecated format, flag for migration |
| OpenCode | `opencode.json` | File | High | Unambiguous config file |
| OpenCode | `AGENTS.md` | File | Medium (alone) | Ambiguous -- also used by Copilot and others |
| OpenCode | `AGENTS.md` + `opencode.json` | Both | High | Corroborating evidence |

## Open Questions

1. **tempfile crate for tests**
   - What we know: Tests need temporary directories for detection testing. `tempfile` crate is the standard approach.
   - What's unclear: Whether to use tempfile or the fixtures/ directory approach.
   - Recommendation: Use tempfile for unit tests (isolation), fixtures/ directory for integration tests (realistic project layouts). Add `tempfile` as a dev-dependency.

2. **Config file location discovery**
   - What we know: Phase 1 parses aisync.toml when given a path. Phase 2 will need to discover the file.
   - What's unclear: Whether to implement discovery (walk up directories) now or later.
   - Recommendation: Phase 1 takes an explicit path. Discovery logic belongs in Phase 2 with the CLI.

## Sources

### Primary (HIGH confidence)
- [Cargo Workspaces - The Rust Book](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html) - Workspace setup patterns
- [Rust 2024 Edition](https://www.developer-tech.com/news/rust-1-85-0-released-2024-edition-stabilised/) - Edition 2024 features and resolver v3
- [OpenCode Config Docs](https://opencode.ai/docs/config/) - opencode.json format
- [OpenCode Rules Docs](https://opencode.ai/docs/rules/) - AGENTS.md usage
- [Cursor Rules Docs](https://cursor.com/docs/context/rules) - .mdc format and .cursor/rules/

### Secondary (MEDIUM confidence)
- [Enum or Trait Object - Possible Rust](https://www.possiblerust.com/guide/enum-or-trait-object) - Dispatch pattern comparison
- [Large Rust Workspaces - matklad](https://matklad.github.io/2021/08/22/large-rust-workspaces.html) - crates/ directory convention
- [Claude Code CLAUDE.md Guide](https://claude.com/blog/using-claude-md-files) - CLAUDE.md and .claude/ structure

### Tertiary (LOW confidence)
- None -- all findings verified with primary or secondary sources

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All crates are mature (serde 1.x, toml 1.x, thiserror 2.x), verified via cargo search
- Architecture: HIGH - Workspace patterns well-documented; enum dispatch is textbook Rust for closed type sets
- Pitfalls: HIGH - AGENTS.md ambiguity is a known ecosystem issue; Cursor format migration is documented
- Tool markers: HIGH - Verified against official tool documentation

**Research date:** 2026-03-05
**Valid until:** 2026-04-05 (stable ecosystem, slow-moving tool configs)
