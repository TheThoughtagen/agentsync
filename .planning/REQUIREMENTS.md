# Requirements: aisync

**Defined:** 2026-03-08
**Core Value:** Every AI tool working on a project sees the same instructions, memory, and hooks -- always in sync, zero manual copying.

## v1.1 Requirements

Requirements for Adapter Expansion & Plugin SDK milestone. Each maps to roadmap phases.

### Refactoring

- [x] **REFAC-01**: ToolAdapter trait provides all tool metadata (display name, native paths, conditional tags, gitignore entries, watch paths) -- eliminating hardcoded match arms
- [x] **REFAC-02**: ToolsConfig supports arbitrary tool names via BTreeMap with backward-compatible deserialization
- [x] **REFAC-03**: AnyAdapter enum includes Plugin variant for dynamic dispatch of SDK adapters
- [x] **REFAC-04**: Display name logic consolidated into single ToolAdapter method (6 duplications removed)

### Adapters

- [x] **ADPT-01**: Windsurf adapter generates `.windsurf/rules/project.md` with correct YAML frontmatter
- [x] **ADPT-02**: Codex adapter symlinks `AGENTS.md` to `.ai/instructions.md`
- [x] **ADPT-03**: Codex detected via `.codex/` directory, disambiguated from OpenCode
- [x] **ADPT-04**: SyncEngine deduplicates identical AGENTS.md symlink actions when both Codex and OpenCode are present
- [x] **ADPT-05**: Legacy `.windsurfrules` file detected with migration hint to modern format
- [x] **ADPT-06**: Content size limit warnings for Windsurf (12K chars) and Codex (32 KiB)

### Add Tool

- [x] **TOOL-01**: `aisync add-tool` auto-detects tools not yet configured in aisync.toml
- [x] **TOOL-02**: User interactively selects which detected tools to add
- [x] **TOOL-03**: Selected tools are added to aisync.toml and synced immediately
- [x] **TOOL-04**: Partial sync runs only for newly added tools (not full re-sync)

### Plugin SDK

- [x] **SDK-01**: `aisync-types` crate extracted with shared types (ToolKind, SyncStrategy, etc.)
- [x] **SDK-02**: `aisync-adapter` crate published with ToolAdapter trait and supporting types
- [x] **SDK-03**: Declarative TOML adapter schema supports detection rules, file mappings, sync strategy, and templates
- [x] **SDK-04**: DeclarativeAdapter struct implements ToolAdapter from parsed TOML definitions
- [x] **SDK-05**: `.ai/adapters/*.toml` files auto-discovered and loaded as plugin adapters
- [ ] **SDK-06**: Compile-time registration via `inventory` crate for community Rust adapter crates
- [ ] **SDK-07**: Documentation for community adapter authoring (both TOML and Rust paths)

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Adapters

- **ADPT-07**: Aider adapter (`.aider.conf.yml` conventions)
- **ADPT-08**: Continue adapter (`.continue/config.json` rules)

### Plugin SDK

- **SDK-08**: Dynamic plugin loading via dylib/WASM at runtime
- **SDK-09**: Runtime adapter hot-reloading during watch mode

### Codex Advanced

- **CODEX-01**: Per-subdirectory AGENTS.md sync for Codex hierarchical discovery
- **CODEX-02**: Codex config.toml sync

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Dynamic plugin loading (dylib/WASM) | Prove interface stability first; compile-time is sufficient for v1.1 |
| Aider/Continue adapters | Good first community adapter candidates via Plugin SDK |
| PearAI/Tier 3 tools | Deferred to community adapters |
| MCP server config sync | Complex tool-specific JSON schemas |
| Codex hierarchical AGENTS.md | Per-subdirectory sync adds significant complexity |
| Watch daemon auto-reload on add-tool | Require restart for v1.1; auto-reload in future |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| REFAC-01 | Phase 6 | Complete |
| REFAC-02 | Phase 6 | Complete |
| REFAC-03 | Phase 6 | Complete |
| REFAC-04 | Phase 6 | Complete |
| ADPT-01 | Phase 7 | Complete |
| ADPT-02 | Phase 7 | Complete |
| ADPT-03 | Phase 7 | Complete |
| ADPT-04 | Phase 7 | Complete |
| ADPT-05 | Phase 7 | Complete |
| ADPT-06 | Phase 7 | Complete |
| TOOL-01 | Phase 8 | Complete |
| TOOL-02 | Phase 8 | Complete |
| TOOL-03 | Phase 8 | Complete |
| TOOL-04 | Phase 8 | Complete |
| SDK-01 | Phase 9 | Complete |
| SDK-02 | Phase 9 | Complete |
| SDK-03 | Phase 10 | Complete |
| SDK-04 | Phase 10 | Complete |
| SDK-05 | Phase 10 | Complete |
| SDK-06 | Phase 11 | Pending |
| SDK-07 | Phase 11 | Pending |

**Coverage:**
- v1.1 requirements: 21 total
- Mapped to phases: 21
- Unmapped: 0

---
*Requirements defined: 2026-03-08*
*Last updated: 2026-03-08 after roadmap creation*
