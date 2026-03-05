# Technology Stack

**Project:** aisync -- Universal AI Agent Context Synchronizer
**Researched:** 2026-03-05
**Overall confidence:** MEDIUM (versions based on training data, not live-verified -- run `cargo search <crate>` to confirm)

## Recommended Stack

### Core Framework: CLI

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| clap | 4.5+ | CLI argument parsing, subcommands | Industry standard for Rust CLIs. Derive macro API eliminates boilerplate. Subcommand support maps directly to `aisync init`, `aisync sync`, `aisync watch`, etc. | HIGH |
| clap (feature: `derive`) | -- | Derive macro for struct-based CLI definitions | Cleaner than builder API, catches errors at compile time | HIGH |
| clap (feature: `env`) | -- | Env var fallback for config | Lets users set `AISYNC_*` env vars as config override | HIGH |

### Serialization & Configuration

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| serde | 1.0+ | Serialization/deserialization framework | Non-negotiable for any Rust project doing config files. Derive macros for zero-boilerplate struct mapping. | HIGH |
| serde_json | 1.0+ | JSON reading/writing | Needed for reading/writing tool configs (Cursor settings, package.json metadata) | HIGH |
| toml | 0.8+ | TOML parsing/writing | `aisync.toml` is the canonical config format. `toml` crate handles both reading and writing with serde integration. Prefer over `toml_edit` for config unless you need comment preservation. | HIGH |
| toml_edit | 0.22+ | TOML editing with format preservation | Use ONLY for modifying existing user TOML files where preserving comments/formatting matters (e.g., editing a user's existing config). Not needed for aisync's own generated files. | MEDIUM |

### File Watching

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| notify | 7.0+ (check: may still be 6.x) | Cross-platform filesystem event watching | The only serious option for Rust file watching. Wraps inotify (Linux), FSEvents (macOS), ReadDirectoryChanges (Windows). Powers `aisync watch` daemon mode. | HIGH |
| notify-debouncer-full | 0.4+ | Debounced file events with file rename tracking | Raw notify fires multiple events per save. The debouncer collapses these into single logical events. Use `full` not `mini` -- it tracks file renames which matters for detecting tool config file moves. | MEDIUM |

### Interactive Prompts & Terminal UI

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| dialoguer | 0.11+ | Interactive prompts (select, confirm, input, multi-select) | Best Rust crate for interactive CLI prompts. Powers `aisync init` wizard (tool selection, strategy choices). Built on `console` crate. | HIGH |
| indicatif | 0.17+ | Progress bars, spinners | Shows sync progress during `aisync sync`, watch status during `aisync watch`. Pairs naturally with dialoguer (same author ecosystem). | HIGH |
| console | 0.15+ | Terminal styling, colors, terminal detection | Underlying terminal abstraction used by dialoguer/indicatif. Use directly for styled output (success/error messages, status tables). | HIGH |

### Template Rendering

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| minijinja | 2.0+ | Jinja2-compatible template engine | Generates tool-native config files (.mdc for Cursor, .windsurfrules for Windsurf). Jinja2 syntax is familiar, well-documented. Lightweight -- no runtime dependency bloat. Created by Armin Ronacher (Flask/Jinja2 author). | HIGH |

### Diffing & Conflict Detection

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| similar | 2.6+ | Text diffing (unified diff, patience algorithm) | Bidirectional sync needs conflict detection. `similar` provides patience diff (better than Myers for config files) and inline diff for showing users what changed. | HIGH |

### Filesystem & Paths

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| dirs | 5.0+ | Platform-standard directory paths | Finds user config dir (`~/.config/aisync/`), home dir for global config. Cross-platform (XDG on Linux, Library on macOS, AppData on Windows). | HIGH |
| walkdir | 2.5+ | Recursive directory traversal | Scanning `.ai/` directory tree, discovering tool config files across project. More ergonomic than `std::fs::read_dir` recursion. | HIGH |
| tempfile | 3.10+ | Temporary files/directories for atomic writes | Write config to temp file, then rename -- prevents corruption on crash during sync. Also essential for integration tests. | HIGH |

### Error Handling

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| anyhow | 1.0+ | Application-level error handling with context | Use in binary/CLI code. `.context("reading aisync.toml")?` gives users actionable error messages. | HIGH |
| thiserror | 2.0+ (check: may be 1.x still) | Library-level typed errors via derive macro | Use in library/core code where callers need to match on error variants (e.g., `SyncError::ConflictDetected`). | HIGH |

### Logging & Diagnostics

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| tracing | 0.1+ | Structured logging and diagnostics | Better than `log` crate -- structured spans for tracking sync operations. `AISYNC_LOG=debug aisync sync` for debugging. | HIGH |
| tracing-subscriber | 0.3+ | Console output for tracing | `fmt` subscriber with `EnvFilter` for `AISYNC_LOG` env var control. | HIGH |

### Testing

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| assert_cmd | 2.0+ | CLI integration testing | Run `aisync` binary in tests, assert on stdout/stderr/exit code. Standard for Rust CLI testing. | HIGH |
| predicates | 3.0+ | Assertion predicates for assert_cmd | `stdout(contains("Synced 3 tools"))` style assertions. | HIGH |
| assert_fs | 1.1+ | Temporary filesystem fixtures | Create temp project dirs with tool configs, run aisync against them, verify output files. Essential for integration tests. | HIGH |
| insta | 1.39+ | Snapshot testing | Snapshot the generated .mdc, .windsurfrules, AGENTS.md files. Catch unintended output changes. | MEDIUM |

### Build & Distribution

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| cargo-dist | 0.25+ | GitHub release binary builds | Automates cross-platform binary builds and GitHub releases. Generates shell/PowerShell installers. | MEDIUM |
| cargo-release | 0.25+ | Release workflow automation | Version bumping, changelog, git tag, publish in one command. | MEDIUM |

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| CLI parsing | clap (derive) | argh | argh is simpler but lacks clap's subcommand power, shell completions, and ecosystem. aisync needs nested subcommands (`aisync memory list`). |
| CLI parsing | clap (derive) | bpaf | Smaller but less ecosystem support, less documentation, fewer contributors. |
| Template engine | minijinja | tera | Tera is heavier, has had maintenance gaps. Minijinja is lighter, faster, actively maintained by a legendary developer. |
| Template engine | minijinja | handlebars | Handlebars syntax is less powerful than Jinja2. No filters, limited control flow. |
| Config format | toml | serde_yaml | YAML has footguns (Norway problem, implicit typing). TOML is the Rust ecosystem standard. |
| File watching | notify | watchexec-events | watchexec is a CLI wrapper, not a library. notify is the underlying engine. |
| Diffing | similar | diff | `diff` crate is older, less maintained. `similar` has better algorithms (patience diff) and API. |
| Error handling | anyhow + thiserror | eyre + color-eyre | eyre/color-eyre add colored backtraces which are nice for dev but overkill for a CLI tool users run. anyhow is simpler, more widely used. |
| Logging | tracing | log + env_logger | `log` is fine but tracing's structured spans are better for debugging sync operations across multiple adapters. |
| Interactive | dialoguer | inquire | inquire has a nicer API in some ways but dialoguer is more mature and shares the console/indicatif ecosystem. |
| Directory paths | dirs | directories | `directories` is the fuller version with project-specific dirs. `dirs` is simpler and sufficient -- aisync only needs home/config dir. |
| Snapshot testing | insta | expect-test | insta has better workflow (cargo insta review), more features. |

## What NOT to Use

| Crate | Why Not |
|-------|---------|
| tokio / async-std | aisync is I/O-bound on local filesystem, not network. Async adds complexity with zero benefit. `notify` works fine with sync callbacks or channels. Keep it simple with `std::sync::mpsc`. |
| reqwest / hyper | No network requests needed. This is a local filesystem tool. |
| serde_yaml | YAML has too many parsing surprises for config files. TOML is safer and idiomatic Rust. |
| config (crate) | Over-abstracted config merging. aisync's config is simple enough that raw toml + serde handles it cleanly. |
| structopt | Deprecated in favor of clap's derive macros. Same author, merged into clap 3+. |
| prettytable-rs | Unmaintained. Use comfy-table or tabled if you need table output, but for aisync's `status` command, simple console formatting with `console` crate suffices. |

## Async Decision: Explicitly Synchronous

This is a critical architectural decision. aisync should be **synchronous** (no tokio/async-std) because:

1. **All I/O is local filesystem** -- no network calls, no HTTP, no database
2. **notify crate works with std channels** -- `std::sync::mpsc::channel()` for file events
3. **File operations are fast** -- symlink creation, config file writes are microsecond operations
4. **Complexity cost is real** -- async Rust has a steep learning curve, colored function problems, and larger binary size
5. **Watch mode uses a simple event loop** -- `recv()` on a channel, process event, repeat. No concurrency needed.

If aisync ever adds network features (e.g., remote config sync), revisit this decision then. Not now.

## Installation Commands

```bash
# Create project
cargo init aisync
cd aisync

# Core dependencies
cargo add clap --features derive,env
cargo add serde --features derive
cargo add serde_json
cargo add toml
cargo add notify
cargo add notify-debouncer-full
cargo add dialoguer
cargo add indicatif
cargo add console
cargo add minijinja
cargo add similar
cargo add dirs
cargo add walkdir
cargo add tempfile
cargo add anyhow
cargo add thiserror
cargo add tracing
cargo add tracing-subscriber --features env-filter

# Dev dependencies
cargo add --dev assert_cmd
cargo add --dev predicates
cargo add --dev assert_fs
cargo add --dev insta
```

## Minimum Supported Rust Version (MSRV)

**Recommendation:** Rust 1.75+ (stable as of Dec 2023)

Rationale: This is old enough that virtually all users have it, new enough to support all recommended crates and `async fn` in traits (if ever needed). Set in `Cargo.toml`:

```toml
[package]
rust-version = "1.75"
```

## Cargo.toml Profile Configuration

```toml
[profile.release]
opt-level = 3
lto = "thin"       # Good balance of compile time vs binary size
strip = true        # Strip debug symbols from release binary
codegen-units = 1   # Better optimization at cost of compile time
```

This produces small, fast binaries suitable for distribution.

## Sources

- clap: https://docs.rs/clap (derive API documentation) -- HIGH confidence on features/API
- notify: https://docs.rs/notify (cross-platform file watching) -- HIGH confidence on approach
- minijinja: https://docs.rs/minijinja (template engine by Armin Ronacher) -- HIGH confidence
- dialoguer: https://docs.rs/dialoguer (interactive prompts) -- HIGH confidence
- similar: https://docs.rs/similar (diffing library) -- HIGH confidence
- All version numbers: MEDIUM confidence (based on training data as of early 2025; verify with `cargo search <crate>` or crates.io before adding to Cargo.toml)

## Version Verification Note

All version numbers above are based on training data current to approximately early 2025. Before creating the project's `Cargo.toml`, run `cargo search <crate> --limit 1` for each dependency to confirm you're using the latest versions. The API surfaces and feature recommendations are HIGH confidence regardless of exact version numbers.
