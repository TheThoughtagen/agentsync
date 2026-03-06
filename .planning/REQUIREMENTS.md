# Requirements: aisync

**Defined:** 2026-03-05
**Core Value:** Every AI tool working on a project sees the same instructions, memory, and hooks — always in sync, zero manual copying.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### CLI Core

- [x] **CLI-01**: User can run `aisync init` to scaffold `.ai/` directory with interactive detection and import
- [x] **CLI-02**: User can run `aisync sync` to one-shot sync from `.ai/` to all configured tools
- [x] **CLI-03**: User can run `aisync sync --dry-run` to preview changes without applying them
- [x] **CLI-04**: User can run `aisync status` to see per-tool sync state and drift detection
- [ ] **CLI-05**: User can run `aisync watch` to start a daemon that auto-syncs on file changes
- [x] **CLI-06**: User can run `aisync diff` to compare canonical `.ai/` content vs tool-native files
- [ ] **CLI-07**: User can run `aisync check` to validate sync state in CI (exit non-zero on drift)
- [x] **CLI-08**: `aisync.toml` config file with `schema_version`, per-tool settings, sync strategy
- [x] **CLI-09**: All sync operations are idempotent — running twice produces the same result
- [ ] **CLI-10**: Shell completions generated for bash, zsh, and fish
- [x] **CLI-11**: Clear error messages with `--verbose` flag for debugging

### Instructions Sync

- [x] **INST-01**: `.ai/instructions.md` syncs to CLAUDE.md via symlink or copy
- [x] **INST-02**: `.ai/instructions.md` syncs to AGENTS.md via symlink or copy
- [x] **INST-03**: `.ai/instructions.md` generates `.cursor/rules/project.mdc` with YAML frontmatter
- [x] **INST-04**: Symlink-based sync by default on macOS/Linux, copy fallback on Windows
- [x] **INST-05**: `aisync init` imports existing CLAUDE.md/AGENTS.md/.mdc as `.ai/instructions.md`
- [x] **INST-06**: Import prompts user when multiple existing configs conflict
- [x] **INST-07**: `.gitignore` entries suggested/managed for tool-generated files
- [ ] **INST-08**: Bidirectional sync detects external edits to tool-native files and reverse-syncs to `.ai/`
- [x] **INST-09**: Conditional sections (`<!-- aisync:tool-only -->`) include/exclude content per tool
- [x] **INST-10**: Symlink targets validated in `aisync status` (detect dangling symlinks)

### Tool Adapters

- [x] **ADPT-01**: Claude Code adapter — instructions, memory symlink, hooks translation
- [x] **ADPT-02**: OpenCode adapter — AGENTS.md, memory references, hook plugin stubs
- [x] **ADPT-03**: Cursor adapter — .mdc generation with frontmatter, memory references
- [x] **ADPT-04**: Tool detection engine scans project root for AI tool config markers
- [x] **ADPT-05**: Adapter trait with detect, read, write, sync_memory, translate_hook, watch_paths

### Memory

- [x] **MEM-01**: `.ai/memory/` files synced to Claude Code auto-memory path via symlink
- [x] **MEM-02**: Memory file references injected into AGENTS.md for OpenCode
- [x] **MEM-03**: Memory file references injected into .mdc rules for Cursor
- [x] **MEM-04**: `aisync memory list` shows all memory files
- [x] **MEM-05**: `aisync memory add <topic>` creates new memory file
- [x] **MEM-06**: `aisync memory import claude` pulls Claude auto-memory updates into `.ai/memory/`
- [x] **MEM-07**: `aisync memory export` writes memory to all configured tools

### Hooks

- [x] **HOOK-01**: Canonical hook definitions in `.ai/hooks/*.toml` with tool-agnostic schema
- [x] **HOOK-02**: Hook translation to Claude Code `.claude/settings.json` format
- [x] **HOOK-03**: Hook translation to OpenCode `opencode.json` plugin stubs
- [x] **HOOK-04**: `aisync hooks list` shows all hooks and their tool translations
- [x] **HOOK-05**: `aisync hooks add <name>` creates canonical hook definition
- [x] **HOOK-06**: `aisync hooks translate <name>` previews each tool's version
- [x] **HOOK-07**: Warning surfaced for tools that don't support hooks (Cursor)

### Distribution

- [ ] **DIST-01**: Installable via `cargo install aisync`
- [ ] **DIST-02**: Homebrew tap (`brew install aisync`)
- [ ] **DIST-03**: GitHub releases with pre-built binaries for macOS/Linux/Windows
- [ ] **DIST-04**: Shell installer script (`curl | sh`)
- [ ] **DIST-05**: Cross-platform CI testing (macOS, Linux, Windows)

### Quality

- [ ] **QUAL-01**: Unit tests for each adapter's read/write/translate logic
- [ ] **QUAL-02**: Integration tests with fixture projects simulating multi-tool setups
- [ ] **QUAL-03**: Round-trip tests for instructions translation (canonical → native → canonical)

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Tool Breadth

- **TOOL-01**: Windsurf adapter — `.windsurfrules` generation
- **TOOL-02**: Codex adapter — AGENTS.md/codex.md symlink
- **TOOL-03**: `aisync add-tool <tool>` command for incremental tool adoption
- **TOOL-04**: Aider adapter (`.aider.conf.yml` conventions)
- **TOOL-05**: Continue adapter (`.continue/config.json` rules)
- **TOOL-06**: Plugin SDK for community-contributed adapters

### Advanced Features

- **ADV-01**: Interactive TUI conflict resolution for bidirectional sync
- **ADV-02**: `aisync migrate` — migrate from one tool to another
- **ADV-03**: Team sync for shared `.ai/` conventions across monorepo packages
- **ADV-04**: Memory dedup — detect and merge overlapping memory entries
- **ADV-05**: Hook marketplace — community-contributed hook definitions
- **ADV-06**: Git hooks — auto-run `aisync sync` on commit/checkout
- **ADV-07**: Global (non-project) config sync (`--global` flag)

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| MCP server config sync | Complex tool-specific JSON schemas, different product |
| IDE settings sync (themes, keybindings) | VS Code Settings Sync already solves this |
| Chat history / session sync | Proprietary formats, ephemeral by nature |
| Auth / credential sharing | Security concern — never sync secrets |
| Plugin/extension sync | Each tool has its own ecosystem |
| GUI / TUI dashboard | CLI output is sufficient for v1 |
| Auto-updating / self-update | Let package managers handle updates |
| Template / starter kit generation | Content creation, not config sync |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| CLI-01 | Phase 2 | Complete |
| CLI-02 | Phase 2 | Complete |
| CLI-03 | Phase 2 | Complete |
| CLI-04 | Phase 2 | Complete |
| CLI-05 | Phase 4 | Pending |
| CLI-06 | Phase 4 | Complete |
| CLI-07 | Phase 4 | Pending |
| CLI-08 | Phase 1 | Complete |
| CLI-09 | Phase 2 | Complete |
| CLI-10 | Phase 5 | Pending |
| CLI-11 | Phase 2 | Complete |
| INST-01 | Phase 2 | Complete |
| INST-02 | Phase 2 | Complete |
| INST-03 | Phase 2 | Complete |
| INST-04 | Phase 2 | Complete |
| INST-05 | Phase 2 | Complete |
| INST-06 | Phase 2 | Complete |
| INST-07 | Phase 2 | Complete |
| INST-08 | Phase 4 | Pending |
| INST-09 | Phase 4 | Complete |
| INST-10 | Phase 2 | Complete |
| ADPT-01 | Phase 2 | Complete |
| ADPT-02 | Phase 2 | Complete |
| ADPT-03 | Phase 2 | Complete |
| ADPT-04 | Phase 1 | Complete |
| ADPT-05 | Phase 1 | Complete |
| MEM-01 | Phase 3 | Complete |
| MEM-02 | Phase 3 | Complete |
| MEM-03 | Phase 3 | Complete |
| MEM-04 | Phase 3 | Complete |
| MEM-05 | Phase 3 | Complete |
| MEM-06 | Phase 3 | Complete |
| MEM-07 | Phase 3 | Complete |
| HOOK-01 | Phase 3 | Complete |
| HOOK-02 | Phase 3 | Complete |
| HOOK-03 | Phase 3 | Complete |
| HOOK-04 | Phase 3 | Complete |
| HOOK-05 | Phase 3 | Complete |
| HOOK-06 | Phase 3 | Complete |
| HOOK-07 | Phase 3 | Complete |
| DIST-01 | Phase 5 | Pending |
| DIST-02 | Phase 5 | Pending |
| DIST-03 | Phase 5 | Pending |
| DIST-04 | Phase 5 | Pending |
| DIST-05 | Phase 5 | Pending |
| QUAL-01 | Phase 5 | Pending |
| QUAL-02 | Phase 5 | Pending |
| QUAL-03 | Phase 5 | Pending |

**Coverage:**
- v1 requirements: 43 total
- Mapped to phases: 43
- Unmapped: 0

---
*Requirements defined: 2026-03-05*
*Last updated: 2026-03-05 after roadmap creation*
