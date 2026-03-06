# Phase 5: Polish and Distribution - Research

**Researched:** 2026-03-06
**Domain:** Rust CLI distribution, CI/CD, shell completions, test architecture
**Confidence:** HIGH

## Summary

Phase 5 covers three distinct domains: (1) distribution -- making aisync installable via `cargo install`, Homebrew, GitHub Releases, and a shell installer; (2) shell completions for bash/zsh/fish using `clap_complete`; and (3) quality -- comprehensive unit tests, integration tests, and cross-platform CI. The project already has 174 passing unit tests and a mature error hierarchy with `thiserror`. The CLI uses `clap 4.5` derive-based parsing, making shell completion generation straightforward via `clap_complete`.

For distribution, `cargo-dist` (axo.dev) is the standard tool for Rust projects. It generates GitHub Actions workflows that build cross-platform binaries, create GitHub Releases, generate shell installer scripts, and can produce Homebrew formulas -- all from a single `dist init`. This eliminates the need to hand-roll release CI, installer scripts, or Homebrew tap formulas.

**Primary recommendation:** Use `cargo-dist` for all distribution concerns (DIST-01 through DIST-05). Use `clap_complete` for shell completions (CLI-10). Write integration tests using `assert_cmd` and `assert_fs` crates against fixture directories.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CLI-10 | Shell completions generated for bash, zsh, and fish | `clap_complete` crate generates completions from existing clap derive definitions |
| DIST-01 | Installable via `cargo install aisync` | Requires Cargo.toml metadata (description, license, repository); workspace publish setup |
| DIST-02 | Homebrew tap (`brew install aisync`) | `cargo-dist` can generate Homebrew formula automatically |
| DIST-03 | GitHub releases with pre-built binaries for macOS/Linux/Windows | `cargo-dist` generates release CI with matrix builds |
| DIST-04 | Shell installer script (`curl \| sh`) | `cargo-dist` generates installer scripts automatically |
| DIST-05 | Cross-platform CI testing (macOS, Linux, Windows) | GitHub Actions matrix strategy with `ubuntu-latest`, `macos-latest`, `windows-latest` |
| QUAL-01 | Unit tests for each adapter's read/write/translate logic | Existing 174 tests; gap analysis needed per adapter |
| QUAL-02 | Integration tests with fixture projects simulating multi-tool setups | `assert_cmd` + `assert_fs` crates; existing `fixtures/` directory |
| QUAL-03 | Round-trip tests for instructions translation (canonical to native to canonical) | Test each adapter's plan_sync + read_instructions round-trip |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap_complete | 4.5 | Shell completion generation | Official companion to clap; generates bash/zsh/fish from derive definitions |
| cargo-dist | 0.31+ | Release automation | Generates CI, builds binaries, creates GitHub Releases, Homebrew formulas, shell installers |
| assert_cmd | 2.0 | CLI integration testing | Standard for testing Rust CLI binaries; runs actual binary and asserts on output/exit code |
| assert_fs | 1.1 | Filesystem test fixtures | Creates temp directories with file fixtures; integrates with assert_cmd |
| predicates | 3.1 | Test assertions | Used by assert_cmd for flexible output matching |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tempfile | 3.14 | Temp directories in tests | Already a dev-dependency; use for integration test workspaces |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| cargo-dist | Manual GitHub Actions | More control but must maintain CI, installer scripts, Homebrew formulas separately |
| assert_cmd | std::process::Command | assert_cmd provides better error messages and integrates with predicates |

**Installation (dev-dependencies):**
```bash
cargo add --dev assert_cmd assert_fs predicates
```

**Installation (build/runtime):**
```bash
cargo add clap_complete --package aisync
```

**Installation (tooling -- not a Rust dep):**
```bash
cargo install cargo-dist
```

## Architecture Patterns

### Shell Completions via Subcommand

**What:** Add a hidden `completions` subcommand that generates shell completion scripts to stdout.

**When to use:** This is the standard clap pattern -- user runs `aisync completions bash > ~/.bash_completions/aisync` or the package manager installs completions during install.

**Example:**
```rust
// In main.rs Commands enum:
/// Generate shell completions
#[command(hide = true)]
Completions {
    /// Shell to generate completions for
    shell: clap_complete::Shell,
},

// In handler:
Commands::Completions { shell } => {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "aisync", &mut std::io::stdout());
    Ok(())
}
```

### Build-time Completion Generation (Alternative)

**What:** Generate completion files during `cargo build` via `build.rs` and include them in the package.

**Example (build.rs):**
```rust
use clap::CommandFactory;
use clap_complete::{generate_to, shells};

fn main() {
    let outdir = std::env::var("OUT_DIR").unwrap();
    let mut cmd = Cli::command();
    for shell in [shells::Bash, shells::Zsh, shells::Fish] {
        generate_to(shell, &mut cmd, "aisync", &outdir).unwrap();
    }
}
```

**Recommendation:** Use the subcommand approach for simplicity. cargo-dist and Homebrew formulas can call `aisync completions <shell>` during install.

### cargo-dist Integration

**What:** Run `dist init` to configure the project for automated releases.

**How it works:**
1. `dist init` adds `[dist]` metadata to workspace `Cargo.toml` (or creates `dist-workspace.toml`)
2. Generates `.github/workflows/release.yml` with matrix builds
3. On tag push (e.g., `v0.1.0`), CI builds binaries for all targets, creates GitHub Release, uploads assets
4. Optionally generates Homebrew formula and shell installer

**Targets to configure:**
- `x86_64-apple-darwin` (macOS Intel)
- `aarch64-apple-darwin` (macOS Apple Silicon)
- `x86_64-unknown-linux-gnu` (Linux x86_64)
- `x86_64-pc-windows-msvc` (Windows)

### Integration Test Structure
```
tests/
  integration/
    mod.rs
    test_init.rs       # aisync init against fixture dirs
    test_sync.rs       # aisync sync against multi-tool fixtures
    test_status.rs     # aisync status output verification
    test_round_trip.rs # canonical -> native -> canonical
    test_completions.rs # completion generation smoke test
```

### Anti-Patterns to Avoid
- **Hand-rolling release CI:** cargo-dist handles the full matrix; custom workflows become maintenance burden
- **Generating completions only at build time:** Users need runtime generation too for non-package-manager installs
- **Testing against real home directories:** Always use tempfile/assert_fs to create isolated test environments

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Shell completions | Custom completion scripts | `clap_complete` | Stays in sync with CLI changes automatically |
| Release CI matrix | Custom GitHub Actions workflow | `cargo-dist init` | Handles cross-compilation, signing, checksums, installer generation |
| Homebrew formula | Manual Ruby formula | `cargo-dist` Homebrew integration | Auto-updates formula on each release |
| Shell installer | Custom `install.sh` script | `cargo-dist` installer generation | Handles platform detection, checksums, PATH setup |
| CLI integration testing | Raw `std::process::Command` | `assert_cmd` + `assert_fs` | Better error messages, predicate assertions, temp dir management |

**Key insight:** Distribution is deceptively complex (cross-compilation, code signing, checksums, platform detection, formula generation). cargo-dist exists precisely because every Rust project was reimplementing the same fragile CI scripts.

## Common Pitfalls

### Pitfall 1: Workspace Publishing Order
**What goes wrong:** `cargo publish` fails because `aisync` depends on `aisync-core` which isn't published yet.
**Why it happens:** Workspace members must be published in dependency order.
**How to avoid:** Publish `aisync-core` first, then `aisync`. Or use `cargo-dist` which handles publish ordering. Set `publish = false` on `aisync-core` if it's not meant to be a standalone crate.
**Warning signs:** `cargo publish --dry-run` fails with "dependency not found on crates.io."

### Pitfall 2: Missing Cargo.toml Metadata for crates.io
**What goes wrong:** `cargo publish` rejects the crate.
**Why it happens:** crates.io requires `description`, `license` (or `license-file`), and either `repository` or `homepage`.
**How to avoid:** Add all required fields to both crate Cargo.toml files before attempting publish.
**Required fields:** `description`, `license`, `repository`, `readme` (recommended), `keywords`, `categories`.

### Pitfall 3: Windows Path Separator Issues
**What goes wrong:** Tests using hardcoded `/` paths fail on Windows.
**Why it happens:** Windows uses `\` as path separator.
**How to avoid:** Use `std::path::PathBuf` and `Path::join()` everywhere. Never construct paths with string formatting. Use `assert_fs` which handles this.
**Warning signs:** Tests pass on macOS/Linux but fail on Windows CI.

### Pitfall 4: Symlink Tests on Windows
**What goes wrong:** Symlink-based sync tests fail on Windows CI.
**Why it happens:** Windows requires elevated privileges or Developer Mode for symlinks.
**How to avoid:** Gate symlink tests with `#[cfg(unix)]` or use copy-based sync for Windows tests. The crate already has copy fallback logic (INST-04).
**Warning signs:** `Os { code: 1314, kind: Uncategorized, message: "A required privilege is not held by the client." }`

### Pitfall 5: Completion Scripts Not Matching Installed Binary Name
**What goes wrong:** Completions don't activate because the binary name in completions doesn't match the installed binary.
**Why it happens:** Passing wrong name to `clap_complete::generate()`.
**How to avoid:** Always use the crate's binary name from `Cli::command().get_name()` or hardcode `"aisync"`.

### Pitfall 6: GitHub Actions Permissions for Release
**What goes wrong:** Release workflow can't create releases or upload assets.
**Why it happens:** Default `GITHUB_TOKEN` permissions may be too restrictive.
**How to avoid:** cargo-dist generated workflows include correct `permissions: contents: write` configuration.

## Code Examples

### Shell Completion Subcommand (clap_complete)
```rust
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

#[derive(Subcommand)]
enum Commands {
    // ... existing commands ...

    /// Generate shell completions
    #[command(hide = true)]
    Completions {
        /// Target shell
        shell: Shell,
    },
}

// Handler:
Commands::Completions { shell } => {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "aisync", &mut std::io::stdout());
    Ok(())
}
```

### Integration Test with assert_cmd + assert_fs
```rust
use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

#[test]
fn test_init_creates_ai_directory() {
    let temp = assert_fs::TempDir::new().unwrap();
    // Set up a fixture with a CLAUDE.md
    temp.child("CLAUDE.md").write_str("# Instructions").unwrap();

    Command::cargo_bin("aisync")
        .unwrap()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized"));

    temp.child(".ai/instructions.md").assert(predicate::path::exists());
}

#[test]
fn test_sync_multi_tool_fixture() {
    let temp = assert_fs::TempDir::new().unwrap();
    // Create .ai/ structure
    temp.child(".ai/instructions.md").write_str("# Project").unwrap();
    temp.child("aisync.toml").write_str(r#"
        schema_version = 1
        [tools.claude_code]
        enabled = true
        [tools.opencode]
        enabled = true
    "#).unwrap();

    Command::cargo_bin("aisync")
        .unwrap()
        .arg("sync")
        .current_dir(temp.path())
        .assert()
        .success();

    // Verify outputs exist
    temp.child("CLAUDE.md").assert(predicate::path::exists());
    temp.child("AGENTS.md").assert(predicate::path::exists());
}
```

### Cargo.toml Metadata for Publishing
```toml
[package]
name = "aisync"
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
description = "Sync AI tool configurations across Claude Code, Cursor, and OpenCode"
license = "MIT"
repository = "https://github.com/<owner>/agentsync"
readme = "README.md"
keywords = ["ai", "sync", "cli", "claude", "cursor"]
categories = ["command-line-utilities", "development-tools"]
```

### CI Test Matrix (for reference -- cargo-dist generates this)
```yaml
name: CI
on: [push, pull_request]
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --workspace
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Hand-written release CI | cargo-dist | 2023+ | Single command generates full release pipeline |
| Manual Homebrew formula | cargo-dist Homebrew integration | 2024+ | Auto-generated and auto-updated formulas |
| Custom shell installer | cargo-dist installer generation | 2023+ | Platform-detecting installer with checksum verification |
| structopt for CLI | clap 4.x derive | 2022+ | Built-in completion support via clap_complete |

## Open Questions

1. **Crate name availability on crates.io**
   - What we know: The binary is named `aisync`
   - What's unclear: Whether `aisync` is available on crates.io
   - Recommendation: Check `cargo search aisync` before attempting publish; have fallback name ready

2. **aisync-core publishing strategy**
   - What we know: It's a separate workspace member used by the CLI
   - What's unclear: Should it be published as a standalone library crate or kept private?
   - Recommendation: Start with `publish = false` on `aisync-core` unless there's a use case for it as a library

3. **GitHub repository visibility**
   - What we know: cargo-dist needs a public GitHub repo for releases
   - What's unclear: Whether the repo is currently public
   - Recommendation: Verify repo is public before configuring releases

## Sources

### Primary (HIGH confidence)
- clap_complete crate docs (docs.rs/clap_complete) -- API for generate() and Shell enum
- cargo-dist quickstart (axodotdev.github.io/cargo-dist) -- init flow, release workflow generation
- Cargo Book: Publishing on crates.io (doc.rust-lang.org/cargo/reference/publishing.html) -- required metadata fields

### Secondary (MEDIUM confidence)
- [dzfrias blog on Rust cross-platform GitHub Actions](https://dzfrias.dev/blog/deploy-rust-cross-platform-github-actions/) -- matrix strategy patterns
- [ahmedjama blog on cross-platform Rust CI/CD](https://ahmedjama.com/blog/2025/12/cross-platform-rust-pipeline-github-actions/) -- CI pipeline patterns
- [taiki-e/upload-rust-binary-action](https://github.com/taiki-e/upload-rust-binary-action) -- GitHub Action for binary releases
- [Federico Terzi blog on Homebrew Rust publishing](https://federicoterzi.com/blog/how-to-publish-your-rust-project-on-homebrew/) -- manual Homebrew tap setup

### Tertiary (LOW confidence)
- cargo-dist v0.31 feature set -- verified via GitHub releases page but exact Homebrew integration details need validation during implementation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- clap_complete and cargo-dist are the established Rust ecosystem tools for these problems
- Architecture: HIGH -- patterns are well-documented and widely used in Rust CLI projects
- Pitfalls: HIGH -- Windows symlink issues and workspace publish ordering are well-known Rust problems
- Distribution (cargo-dist specifics): MEDIUM -- tool evolves quickly; exact flags may differ at implementation time

**Research date:** 2026-03-06
**Valid until:** 2026-04-06 (30 days -- stable domain)
