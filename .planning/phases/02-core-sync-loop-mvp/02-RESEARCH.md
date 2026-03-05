# Phase 2: Core Sync Loop (MVP) - Research

**Researched:** 2026-03-05
**Domain:** CLI scaffolding, filesystem sync (symlinks/generation), interactive prompts, content hashing
**Confidence:** HIGH

## Summary

Phase 2 transforms the Phase 1 foundation (types, config, detection) into a working CLI with three commands: `init`, `sync`, and `status`. The core technical challenges are: (1) building a clap-based CLI with subcommands in the `aisync` binary crate, (2) implementing symlink-based sync for Claude Code and OpenCode with generated `.mdc` output for Cursor, (3) interactive import/merge during init using dialoguer, (4) SHA-256 content hashing for drift detection, and (5) colored terminal output with `--json` alternative.

The existing codebase provides strong foundations: `AnyAdapter` enum dispatch, `ToolAdapter` trait (needs extension with read/write methods), `DetectionEngine::scan()`, `AisyncConfig` parsing, and `SyncStrategy` enum. Phase 2 extends these rather than replacing them.

**Primary recommendation:** Add `clap` (derive), `dialoguer`, `sha2`, `colored`, and `serde_json` to workspace dependencies. Extend `ToolAdapter` trait with `read_instructions()`, `write_instructions()`, and `sync_status()` methods. Build sync engine as a new `sync` module in `aisync-core`, with CLI commands in the `aisync` binary crate.

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Import strategy**: Interactive merge -- when multiple existing configs exist (CLAUDE.md + .cursor/rules/), show a diff of all found configs side-by-side and let user pick sections from each
- **Scaffold contents**: Full scaffold -- create `.ai/instructions.md`, `.ai/memory/`, `.ai/hooks/`, `.ai/commands/`, and `aisync.toml` even if subdirectories are empty. Ready for Phase 3+
- **Tool detection**: Auto-detect + confirm -- run detection engine, show results ("Found: Claude Code, Cursor"), ask user to confirm before writing config
- **Re-init behavior**: Offer re-init -- warn and ask "Re-initialize? This will overwrite aisync.toml and re-import instructions." Allows user to reset
- **No tools detected**: Proceed anyway -- create `.ai/` with empty tool config. User can add tools later. Low friction
- **Symlink direction**: Tool file -> .ai/ -- CLAUDE.md and AGENTS.md are symlinks pointing to `.ai/instructions.md`. Canonical file is the real file, tool files are symlinks
- **Existing file handling**: Prompt interactively -- when sync finds an existing non-symlink tool file, ask the user: "CLAUDE.md exists. Replace with symlink? [y/N]"
- **Cursor .mdc frontmatter**: Minimal -- just `description` and `globs: '**'`, enough for Cursor to load the rule
- **.gitignore management**: Auto-add with marker -- append a managed section (`# aisync-managed` ... `# /aisync-managed`) with entries for symlinked/generated files
- **Idempotency**: Running `aisync sync` twice produces identical results -- symlinks verified, .mdc regenerated only if content changed
- **Drift detection**: Content hash (SHA-256) -- compare file contents regardless of filesystem. For symlinks: verify target + hash. For generated files: compare content hash
- **Default output**: Colored table -- Tool | Strategy | Status (checkmark/X) | Details. Green/red for quick scanning
- **All-in-sync output**: Single summary line -- "All 3 tools in sync" when everything is clean. Full table only when drift detected
- **JSON flag**: Yes, from day one -- `--json` outputs structured JSON for scripts and CI
- **Symlink validation**: Detect dangling symlinks and report in status
- **Partial sync failure**: Continue + report -- sync all tools, collect errors, report failures at the end. Exit code reflects failures (non-zero if any tool failed)
- **--dry-run output**: Action list -- one line per planned action: "Would create symlink: CLAUDE.md -> .ai/instructions.md"
- **--verbose**: Claude's Discretion -- pick whatever format is clearest for debugging

### Claude's Discretion
- Verbose output format (structured debug vs narrative)
- Interactive merge UI implementation details (dialoguer prompts, diff rendering)
- Exact .mdc template beyond minimal frontmatter
- Internal module organization for CLI commands
- Temp file handling during sync operations

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope

</user_constraints>

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CLI-01 | `aisync init` scaffolds `.ai/` with interactive detection and import | clap subcommand + dialoguer prompts + DetectionEngine reuse |
| CLI-02 | `aisync sync` one-shot sync from `.ai/` to all configured tools | Sync engine module + per-adapter write methods |
| CLI-03 | `aisync sync --dry-run` preview changes without applying | Dry-run plan collector pattern |
| CLI-04 | `aisync status` per-tool sync state and drift detection | SHA-256 content hashing + symlink validation |
| CLI-09 | All sync operations idempotent | Symlink target check + content hash comparison before write |
| CLI-11 | Clear error messages with `--verbose` flag | thiserror + clap global flag |
| INST-01 | `.ai/instructions.md` syncs to CLAUDE.md via symlink | `std::os::unix::fs::symlink` with copy fallback |
| INST-02 | `.ai/instructions.md` syncs to AGENTS.md via symlink | Same symlink mechanism as INST-01 |
| INST-03 | `.ai/instructions.md` generates `.cursor/rules/project.mdc` with YAML frontmatter | `.mdc` generation with `---` frontmatter block |
| INST-04 | Symlink by default on macOS/Linux, copy fallback on Windows | `cfg!(target_family)` conditional compilation |
| INST-05 | `aisync init` imports existing CLAUDE.md/AGENTS.md/.mdc | Adapter `read_instructions()` methods |
| INST-06 | Import prompts user when multiple existing configs conflict | dialoguer `Select`/`Confirm` prompts |
| INST-07 | `.gitignore` entries managed for tool-generated files | Managed section with markers in .gitignore |
| INST-10 | Symlink targets validated in `aisync status` | `fs::read_link()` + target existence check |
| ADPT-01 | Claude Code adapter -- instructions sync | Extend `ClaudeCodeAdapter` with read/write for CLAUDE.md |
| ADPT-02 | OpenCode adapter -- AGENTS.md sync | Extend `OpenCodeAdapter` with read/write for AGENTS.md |
| ADPT-03 | Cursor adapter -- .mdc generation with frontmatter | Extend `CursorAdapter` with generate for .cursor/rules/project.mdc |

</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.5 | CLI argument parsing with derive macros | De facto Rust CLI standard; derive API eliminates boilerplate |
| dialoguer | 0.12 | Interactive terminal prompts (Confirm, Select, Input) | console-rs ecosystem; pairs with colored output |
| sha2 | 0.10 | SHA-256 content hashing for drift detection | RustCrypto standard; pure Rust, no C deps |
| colored | 3.1 | Colored terminal output | Simple trait-based API; CLICOLOR support |
| serde_json | 1.0 | JSON output for `--json` flag | Ecosystem standard for JSON serialization |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| hex | 0.4 | Encode SHA-256 hash bytes to hex string | Display and compare content hashes |
| serde (workspace) | 1.0 | Serialization for JSON output types | Already in workspace deps |
| toml (workspace) | 0.8 | Config file generation during init | Already in workspace deps |
| thiserror (workspace) | 2.0 | Error type definitions | Already in workspace deps |
| tempfile (dev) | 3.14 | Test fixture creation | Already in dev-dependencies |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| colored | owo-colors | owo-colors is zero-alloc but colored has broader ecosystem adoption and simpler API |
| dialoguer | inquire | inquire has nicer UI but dialoguer is part of console-rs ecosystem (same authors as colored) |
| sha2 | ring | ring is faster but pulls in C code; sha2 is pure Rust and sufficient for small files |

**Installation (workspace Cargo.toml):**
```toml
[workspace.dependencies]
clap = { version = "4.5", features = ["derive"] }
dialoguer = "0.12"
sha2 = "0.10"
colored = "3.1"
serde_json = "1.0"
hex = "0.4"
```

## Architecture Patterns

### Recommended Module Structure
```
crates/aisync-core/src/
  adapter.rs          # ToolAdapter trait (extend with read/write/status)
  adapters/
    claude_code.rs    # Add read_instructions, write_instructions, sync_status
    cursor.rs         # Add generate_mdc, sync_status
    opencode.rs       # Add read_instructions, write_instructions, sync_status
  config.rs           # AisyncConfig (existing)
  detection.rs        # DetectionEngine (existing)
  error.rs            # Extend with SyncError, InitError variants
  init.rs             # NEW: init engine -- scaffold + import logic
  sync.rs             # NEW: sync engine -- orchestrates per-tool sync
  status.rs           # NEW: status checker -- drift detection + reporting
  gitignore.rs        # NEW: .gitignore managed section logic
  types.rs            # Extend with SyncResult, StatusReport, DriftState

crates/aisync/src/
  main.rs             # Clap App with subcommands
  commands/
    mod.rs
    init.rs           # aisync init command handler
    sync.rs           # aisync sync command handler
    status.rs         # aisync status command handler
```

### Pattern 1: Clap Derive Subcommands
**What:** Use derive macros for CLI structure with enum-based subcommands
**When to use:** All CLI entry points
**Example:**
```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "aisync", version, about = "Sync AI tool configurations")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output
    #[arg(long, global = true)]
    verbose: bool,

    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize .ai/ directory with detection and import
    Init,
    /// Sync .ai/ to all configured tools
    Sync {
        /// Preview changes without applying
        #[arg(long)]
        dry_run: bool,
    },
    /// Show per-tool sync status
    Status,
}
```

### Pattern 2: Sync Action Plan (supports dry-run)
**What:** Compute actions first, then optionally execute -- enables dry-run for free
**When to use:** `aisync sync` and `aisync sync --dry-run`
**Example:**
```rust
/// A planned sync action that can be displayed (dry-run) or executed.
enum SyncAction {
    CreateSymlink { link: PathBuf, target: PathBuf },
    RemoveAndRelink { link: PathBuf, target: PathBuf },
    GenerateMdc { output: PathBuf, content: String },
    UpdateGitignore { path: PathBuf, entries: Vec<String> },
    PromptReplaceFile { path: PathBuf, target: PathBuf },
}

/// Compute what needs to happen without doing it.
fn plan_sync(config: &AisyncConfig, project_root: &Path) -> Result<Vec<SyncAction>, AisyncError> {
    // ... build action list
}

/// Execute planned actions (skip for dry-run).
fn execute_sync(actions: Vec<SyncAction>, interactive: bool) -> Result<SyncReport, AisyncError> {
    // ... execute each action, collecting results
}
```

### Pattern 3: Continue-on-Error Collection
**What:** Try all tools, collect errors, report at end
**When to use:** Sync operations where one tool failure should not block others
**Example:**
```rust
struct SyncReport {
    successes: Vec<(ToolKind, SyncAction)>,
    failures: Vec<(ToolKind, AisyncError)>,
}

impl SyncReport {
    fn exit_code(&self) -> i32 {
        if self.failures.is_empty() { 0 } else { 1 }
    }
}
```

### Pattern 4: Extend ToolAdapter via New Trait
**What:** Add a `SyncAdapter` trait that builds on existing `ToolAdapter`
**When to use:** Adding read/write/status capabilities to adapters without changing Phase 1 trait
**Example:**
```rust
/// Extension trait for sync operations. Implemented per adapter.
pub trait SyncAdapter: ToolAdapter {
    /// Read existing instructions from this tool's native format.
    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AisyncError>;

    /// Write/sync instructions to this tool's native format.
    fn write_instructions(&self, project_root: &Path, content: &str, strategy: SyncStrategy) -> Result<SyncAction, AisyncError>;

    /// Check sync status for this tool.
    fn sync_status(&self, project_root: &Path, canonical_hash: &str) -> Result<ToolSyncStatus, AisyncError>;
}
```

### Anti-Patterns to Avoid
- **Mutating ToolAdapter trait for Phase 2:** Add `SyncAdapter` as a separate trait or extend `ToolAdapter` with default methods. Do not break Phase 1 detect-only contract with required methods that detection does not need.
- **Hardcoding file paths:** Use constants or config for paths like `CLAUDE.md`, `AGENTS.md`, `.cursor/rules/project.mdc`. Makes testing with fixtures simpler.
- **Blocking on interactive prompts in library code:** Keep dialoguer prompts in the CLI binary crate (`aisync`), not in `aisync-core`. The core library should accept decisions as parameters.
- **Testing symlinks on filesystem directly:** Use tempfile-based fixtures. Symlink behavior varies by OS; isolate with temp directories.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CLI argument parsing | Manual arg parsing (current main.rs) | clap 4.5 derive | Handles help text, validation, completions, subcommands |
| Interactive prompts | Raw stdin reading | dialoguer 0.12 | Handles terminal modes, cursor, ANSI escapes, edge cases |
| Content hashing | Manual hash implementation | sha2 0.10 + hex 0.4 | Cryptographic correctness, performance |
| Colored terminal output | Manual ANSI escape codes | colored 3.1 | Handles CLICOLOR, NO_COLOR, Windows terminal compat |
| JSON serialization | String formatting | serde_json 1.0 | Correct escaping, nested structures, derives |
| .gitignore parsing | Regex-based section finding | Simple marker-based append/replace | .gitignore is line-based; markers (`# aisync-managed`) make section management reliable |

**Key insight:** The current `main.rs` manually parses args -- replacing with clap derive is the first step and unlocks all CLI requirements (help, version, global flags, subcommands).

## Common Pitfalls

### Pitfall 1: Symlink Relativity
**What goes wrong:** Creating symlinks with absolute paths makes projects non-portable (breaks when project directory moves)
**Why it happens:** `std::os::unix::fs::symlink(target, link)` takes the target path as-is
**How to avoid:** Always compute relative paths from the symlink location to the target. E.g., `CLAUDE.md -> .ai/instructions.md` (relative), not `CLAUDE.md -> /home/user/project/.ai/instructions.md` (absolute)
**Warning signs:** Symlinks break after `git clone` or directory rename

### Pitfall 2: Symlink Exists But Wrong Target
**What goes wrong:** `symlink()` fails with "file exists" when re-running sync, even if the symlink is correct
**Why it happens:** Need to check existing symlink target before deciding to recreate
**How to avoid:** Check with `fs::read_link()` first. If target matches, skip. If wrong target, remove and recreate. If regular file (not symlink), prompt user.
**Warning signs:** `aisync sync` fails on second run (breaks idempotency)

### Pitfall 3: .mdc Frontmatter Format
**What goes wrong:** Cursor does not load the rule if frontmatter is malformed
**Why it happens:** The YAML frontmatter must be between `---` delimiters with specific field names
**How to avoid:** Use exact format: `---\ndescription: <desc>\nglobs: \"**\"\nalwaysApply: true\n---\n\n<content>`. Quote glob patterns. Use `alwaysApply: true` for project-wide rules.
**Warning signs:** Rule appears in `.cursor/rules/` but Cursor does not apply it

### Pitfall 4: .gitignore Managed Section Corruption
**What goes wrong:** Multiple runs create duplicate managed sections, or user edits within the managed section get lost
**Why it happens:** Marker-based section management needs careful find-and-replace logic
**How to avoid:** Parse .gitignore, find markers, replace entire section between markers. If no markers exist, append. Never partially update.
**Warning signs:** Duplicate `# aisync-managed` lines in .gitignore

### Pitfall 5: Interactive Prompts in Non-TTY
**What goes wrong:** `dialoguer` panics or hangs when stdin is not a terminal (CI, piped input)
**Why it happens:** Interactive prompts require a TTY
**How to avoid:** Check `atty::is(Stream::Stdin)` or `std::io::stdin().is_terminal()` (Rust 1.70+). In non-TTY mode, use defaults or require `--yes`/`--force` flags. For sync, non-interactive mode should skip prompts and refuse to overwrite existing files without explicit flag.
**Warning signs:** CI hangs on `aisync init`

### Pitfall 6: SHA-256 Comparison of Symlinked Files
**What goes wrong:** Hashing the symlink itself vs the target file gives different results
**Why it happens:** Need to follow symlinks when reading content for hashing
**How to avoid:** Always use `fs::read()` which follows symlinks by default. For status, hash the canonical `.ai/instructions.md` content and compare with what the tool file resolves to.
**Warning signs:** Status shows drift when files are actually in sync

## Code Examples

### Symlink Creation (Unix, relative path)
```rust
use std::os::unix::fs as unix_fs;
use std::path::Path;

fn create_relative_symlink(link: &Path, target: &Path) -> std::io::Result<()> {
    // Compute relative path from link's parent to target
    let link_parent = link.parent().unwrap();
    let relative_target = pathdiff::diff_paths(target, link_parent)
        .unwrap_or_else(|| target.to_path_buf());

    // Remove existing symlink if present
    if link.symlink_metadata().is_ok() {
        std::fs::remove_file(link)?;
    }

    unix_fs::symlink(&relative_target, link)
}
```
Note: `pathdiff` crate (0.2) provides `diff_paths()` for relative path computation. Alternatively, implement manually for the simple `.ai/instructions.md` case where the relative path is always `.ai/instructions.md`.

### SHA-256 Content Hash
```rust
use sha2::{Sha256, Digest};

fn content_hash(path: &Path) -> Result<String, std::io::Error> {
    let content = std::fs::read(path)?; // follows symlinks
    let hash = Sha256::digest(&content);
    Ok(hex::encode(hash))
}
```

### Cursor .mdc Generation
```rust
fn generate_mdc(instructions: &str, description: &str) -> String {
    format!(
        "---\ndescription: {description}\nglobs: \"**\"\nalwaysApply: true\n---\n\n{instructions}"
    )
}
```

### .gitignore Managed Section
```rust
const MARKER_START: &str = "# aisync-managed";
const MARKER_END: &str = "# /aisync-managed";

fn update_gitignore(gitignore_path: &Path, entries: &[&str]) -> std::io::Result<()> {
    let content = std::fs::read_to_string(gitignore_path).unwrap_or_default();

    let managed_section = format!(
        "{MARKER_START}\n{}\n{MARKER_END}",
        entries.join("\n")
    );

    let new_content = if let Some(start_idx) = content.find(MARKER_START) {
        if let Some(end_idx) = content.find(MARKER_END) {
            let end_pos = end_idx + MARKER_END.len();
            format!("{}{}{}", &content[..start_idx], managed_section, &content[end_pos..])
        } else {
            // Broken markers -- replace from start marker to end of file
            format!("{}{}", &content[..start_idx], managed_section)
        }
    } else {
        // No existing section -- append
        if content.is_empty() || content.ends_with('\n') {
            format!("{content}{managed_section}\n")
        } else {
            format!("{content}\n{managed_section}\n")
        }
    };

    std::fs::write(gitignore_path, new_content)
}
```

### Interactive Tool Confirmation (dialoguer)
```rust
use dialoguer::Confirm;

fn confirm_detected_tools(tools: &[DetectionResult]) -> bool {
    let tool_list: Vec<String> = tools.iter()
        .map(|t| format!("{:?}", t.tool))
        .collect();

    println!("Detected tools: {}", tool_list.join(", "));

    Confirm::new()
        .with_prompt("Proceed with these tools?")
        .default(true)
        .interact()
        .unwrap_or(false)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual arg parsing in main.rs | clap 4.5 derive macros | Phase 2 | Replaces entire main.rs arg handling |
| `.cursorrules` (single file) | `.cursor/rules/*.mdc` (directory with frontmatter) | Cursor 2024 | Must generate `.mdc` format, not legacy |
| `std::fs::soft_link` (deprecated) | `std::os::unix::fs::symlink` | Rust 1.1+ | Use platform-specific symlink APIs |
| Detect-only adapters | Read/write/status adapters | Phase 2 | Extends Phase 1 trait with sync capabilities |

**Deprecated/outdated:**
- `.cursorrules` file: Legacy Cursor format. Generate `.cursor/rules/project.mdc` instead
- `std::fs::soft_link`: Deprecated in favor of platform-specific `symlink` functions

## Open Questions

1. **`pathdiff` crate vs manual relative path**
   - What we know: Relative symlinks needed for portability. `pathdiff` 0.2 provides `diff_paths()`.
   - What's unclear: Whether to add another dependency for a function that could be hand-written for the simple case (link at project root, target at `.ai/instructions.md`)
   - Recommendation: For the MVP, the relative path is always predictable (`.ai/instructions.md`). Hard-code it. Add `pathdiff` later if needed for complex scenarios.

2. **Non-interactive mode for CI**
   - What we know: dialoguer needs a TTY. CI pipelines are non-TTY.
   - What's unclear: Whether `--yes` or `--force` flag is the right UX for skipping prompts
   - Recommendation: Use `std::io::stdin().is_terminal()` to detect non-TTY. In non-TTY mode for `sync`, default to safe behavior (skip files that need prompts, report as skipped). For `init`, require `--yes` flag to proceed non-interactively.

3. **SyncAdapter trait vs extending ToolAdapter**
   - What we know: Current `ToolAdapter` has only `detect()` and `name()`. Phase 2 needs read/write/status.
   - What's unclear: Whether to add methods with default implementations to `ToolAdapter` or create a separate `SyncAdapter` trait
   - Recommendation: Extend `ToolAdapter` with new methods. The adapter set is fixed and small (3 adapters). A separate trait adds complexity without benefit. Add methods with `todo!()` defaults during transition if needed.

## Sources

### Primary (HIGH confidence)
- [clap 4.5 docs](https://docs.rs/clap/latest/clap/) - derive macros, subcommands, global args
- [dialoguer docs](https://docs.rs/dialoguer/latest/dialoguer/) - Confirm, Select, Input prompts
- [sha2 docs](https://docs.rs/sha2/latest/sha2/) - SHA-256 digest API
- [std::os::unix::fs::symlink](https://doc.rust-lang.org/std/os/unix/fs/fn.symlink.html) - Unix symlink creation
- [std::os::windows::fs::symlink_file](https://doc.rust-lang.org/stable/std/os/windows/fs/fn.symlink_file.html) - Windows symlink
- Existing codebase: `crates/aisync-core/src/` -- all Phase 1 types and patterns verified by reading source

### Secondary (MEDIUM confidence)
- [Cursor .mdc format](https://forum.cursor.com/t/mdc-files-can-we-just-edit-yaml-front-matter-directly/75561) - YAML frontmatter fields (description, globs, alwaysApply)
- [Cursor rules deep dive](https://mer.vin/2025/12/cursor-ide-rules-deep-dive/) - .mdc format details
- `cargo search` -- verified latest crate versions on 2026-03-05

### Tertiary (LOW confidence)
- None -- all findings verified with official docs or crate registries

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all crates verified via `cargo search`, docs confirmed
- Architecture: HIGH - patterns derived from existing Phase 1 codebase + standard Rust CLI conventions
- Pitfalls: HIGH - symlink, frontmatter, and TTY issues are well-documented in ecosystem
- .mdc format: MEDIUM - based on community docs, not official Cursor specification

**Research date:** 2026-03-05
**Valid until:** 2026-04-05 (stable ecosystem, 30-day validity)
