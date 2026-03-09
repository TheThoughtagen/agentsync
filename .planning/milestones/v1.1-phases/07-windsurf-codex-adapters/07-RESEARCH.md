# Phase 7: Windsurf & Codex Adapters - Research

**Researched:** 2026-03-08
**Domain:** Rust adapter implementation for Windsurf and Codex AI tools
**Confidence:** HIGH

## Summary

Phase 7 adds two new built-in adapters (Windsurf, Codex) to the existing adapter system established in Phase 6. The codebase is well-structured for this: `ToolAdapter` trait with default implementations, `AnyAdapter` enum with `dispatch_adapter!` macro, and `ToolKind` enum with `Custom(String)` variant already handling arbitrary tool names via deserialization.

The core challenge is not the adapter implementation itself (the pattern is well-established across three existing adapters), but rather: (1) Windsurf uses a Generate strategy with YAML frontmatter similar to Cursor but with different fields (`trigger` instead of `alwaysApply`), (2) Codex shares `AGENTS.md` with OpenCode requiring deduplication in SyncEngine, and (3) both tools need ToolKind variants promoted from `Custom(String)` to named variants.

**Primary recommendation:** Follow the Cursor adapter pattern for Windsurf (Generate strategy, YAML frontmatter, directory creation) and the OpenCode adapter pattern for Codex (symlink to `.ai/instructions.md`), with SyncEngine deduplication added for the shared AGENTS.md target.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| ADPT-01 | Windsurf adapter generates `.windsurf/rules/project.md` with correct YAML frontmatter | Windsurf frontmatter format documented; Cursor adapter provides exact pattern to follow |
| ADPT-02 | Codex adapter symlinks `AGENTS.md` to `.ai/instructions.md` | OpenCode adapter provides exact symlink pattern; Codex uses AGENTS.md natively |
| ADPT-03 | Codex detected via `.codex/` directory, disambiguated from OpenCode | Detection pattern established; `.codex/` is unique marker; OpenCode uses `opencode.json` |
| ADPT-04 | SyncEngine deduplicates identical AGENTS.md symlink actions when both Codex and OpenCode are present | SyncEngine.plan() iterates enabled_tools(); dedup logic needed at action collection level |
| ADPT-05 | Legacy `.windsurfrules` file detected with migration hint to modern format | Cursor adapter's `.cursorrules` legacy detection provides exact pattern |
| ADPT-06 | Content size limit warnings for Windsurf (12K chars) and Codex (32 KiB) | Limits verified from official sources; warning action type exists (WarnUnsupportedHooks pattern) |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| aisync-core | workspace | Core adapter trait, types, sync engine | This is the project |
| tempfile | existing dep | Test fixtures with temp directories | Already used in all adapter tests |
| serde/toml | existing deps | Config serialization | Already used throughout |

### Supporting
No new dependencies needed. All required functionality exists in the workspace.

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Named ToolKind variants | Keep Custom("windsurf") | Named variants enable pattern matching in SyncEngine; worth the small enum growth |
| New SyncAction::WarnSizeLimit | Reuse WarnUnsupportedHooks | Dedicated variant is clearer; WarnUnsupportedHooks has wrong semantics |

## Architecture Patterns

### Recommended Project Structure
```
crates/aisync-core/src/
  adapters/
    mod.rs            # Add windsurf + codex modules
    windsurf.rs       # NEW: WindsurfAdapter impl
    codex.rs          # NEW: CodexAdapter impl
    claude_code.rs    # Existing
    cursor.rs         # Existing (reference for Windsurf)
    opencode.rs       # Existing (reference for Codex)
  adapter.rs          # Add Windsurf/Codex to AnyAdapter enum + dispatch
  types.rs            # Add Windsurf/Codex to ToolKind enum
  sync.rs             # Add AGENTS.md deduplication logic
  detection.rs        # Automatically picks up new adapters via all_builtin()
fixtures/
  windsurf-only/      # NEW: .windsurf/rules/ directory
  codex-only/         # NEW: .codex/ directory
  windsurf-legacy/    # NEW: .windsurfrules file
  codex-opencode/     # NEW: both .codex/ and opencode.json
```

### Pattern 1: Windsurf Adapter (mirrors Cursor adapter)
**What:** Generate strategy adapter producing `.windsurf/rules/project.md` with YAML frontmatter
**When to use:** Windsurf detected via `.windsurf/rules/` directory or legacy `.windsurfrules`
**Key differences from Cursor:**
- Output path: `.windsurf/rules/project.md` (not `.mdc`)
- Frontmatter fields: `trigger: always_on` (not `alwaysApply: true`)
- Frontmatter also includes: `description` field
- No `globs` field needed (Windsurf uses `trigger: always_on` for global rules)

```rust
// Windsurf YAML frontmatter format
const WINDSURF_FRONTMATTER: &str = "\
---
trigger: always_on
description: Project instructions synced by aisync
---

";

fn generate_windsurf_content(canonical_content: &str) -> String {
    format!("{WINDSURF_FRONTMATTER}{canonical_content}")
}
```

### Pattern 2: Codex Adapter (mirrors OpenCode adapter)
**What:** Symlink strategy adapter creating `AGENTS.md -> .ai/instructions.md`
**When to use:** Codex detected via `.codex/` directory
**Key insight:** Codex and OpenCode both target `AGENTS.md` -- identical output path

```rust
// Detection: .codex/ directory is the unique marker
fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError> {
    let codex_dir = project_root.join(".codex");
    let detected = codex_dir.is_dir();
    Ok(DetectionResult {
        tool: ToolKind::Codex,
        detected,
        confidence: Confidence::High,
        markers_found: if detected { vec![codex_dir] } else { vec![] },
        version_hint: None,
    })
}
```

### Pattern 3: AGENTS.md Deduplication in SyncEngine
**What:** When both Codex and OpenCode produce identical AGENTS.md symlink actions, emit only one
**When to use:** Both tools enabled and detected
**Where:** `SyncEngine::plan()` -- after collecting all tool results, deduplicate AGENTS.md actions

```rust
// Deduplication approach: track seen output paths, skip duplicate symlink actions
// The first adapter to claim AGENTS.md wins; the second gets an empty action list
// Alternative: deduplicate at action level after collection
```

### Pattern 4: Content Size Warnings
**What:** Check content length before/after sync and emit warnings
**When to use:** Always, during plan_sync
**Limits:** Windsurf: 12,000 chars per rule file, Codex: 32,768 bytes (32 KiB)

```rust
// New SyncAction variant
SyncAction::WarnContentSize {
    tool: ToolKind,
    path: PathBuf,
    actual_size: usize,
    limit: usize,
    unit: String, // "chars" or "bytes"
}
```

### Anti-Patterns to Avoid
- **Codex detection via AGENTS.md alone:** AGENTS.md is ambiguous (could be OpenCode, could be hand-written). Only `.codex/` directory is a reliable Codex marker.
- **Modifying OpenCode detection:** OpenCode already handles AGENTS.md with Medium confidence. Do NOT change this. Codex detection is independent.
- **Duplicating plan_sync logic:** The OpenCode adapter's symlink logic is complex (conditionals, copy strategy). Codex should reuse the same patterns, possibly via shared helper functions.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| YAML frontmatter generation | Custom YAML serializer | String constant (like Cursor adapter) | Frontmatter is static; no dynamic fields needed |
| Symlink creation logic | New symlink code | Follow OpenCode's exact plan_sync pattern | Edge cases (existing file, wrong symlink, conditionals) already handled |
| Tool registration | Manual adapter lists | Add to `AnyAdapter::all_builtin()` and `for_tool()` | Single point of registration |
| Legacy file detection | Separate detection pass | `version_hint` field in DetectionResult | Cursor adapter already demonstrates this pattern |

**Key insight:** Every pattern needed for Phase 7 already exists in the codebase. Windsurf = Cursor clone with different frontmatter. Codex = OpenCode clone with different detection. The only genuinely new logic is AGENTS.md deduplication.

## Common Pitfalls

### Pitfall 1: Forgetting to Update All Dispatch Points
**What goes wrong:** Adding new ToolKind variants without updating all match arms
**Why it happens:** ToolKind is matched in multiple places (SyncEngine hook path resolution, memory status, display_name, as_str, serde)
**How to avoid:** Use `#[non_exhaustive]` or ensure the compiler catches missing arms. Search for `ToolKind::OpenCode` to find all match sites.
**Warning signs:** Compiler warnings about non-exhaustive patterns (if using _ catch-all, these are silent!)

### Pitfall 2: AGENTS.md Ownership Conflict
**What goes wrong:** Both Codex and OpenCode try to create/modify AGENTS.md, resulting in duplicate actions or symlink conflicts
**Why it happens:** Both tools legitimately use AGENTS.md as their native format
**How to avoid:** Deduplication at SyncEngine level before executing actions. First adapter claims the path; second gets informed it's already handled.
**Warning signs:** `aisync sync` creates two entries for AGENTS.md in the report

### Pitfall 3: Codex Detection False Positives
**What goes wrong:** `.codex/` directory exists but is from a different tool or manually created
**Why it happens:** Directory name collision
**How to avoid:** Use High confidence for `.codex/` directory (it's sufficiently unique). Do NOT detect via AGENTS.md (that's OpenCode's medium-confidence path).

### Pitfall 4: Windsurf Frontmatter Field Names
**What goes wrong:** Using Cursor's `alwaysApply: true` instead of Windsurf's `trigger: always_on`
**Why it happens:** Formats look similar but have different field names
**How to avoid:** Use the documented Windsurf frontmatter from official Windsurf-Samples catalog

### Pitfall 5: Size Limit Units Mismatch
**What goes wrong:** Comparing bytes when limit is in chars, or vice versa
**Why it happens:** Windsurf limit is 12K chars, Codex limit is 32 KiB (bytes)
**How to avoid:** Windsurf: `content.chars().count()`, Codex: `content.len()` (bytes)

## Code Examples

### Windsurf Adapter - plan_sync (Generate strategy)
```rust
// Mirrors cursor.rs almost exactly
fn plan_sync(
    &self,
    project_root: &Path,
    canonical_content: &str,
    _strategy: SyncStrategy,
) -> Result<Vec<SyncAction>, AisyncError> {
    let output_path = project_root.join(".windsurf/rules/project.md");
    let expected_content = generate_windsurf_content(canonical_content);
    let mut actions = Vec::new();

    let rules_dir = project_root.join(".windsurf/rules");
    if !rules_dir.is_dir() {
        actions.push(SyncAction::CreateDirectory { path: rules_dir });
    }

    if output_path.exists() {
        let existing = std::fs::read_to_string(&output_path)
            .map_err(|e| /* ... */)?;
        if existing == expected_content {
            return Ok(vec![]);
        }
    }

    // Size warning check
    if canonical_content.chars().count() > 12_000 {
        actions.push(SyncAction::WarnContentSize { /* ... */ });
    }

    actions.push(SyncAction::CreateFile {
        path: output_path,
        content: expected_content,
    });
    Ok(actions)
}
```

### Codex Adapter - detect (disambiguated from OpenCode)
```rust
fn detect(&self, project_root: &Path) -> Result<DetectionResult, AisyncError> {
    let codex_dir = project_root.join(".codex");
    let detected = codex_dir.is_dir();
    Ok(DetectionResult {
        tool: ToolKind::Codex,
        detected,
        confidence: Confidence::High,
        markers_found: if detected { vec![codex_dir] } else { vec![] },
        version_hint: None,
    })
}
```

### AnyAdapter Enum Extension
```rust
pub enum AnyAdapter {
    ClaudeCode(ClaudeCodeAdapter),
    Cursor(CursorAdapter),
    OpenCode(OpenCodeAdapter),
    Windsurf(WindsurfAdapter),   // NEW
    Codex(CodexAdapter),         // NEW
    Plugin(Arc<dyn ToolAdapter>),
}

// dispatch_adapter! macro needs two new arms
macro_rules! dispatch_adapter {
    ($self:expr, $inner:ident => $body:expr) => {
        match $self {
            AnyAdapter::ClaudeCode($inner) => $body,
            AnyAdapter::Cursor($inner) => $body,
            AnyAdapter::OpenCode($inner) => $body,
            AnyAdapter::Windsurf($inner) => $body,  // NEW
            AnyAdapter::Codex($inner) => $body,      // NEW
            AnyAdapter::Plugin($inner) => $body,
        }
    };
}
```

### ToolKind Enum Extension
```rust
pub enum ToolKind {
    ClaudeCode,
    Cursor,
    OpenCode,
    Windsurf,  // NEW -- promote from Custom("windsurf")
    Codex,     // NEW -- promote from Custom("codex")
    Custom(String),
}

// Update as_str, display_name, Serialize, Deserialize
impl ToolKind {
    pub fn as_str(&self) -> &str {
        match self {
            ToolKind::Windsurf => "windsurf",
            ToolKind::Codex => "codex",
            // ... existing arms
        }
    }
    pub fn display_name(&self) -> &str {
        match self {
            ToolKind::Windsurf => "Windsurf",
            ToolKind::Codex => "Codex",
            // ... existing arms
        }
    }
}
```

### SyncEngine AGENTS.md Deduplication
```rust
// In SyncEngine::plan(), after collecting all tool results:
// Approach: track which output paths have been claimed
fn deduplicate_actions(results: &mut Vec<ToolSyncResult>) {
    let mut claimed_paths: HashSet<PathBuf> = HashSet::new();
    for result in results.iter_mut() {
        result.actions.retain(|action| {
            let path = match action {
                SyncAction::CreateSymlink { link, .. } => Some(link.clone()),
                SyncAction::CreateFile { path, .. } => Some(path.clone()),
                _ => None,
            };
            if let Some(p) = path {
                claimed_paths.insert(p) // returns true if newly inserted
            } else {
                true // keep non-path actions
            }
        });
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `.windsurfrules` single file | `.windsurf/rules/*.md` directory | Windsurf Wave 8 (2025) | Must detect legacy and hint migration |
| Codex used GitHub Copilot format | Codex uses AGENTS.md | Codex CLI launch (2025) | Shares format with OpenCode |
| ToolKind had no Windsurf/Codex | Custom(String) variant exists | Phase 6 (this project) | Promoting to named variants |

**Deprecated/outdated:**
- `.windsurfrules`: Legacy single-file format. Detect and suggest migration to `.windsurf/rules/`.

## Open Questions

1. **Windsurf frontmatter field: `trigger` vs `activation`**
   - What we know: Official Windsurf-Samples catalog uses `trigger: always_on`. Some third-party sources show `activation: "always-on"`.
   - What's unclear: Whether both are accepted or only one is canonical.
   - Recommendation: Use `trigger: always_on` (from official catalog). LOW risk -- Windsurf likely accepts both.

2. **Codex symlink support robustness**
   - What we know: GitHub issue #11314 shows Codex had bugs with symlinked paths. Issue may be resolved.
   - What's unclear: Whether Codex reliably follows AGENTS.md symlinks in all cases.
   - Recommendation: Default to symlink strategy (matches OpenCode behavior). If issues arise, fall back to Copy strategy. The adapter already supports both via the `plan_sync_with_conditionals` pattern.

3. **Windsurf GenerateMdc vs CreateFile action type**
   - What we know: Cursor uses `SyncAction::GenerateMdc` specifically. Windsurf output is `.md`, not `.mdc`.
   - Recommendation: Use `SyncAction::CreateFile` for Windsurf (it generates a regular .md file, not Cursor's custom .mdc format). This is simpler and avoids overloading the GenerateMdc variant.

## Sources

### Primary (HIGH confidence)
- Existing codebase: adapter.rs, adapters/*.rs, sync.rs, types.rs, config.rs, detection.rs -- full pattern reference
- [Windsurf-Samples/cascade-customizations-catalog](https://github.com/Windsurf-Samples/cascade-customizations-catalog) -- official Windsurf frontmatter format
- [OpenAI Codex AGENTS.md docs](https://developers.openai.com/codex/guides/agents-md/) -- Codex instruction format
- [OpenAI Codex config reference](https://developers.openai.com/codex/config-reference) -- project_doc_max_bytes setting

### Secondary (MEDIUM confidence)
- [Codex AGENTS.md truncation issue #7138](https://github.com/openai/codex/issues/7138) -- 32 KiB limit confirmed
- [Codex advanced config](https://developers.openai.com/codex/config-advanced/) -- .codex/ directory structure
- [DEV Community Windsurf rules article](https://dev.to/yardenporat/codium-windsurf-ide-rules-file-1hn9) -- legacy .windsurfrules format

### Tertiary (LOW confidence)
- Windsurf `trigger` vs `activation` field name -- third-party sources conflict; using official catalog

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies, all patterns exist in codebase
- Architecture: HIGH - direct extension of existing adapter system
- Pitfalls: HIGH - deduplication is the only non-obvious challenge; all others follow existing patterns
- Windsurf frontmatter format: MEDIUM - official catalog confirms `trigger: always_on` but limited docs

**Research date:** 2026-03-08
**Valid until:** 2026-04-08 (stable -- tool formats unlikely to change rapidly)
