# Architecture

## Workspace Layout

```
crates/
  aisync/          # CLI binary
  aisync-core/     # Core library (types, adapters, engines)
fixtures/          # Test fixture directories
```

- **aisync** depends on **aisync-core** and provides the CLI commands (`init`, `sync`, `status`, `watch`, `completions`).
- **aisync-core** contains all business logic and is tool-agnostic at its boundaries.

## Core Concepts

### Canonical Config (`.ai/`)

The `.ai/` directory is the single source of truth:

```
.ai/
  instructions.md    # Shared instructions for all tools
  commands/          # Command/task definitions
  hooks/             # Hook definitions
  hooks.toml         # Hook configuration
  memory/            # Memory/context files
```

### Adapters

Each supported tool has an adapter implementing the `ToolAdapter` trait:

```rust
pub trait ToolAdapter {
    fn name(&self) -> ToolKind;
    fn detect(&self, project_root: &Path) -> Result<DetectionResult>;
    fn read_instructions(&self, project_root: &Path) -> Result<Option<String>>;
    fn sync(&self, project_root: &Path, actions: &[SyncAction]) -> Result<ToolSyncResult>;
    fn status(&self, project_root: &Path) -> Result<ToolSyncStatus>;
    // ...
}
```

Adapters live in `crates/aisync-core/src/adapters/`:

| Adapter | File | Tool |
|---------|------|------|
| `ClaudeCodeAdapter` | `claude_code.rs` | Claude Code |
| `CursorAdapter` | `cursor.rs` | Cursor |
| `OpenCodeAdapter` | `opencode.rs` | OpenCode |

### Sync Strategies

- **Symlink**: Creates symlinks from tool config locations to `.ai/` files. Default and preferred -- changes propagate instantly.
- **Copy**: Copies content from `.ai/` to tool locations. Fallback for environments that don't support symlinks.

### Managed Sections

When aisync needs to add content to files that already have tool-specific content (like `.gitignore`), it uses delimited managed sections:

```
# aisync-managed
<aisync-controlled content>
# /aisync-managed
```

Content outside the markers is never touched.

### Detection Engine

`DetectionEngine` runs all adapters' `detect()` methods to discover which tools are configured in a project. Results include confidence levels and found markers.

### Sync Engine

`SyncEngine` orchestrates syncing:
1. Reads canonical config from `.ai/`
2. Determines sync actions per tool
3. Delegates to each adapter's `sync()` method
4. Reports results with content hashes for drift detection

### Watch Engine

`WatchEngine` uses `notify` for filesystem watching:
- Watches `.ai/` and all tool config locations
- Debounces rapid changes
- Syncs bidirectionally: tool config changes propagate back to `.ai/` and then out to all other tools

## Adding a New Tool Adapter

1. Create `crates/aisync-core/src/adapters/your_tool.rs`
2. Implement `ToolAdapter` for your struct
3. Add a variant to `ToolKind` in `types.rs`
4. Register the adapter in `adapters/mod.rs`
5. Add detection fixtures in `fixtures/`
6. Add integration tests in `crates/aisync/tests/integration/`
