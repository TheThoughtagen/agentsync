# Adapter Authoring Guide

aisync supports two ways to add tool adapters:

1. **TOML adapters** -- drop a `.toml` file into `.ai/adapters/` for simple tools that only need detection and file sync.
2. **Rust adapters** -- implement the `ToolAdapter` trait in a standalone crate for tools that need custom logic, hooks, or memory sync.

Both paths produce adapters that integrate into aisync's detection and sync pipelines alongside the builtins (Claude Code, Cursor, OpenCode, Windsurf, Codex).

---

## TOML Adapter Authoring

TOML adapters require zero Rust code. You define a `.toml` file describing how to detect the tool and where to write synced instructions.

### File placement

Place your adapter file at:

```
.ai/adapters/<tool-name>.toml
```

aisync scans this directory at startup. Files with a `.toml` extension are parsed as adapter definitions. Non-TOML files are ignored.

### Schema reference

```toml
# Required: unique identifier (must not collide with builtins)
name = "aider"

# Required: human-readable name for display output
display_name = "Aider"

# Optional: detection markers. Defaults to no markers (never detected).
[detection]
directories = [".aider"]           # directories to look for
files = [".aider.conf.yml"]        # files to look for
match_any = true                   # true = any marker sufficient, false = all required

# Required: sync configuration
[sync]
instruction_path = ".aider/rules/project.md"   # where to write synced content
strategy = "generate"                           # "symlink" | "copy" | "generate"
conditional_tags = ["aider-only"]               # optional: content filter tags
gitignore_entries = [".aider/rules/"]           # optional: entries for .gitignore
watch_paths = []                                # optional: paths to watch for reverse sync
                                                # defaults to [instruction_path]

# Optional: template for Generate strategy
[template]
content = "---\nrule: always\n---\n\n{{content}}"
frontmatter_strip = "---"          # delimiter for stripping frontmatter on read-back
```

### Field details

**`name`** (required string): Unique identifier for the adapter. Must not match any builtin name (`claude-code`, `cursor`, `opencode`, `windsurf`, `codex`). Attempting to use a builtin name results in an error and the adapter is skipped.

**`display_name`** (required string): Name shown in CLI output (e.g., `aisync status`).

**`[detection]`** (optional section): Defines how aisync determines whether this tool is configured in a project.

- `directories` -- list of directory paths relative to project root. Each is checked with `is_dir()`.
- `files` -- list of file paths relative to project root. Each is checked with `exists()`.
- `match_any` (default `true`) -- when `true`, detection succeeds if **any** marker is found. When `false`, **all** markers must be present.
- If the entire `[detection]` section is omitted, the adapter is never auto-detected. It can still be used if explicitly enabled in `aisync.toml`.

**`[sync]`** (required section): Defines how instructions are synced to this tool.

- `instruction_path` (required) -- relative path from project root where synced content is written.
- `strategy` (default `"symlink"`) -- one of:
  - `"symlink"` -- creates a symlink from `instruction_path` to `.ai/instructions.md`
  - `"copy"` -- copies canonical content directly to `instruction_path`
  - `"generate"` -- renders canonical content through a template before writing
- `conditional_tags` -- content sections wrapped in these tags are included for this tool.
- `gitignore_entries` -- paths to add to `.gitignore` when this tool is synced.
- `watch_paths` -- files to monitor for reverse sync. Defaults to `[instruction_path]` if empty.

**`[template]`** (optional section, used with `strategy = "generate"`):

- `content` -- template string. The placeholder `{{content}}` is replaced with canonical content.
- `frontmatter_strip` -- when reading instructions back, content between the first pair of this delimiter is stripped. This lets aisync round-trip content through tools that require frontmatter.

### Complete example: Aider adapter

```toml
# .ai/adapters/aider.toml
name = "aider"
display_name = "Aider"

[detection]
directories = [".aider"]
files = [".aider.conf.yml"]
match_any = true

[sync]
strategy = "generate"
instruction_path = ".aider/rules/project.md"
conditional_tags = ["aider-only"]
gitignore_entries = [".aider/rules/"]

[template]
content = "---\nrule: always\n---\n\n{{content}}"
frontmatter_strip = "---"
```

With this file in place, `aisync detect` will report Aider as present when either `.aider/` directory or `.aider.conf.yml` file exists. `aisync sync` will render canonical instructions through the template and write them to `.aider/rules/project.md`.

### Limitations of TOML adapters

TOML adapters cannot:

- Run custom detection logic (e.g., parsing version files, checking binary availability)
- Implement custom hooks or hook translation
- Perform memory sync (the `plan_memory_sync` method always returns empty)
- Implement custom sync status checks beyond hash comparison
- Provide custom `read_instructions` logic beyond optional frontmatter stripping

If your adapter needs any of these, use the Rust path.

---

## Rust Adapter Authoring

Rust adapters implement the `ToolAdapter` trait from the `aisync-adapter` crate and register via `inventory::submit!` for automatic discovery.

### When to choose Rust over TOML

- Custom detection logic (checking file contents, binary versions, environment variables)
- Hook translation to the tool's native format
- Memory file sync support
- Complex output generation beyond template interpolation
- Custom sync status checks

### Crate setup

Create a standalone Cargo project (not a workspace member):

```toml
# examples/adapter-example/Cargo.toml  (or your-adapter/Cargo.toml)
[package]
name = "aisync-adapter-example"
version = "0.1.0"
edition = "2024"
publish = false
description = "Example community adapter for aisync"

[dependencies]
aisync-adapter = { path = "../../crates/aisync-adapter" }   # or version from crates.io
inventory = "0.3"
```

Both `aisync-adapter` and `inventory` are required dependencies. The `aisync-adapter` crate re-exports `aisync_types` so you have access to `ToolKind`, `SyncStrategy`, `Confidence`, and other shared types.

### Implementing ToolAdapter

The `ToolAdapter` trait has three required methods and several optional methods with defaults:

```rust
use std::path::Path;
use aisync_adapter::{AdapterError, DetectionResult, ToolAdapter};
use aisync_adapter::aisync_types::{ToolKind, Confidence, SyncStrategy};

pub struct MyToolAdapter;

impl ToolAdapter for MyToolAdapter {
    // --- Required methods ---

    /// Unique tool identifier. Use ToolKind::Custom for community adapters.
    fn name(&self) -> ToolKind {
        ToolKind::Custom("my-tool".into())
    }

    /// Human-readable name for CLI output.
    fn display_name(&self) -> &str {
        "My Tool"
    }

    /// Relative path to the tool's native instruction file.
    fn native_instruction_path(&self) -> &str {
        ".my-tool/instructions.md"
    }

    /// Detect whether this tool is configured in the project.
    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AdapterError> {
        let config_dir = project_root.join(".my-tool");
        let detected = config_dir.is_dir();
        Ok(DetectionResult {
            tool: self.name(),
            detected,
            confidence: Confidence::High,
            markers_found: if detected { vec![config_dir] } else { vec![] },
            version_hint: None,
        })
    }

    // --- Optional methods (shown with defaults) ---

    /// Content filter tags for conditional sections.
    // fn conditional_tags(&self) -> &[&str] { &[] }

    /// Entries to add to .gitignore.
    // fn gitignore_entries(&self) -> Vec<String> { vec![] }

    /// Paths to watch for reverse sync. Defaults to [native_instruction_path()].
    // fn watch_paths(&self) -> Vec<&str> { vec![self.native_instruction_path()] }

    /// Default sync strategy. Defaults to Symlink.
    // fn default_sync_strategy(&self) -> SyncStrategy { SyncStrategy::Symlink }

    /// Read existing instructions. Defaults to None.
    // fn read_instructions(&self, project_root: &Path) -> Result<Option<String>, AdapterError>

    /// Plan sync actions. Defaults to empty.
    // fn plan_sync(&self, project_root: &Path, canonical_content: &str, strategy: SyncStrategy)
    //     -> Result<Vec<SyncAction>, AdapterError>

    /// Check sync status. Defaults to NotConfigured.
    // fn sync_status(&self, project_root: &Path, canonical_hash: &str, strategy: SyncStrategy)
    //     -> Result<ToolSyncStatus, AdapterError>

    /// Plan memory sync. Defaults to empty.
    // fn plan_memory_sync(&self, project_root: &Path, memory_files: &[PathBuf])
    //     -> Result<Vec<SyncAction>, AdapterError>

    /// Translate hooks. Defaults to Unsupported.
    // fn translate_hooks(&self, hooks: &HooksConfig) -> Result<HookTranslation, AdapterError>
}
```

### Registering with inventory

After implementing the trait, register your adapter using `inventory::submit!` with an `AdapterFactory`:

```rust
use aisync_adapter::AdapterFactory;

inventory::submit! {
    AdapterFactory {
        name: "my-tool",
        create: || Box::new(MyToolAdapter),
    }
}
```

The `name` field is used for deduplication. The `create` field is a function pointer that constructs a boxed adapter instance.

### Linking into the binary

For aisync to discover your adapter, your crate must be a dependency of the final binary. Add it to the binary's `Cargo.toml`:

```toml
# In the binary crate's Cargo.toml (e.g., crates/aisync/Cargo.toml)
[dependencies]
aisync-adapter-my-tool = { path = "../path/to/adapter" }
```

**Important:** If the linker strips your crate because no symbols from it are directly referenced, add an `extern crate` declaration in the binary's `main.rs` or `lib.rs`:

```rust
// Force the linker to include the adapter crate
extern crate aisync_adapter_my_tool;
```

This ensures `inventory::submit!` registrations are linked into the binary even though no code explicitly calls into your crate.

### Full working example

See `examples/adapter-example/` for a complete standalone adapter crate demonstrating the pattern. It implements a fictional "Aider" adapter with detection, Generate sync strategy, and inventory registration.

---

## How It Works

### Discovery order

aisync discovers adapters from three sources, in this order:

1. **Builtins** -- compiled-in adapters (Claude Code, Cursor, OpenCode, Windsurf, Codex)
2. **TOML adapters** -- loaded from `.ai/adapters/*.toml` files
3. **Inventory adapters** -- registered via `inventory::submit!(AdapterFactory {...})` in linked crates

### Name deduplication

Each adapter has a name (from `ToolAdapter::name()` or `AdapterFactory::name`). If multiple adapters share the same name, the first one seen wins:

- Builtins always take priority over TOML and inventory adapters
- TOML adapters take priority over inventory adapters
- Within the same tier, the first discovered wins

When a name collision occurs, the later adapter is silently skipped. For inventory adapters, a warning is printed to stderr.

### Configuration interaction

The `aisync.toml` configuration file controls which adapters are active:

```toml
# aisync.toml
[tools.aider]
enabled = true
sync_strategy = "generate"
```

- Adapters not listed in `[tools.*]` are enabled by default (unconfigured-is-enabled semantics)
- Setting `enabled = false` disables an adapter regardless of detection results
- The `sync_strategy` field overrides the adapter's `default_sync_strategy()` return value

---

## Troubleshooting

### My adapter is not detected

1. **Check marker files exist.** For TOML adapters, verify the directories/files listed in `[detection]` actually exist in the project root. For Rust adapters, verify your `detect()` method logic.

2. **Check `match_any` vs `match_all`.** If `match_any = false`, all listed markers must be present. A single missing marker causes detection to fail.

3. **Check the adapter file location.** TOML adapters must be in `.ai/adapters/` with a `.toml` extension. Files elsewhere or with other extensions are ignored.

4. **Check for parse errors.** Run `aisync detect` and look for `Warning: skipping adapter` messages on stderr. These indicate TOML parse failures.

### My Rust adapter is not picked up

1. **Check it is in Cargo.toml dependencies.** Your adapter crate must be a direct dependency of the binary crate. Transitive dependencies may be stripped by the linker.

2. **Add `extern crate` if needed.** If no code directly references your crate, add `extern crate your_crate_name;` in the binary's `main.rs` to force linking.

3. **Verify `inventory::submit!` is present.** Your crate must contain an `inventory::submit!` block with an `AdapterFactory`. Without it, `inventory::iter` will not find your adapter.

4. **Check the `name` field.** The `name` in `AdapterFactory` must match what your `ToolAdapter::name()` returns (the string inside `ToolKind::Custom(...)`). Mismatches cause deduplication issues.

### Name collision warning

If you see a warning about a name collision:

1. **Builtin names are reserved.** The names `claude-code`, `cursor`, `opencode`, `windsurf`, and `codex` cannot be used by TOML or Rust adapters.

2. **Rename your adapter.** Choose a unique name that does not conflict with builtins or other adapters in the project.

3. **Check for duplicate TOML files.** Two `.toml` files in `.ai/adapters/` with the same `name` field will collide. Only the first one loaded is used.
