# Architecture Patterns

**Domain:** Rust CLI tool with adapter/plugin pattern for file synchronization
**Researched:** 2026-03-05
**Confidence:** MEDIUM (training data + well-established Rust ecosystem patterns; web search unavailable for verification)

## Recommended Architecture

aisync follows a **core-adapter** architecture: a central sync engine owns the canonical `.ai/` model, and tool-specific adapters implement a trait to translate that model to/from native formats. The CLI layer is a thin shell around the engine.

```
                    CLI Layer (clap)
                         |
                    Sync Engine (core)
                    /    |    \
              Config   Differ   Watcher
              Parser           (notify)
                |
          Canonical Model (.ai/)
           /    |    \     \
      Claude  OpenCode Cursor Windsurf  ...adapters
      Adapter Adapter  Adapter Adapter
```

### Workspace Layout

Use a **Cargo workspace** with library + binary separation. This is the standard Rust pattern for testable CLIs.

```
agentsync/
  Cargo.toml              # workspace root
  crates/
    aisync-core/           # library: models, traits, engine, differ
      src/
        lib.rs
        model/             # canonical data structures
          mod.rs
          instructions.rs
          memory.rs
          hooks.rs
          config.rs        # aisync.toml schema
        adapter/           # trait + implementations
          mod.rs
          trait.rs         # ToolAdapter trait
          claude.rs
          opencode.rs
          cursor.rs
          windsurf.rs
          codex.rs
        engine/            # sync orchestration
          mod.rs
          sync.rs          # one-shot sync logic
          differ.rs        # change detection / diffing
          resolver.rs      # conflict resolution
        watcher/           # file-watching subsystem
          mod.rs
          debouncer.rs
        detect.rs          # tool detection (which tools present?)
        error.rs           # unified error types
    aisync-cli/            # binary: argument parsing, UX
      src/
        main.rs
        commands/
          mod.rs
          init.rs
          sync.rs
          watch.rs
          status.rs
          add_tool.rs
          memory.rs
          hooks.rs
```

**Why workspace:** The library crate (`aisync-core`) is independently testable without CLI concerns. Integration tests create fixture `.ai/` directories and assert adapter output. The binary crate is a thin layer that parses args, calls engine, and formats output.

### Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| **CLI (aisync-cli)** | Argument parsing (clap), user interaction (dialoguer), progress display (indicatif), exit codes | Engine, Config Parser |
| **Config Parser** | Reads/writes `aisync.toml`, validates schema, provides typed config | Engine, Adapters |
| **Canonical Model** | In-memory representation of `.ai/` contents (instructions, memory entries, hooks, commands) | Engine, Adapters, Differ |
| **ToolAdapter trait** | Defines interface for reading/writing tool-native formats | Engine (via dynamic dispatch or enum dispatch) |
| **Claude Adapter** | Translates canonical model to/from `.claude/` files (CLAUDE.md, settings.json, commands/) | Canonical Model, Filesystem |
| **OpenCode Adapter** | Translates canonical model to/from `AGENTS.md`, opencode config | Canonical Model, Filesystem |
| **Cursor Adapter** | Generates `.cursor/rules/*.mdc` from canonical model | Canonical Model, Filesystem |
| **Windsurf Adapter** | Generates `.windsurfrules` from canonical model | Canonical Model, Filesystem |
| **Codex Adapter** | Generates `codex.md` / `AGENTS.md` from canonical model | Canonical Model, Filesystem |
| **Sync Engine** | Orchestrates sync: loads model, detects tools, calls adapters, handles conflicts | All adapters, Differ, Config |
| **Differ** | Compares canonical model vs tool-native state, produces change sets | Engine, Adapters |
| **Conflict Resolver** | Resolves bidirectional conflicts (canonical changed + tool-native changed) | Engine, Differ |
| **Watcher** | File system monitoring via `notify` crate, debounced event dispatch | Engine |
| **Tool Detector** | Scans project directory for tool markers (`.claude/`, `.cursor/`, etc.) | Engine, Config |

### Data Flow

#### Forward Sync (canonical -> tools)

```
1. CLI invokes Engine::sync()
2. Engine loads aisync.toml via Config Parser
3. Engine reads .ai/ directory into Canonical Model
4. Tool Detector identifies active tools (or uses config)
5. For each active tool:
   a. Adapter::read_native() loads current tool-native state
   b. Differ compares Canonical Model vs native state
   c. If changes detected:
      - Adapter::write_native(canonical_model) writes updates
      - Engine logs what changed
6. Engine returns SyncReport to CLI
7. CLI displays results
```

#### Reverse Sync (tools -> canonical)

```
1. For each active tool:
   a. Adapter::read_native() loads tool-native state
   b. Adapter::to_canonical() converts to Canonical Model
   c. Differ compares against current .ai/ contents
   d. If changes detected:
      - Conflict Resolver checks for concurrent .ai/ changes
      - If no conflict: update .ai/ files
      - If conflict: apply strategy (aisync.toml: prefer-canonical | prefer-tool | prompt)
2. Forward-sync remaining tools with updated canonical
```

#### Watch Mode

```
1. Watcher registers watches on:
   - .ai/ directory (recursive)
   - Each tool's native config paths
2. On file event:
   a. Debouncer coalesces rapid events (100-300ms window)
   b. Determine source: canonical (.ai/) or tool-native
   c. If canonical changed: forward-sync to all tools
   d. If tool-native changed: reverse-sync to canonical, then forward-sync others
   e. Guard against sync loops (track "last written by aisync" per file)
```

## Patterns to Follow

### Pattern 1: Trait-based Adapter

Use a trait, not dynamic plugins. aisync has a known, finite set of tools. Compile-time dispatch via enum or trait objects is simpler and safer than dynamic loading.

```rust
pub trait ToolAdapter: Send + Sync {
    /// Human-readable tool name
    fn name(&self) -> &str;

    /// Detect if this tool is configured in the project
    fn detect(&self, project_root: &Path) -> bool;

    /// Read tool-native config into a ToolState
    fn read_native(&self, project_root: &Path) -> Result<ToolState>;

    /// Write canonical model to tool-native format
    fn write_native(
        &self,
        project_root: &Path,
        model: &CanonicalModel,
        strategy: &SyncStrategy,
    ) -> Result<SyncResult>;

    /// Convert tool-native state to canonical model (for reverse sync)
    fn to_canonical(&self, state: &ToolState) -> Result<CanonicalModel>;

    /// Return file paths this adapter manages (for watcher registration)
    fn watched_paths(&self, project_root: &Path) -> Vec<PathBuf>;
}
```

**Why trait, not plugin system:** The tool list is known at compile time. Dynamic plugin loading (via `libloading` or similar) adds complexity for no benefit in v1. If community adapters become a thing in v2+, you can add a plugin host then. For now, enum dispatch (matching on a `Tool` enum) or `Box<dyn ToolAdapter>` is sufficient.

### Pattern 2: Canonical Model as Intermediate Representation

Never translate directly between tools (Claude -> Cursor). Always go through the canonical model. This avoids N*N translation paths.

```rust
pub struct CanonicalModel {
    pub instructions: Instructions,
    pub memory: Vec<MemoryEntry>,
    pub hooks: Vec<HookDefinition>,
    pub commands: Vec<CommandDefinition>,
    pub tool_sections: HashMap<String, String>,  // tool-specific instruction blocks
}

pub struct Instructions {
    pub content: String,
    pub sections: Vec<InstructionSection>,  // parsed conditional sections
}

pub struct MemoryEntry {
    pub name: String,
    pub content: String,
    pub tags: Vec<String>,
}

pub struct HookDefinition {
    pub name: String,
    pub trigger: HookTrigger,
    pub command: String,
    pub description: String,
}
```

### Pattern 3: SyncStrategy Configuration

Each tool's sync behavior is configurable in `aisync.toml`:

```rust
pub struct ToolConfig {
    pub enabled: bool,
    pub sync_mode: SyncMode,       // Forward | Bidirectional | Disabled
    pub file_strategy: FileStrategy, // Symlink | Copy | Template
    pub conflict_resolution: ConflictStrategy, // PreferCanonical | PreferTool | Prompt
    pub watch: bool,
}
```

### Pattern 4: Error Handling with thiserror

Use `thiserror` for library errors, `miette` or `anyhow` for CLI-facing errors with rich context.

```rust
#[derive(Debug, thiserror::Error)]
pub enum AisyncError {
    #[error("Tool '{tool}' adapter failed: {source}")]
    AdapterError { tool: String, source: Box<dyn std::error::Error + Send + Sync> },

    #[error("Config parse error in {path}: {message}")]
    ConfigError { path: PathBuf, message: String },

    #[error("Sync conflict in {file}: both canonical and {tool} changed")]
    ConflictError { file: PathBuf, tool: String },

    #[error("File system error: {0}")]
    IoError(#[from] std::io::Error),
}
```

### Pattern 5: Builder Pattern for Engine Setup

```rust
let engine = SyncEngine::builder()
    .project_root(&path)
    .config(config)
    .adapter(ClaudeAdapter::new())
    .adapter(OpenCodeAdapter::new())
    .adapter(CursorAdapter::new())
    .build()?;

let report = engine.sync(SyncDirection::Forward)?;
```

### Pattern 6: Loop Guard for Watch Mode

Prevent infinite sync loops when watcher detects changes that aisync itself wrote.

```rust
pub struct LoopGuard {
    /// Files written by aisync in the current sync cycle, with timestamps
    recently_written: HashMap<PathBuf, SystemTime>,
    /// Debounce window
    cooldown: Duration,
}

impl LoopGuard {
    pub fn should_process(&self, path: &Path, event_time: SystemTime) -> bool {
        match self.recently_written.get(path) {
            Some(write_time) => event_time.duration_since(*write_time)
                .map(|d| d > self.cooldown)
                .unwrap_or(true),
            None => true,
        }
    }
}
```

## Anti-Patterns to Avoid

### Anti-Pattern 1: Direct Tool-to-Tool Translation
**What:** Writing code that converts Claude config directly to Cursor format.
**Why bad:** Creates N*N translation paths. Adding a new tool requires N new converters.
**Instead:** Always translate through the canonical model. Claude -> Canonical -> Cursor.

### Anti-Pattern 2: Monolithic Adapter Files
**What:** Putting all adapter logic in one giant file with match arms.
**Why bad:** Difficult to test, difficult for contributors to add new tools.
**Instead:** One file per adapter, each implementing the `ToolAdapter` trait. Registration in a central `adapter/mod.rs`.

### Anti-Pattern 3: Watching Without Debouncing
**What:** Reacting to every raw `notify` event immediately.
**Why bad:** A single file save can produce 3-5 events (create temp, write, rename, modify metadata). This triggers redundant syncs and potential loops.
**Instead:** Use `notify-debouncer-full` or implement a debounce window (100-300ms) that coalesces events before triggering sync.

### Anti-Pattern 4: Symlink Assumptions on Windows
**What:** Assuming symlinks work everywhere.
**Why bad:** Windows requires developer mode or admin privileges for symlinks.
**Instead:** `FileStrategy` enum with `Symlink | Copy` fallback. Detect platform, default to copy on Windows.

### Anti-Pattern 5: Blocking I/O in Watch Mode
**What:** Running sync synchronously in the watcher callback.
**Why bad:** Blocks the watcher thread, can miss events.
**Instead:** Watcher sends events to a channel, sync runs on a separate thread/task. This naturally provides backpressure.

## Concurrency Model

For v1, use **synchronous I/O with threads** (not async). The `notify` crate is thread-based, file I/O is the bottleneck (not network), and async adds complexity without benefit here.

```
Main Thread:  CLI parsing, one-shot sync
Watch Thread: notify watcher, sends events via crossbeam channel
Sync Thread:  Receives events, runs sync engine, sends results back
```

If watch mode needs to serve a status endpoint later, consider `tokio` then. Not for v1.

## Suggested Build Order

Dependencies flow downward. Build in this order:

```
Phase 1: Foundation
  1. Canonical Model (data structures, serde for TOML)
  2. Config Parser (aisync.toml schema)
  3. ToolAdapter trait definition
  4. Error types

Phase 2: First Adapters
  5. Tool Detector (scan for .claude/, .cursor/, etc.)
  6. Claude Adapter (Tier 1 - author uses daily)
  7. OpenCode Adapter (Tier 1 - author uses daily)
  8. Forward Sync Engine (canonical -> tools, no reverse yet)

Phase 3: CLI Shell
  9.  CLI scaffolding (clap, commands structure)
  10. `aisync init` command (scaffold .ai/, import existing)
  11. `aisync sync` command (one-shot forward sync)
  12. `aisync status` command (show sync state)

Phase 4: Bidirectional + Watch
  13. Differ (change detection between canonical and native)
  14. Reverse sync (tool-native -> canonical)
  15. Conflict Resolver
  16. Watcher + debouncer
  17. Loop Guard
  18. `aisync watch` command

Phase 5: Remaining Adapters
  19. Cursor Adapter (Tier 2)
  20. Windsurf Adapter (Tier 2)
  21. Codex Adapter (Tier 2)

Phase 6: Polish
  22. Hook translation engine
  23. Memory subcommands
  24. Conditional instruction sections
  25. Distribution (Homebrew, releases)
```

**Ordering rationale:**
- Model and traits first because everything depends on the data structures. Get these right and adapters are mechanical.
- Claude + OpenCode first because the author uses them daily -- dogfooding drives quality.
- Forward sync before bidirectional because it is simpler and immediately useful. Bidirectional adds conflict resolution complexity.
- CLI shell after adapters because you need something to wire up before adding commands. But don't wait too long -- having `aisync sync` working end-to-end early validates the whole pipeline.
- Tier 2 adapters last because they don't block dogfooding and are simpler (mostly template generation, no bidirectional).
- Hook translation is late because it is the most complex translation (tool-native hook formats vary wildly).

## Scalability Considerations

| Concern | Small project (5 tools) | Large monorepo | Multi-workspace |
|---------|------------------------|----------------|-----------------|
| File watching | Trivial, < 20 paths | Watch only `.ai/` + known tool paths, not recursive project scan | One aisync instance per workspace root |
| Sync performance | Instant (< 100ms) | Same -- syncing config files, not source code | Same |
| Conflict resolution | Rare | More likely if multiple devs edit tool configs | Per-workspace resolution |
| Memory usage | Negligible | Negligible -- config files are small | Negligible |

Scalability is not a concern for this tool. Config files are small (< 100KB total), the number of tools is bounded (< 10), and sync operations are infrequent. Do not over-engineer for scale.

## Key Design Decisions

### Symlink vs Copy Default
Default to **symlink** on macOS/Linux, **copy** on Windows. Symlinks provide instant propagation (no sync needed for instruction files), but some tools may not follow symlinks correctly. Make this configurable per-tool in `aisync.toml`.

### Template Engine for Format Translation
Use **minijinja** for generating `.mdc`, `.windsurfrules`, and other templated formats. Keep templates as embedded strings in the adapter (not external template files) for single-binary distribution.

### State Tracking
Store sync metadata in `.ai/.aisync-state.json` -- file hashes, last sync timestamps, sync direction. This enables the differ to detect what changed since last sync without re-reading all tool-native files.

## Sources

- Rust `clap` crate: standard CLI argument parser, derive API for declarative command definitions
- Rust `notify` crate (v6+): cross-platform file system watcher, recommended with debouncer
- Rust `serde` + `toml` crates: canonical serialization for TOML config
- Rust `thiserror` crate: derive macro for library error types
- Rust `minijinja` crate: lightweight Jinja2-compatible template engine
- Cargo workspace pattern: standard Rust practice for multi-crate projects
- Adapter/strategy pattern: well-established GoF pattern, natural fit for Rust traits
- Note: Web search was unavailable during research; recommendations based on established Rust ecosystem patterns and the author's project requirements. Confidence is MEDIUM -- patterns are well-known but specific crate version details should be verified.
