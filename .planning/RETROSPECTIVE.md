# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v1.0 — aisync universal AI agent context synchronizer

**Shipped:** 2026-03-07
**Phases:** 5 | **Plans:** 20 | **Sessions:** ~10

### What Was Built
- Full CLI tool (init, sync, status, watch, diff, check, memory, hooks) in 8,514 lines of Rust
- 3 tool adapters (Claude Code, OpenCode, Cursor) with consistent trait-based architecture
- Bidirectional sync with file watching, conditional sections, and loop prevention
- Cross-platform CI/CD with GitHub releases, Homebrew formula, and shell installer
- 188 passing tests (174 unit + 14 integration)

### What Worked
- Phased delivery with each phase building on the last — no rework needed
- Enum-dispatch pattern (AnyAdapter) kept adapter implementations clean and extensible
- Forward sync before bidirectional: Phase 4 built on a solid Phase 2 foundation
- Gap closure phases (03-05, 04-04, 04-05) caught UAT failures and fixed them within the milestone
- Marker-based managed sections pattern (gitignore, AGENTS.md, .mdc) enabled idempotent updates

### What Was Inefficient
- ROADMAP.md progress table wasn't updated during execution (all phases show "Not started" despite being complete)
- ADPT-05 was prematurely marked Complete in Phase 1 when only detect()+name() existed
- Integration test config used underscore (`claude_code`) instead of hyphen (`claude-code`) — silently passed due to permissive defaults
- todo!() defaults in trait methods — safe since all adapters override, but a trap for future adapters

### Patterns Established
- Core library handles logic; CLI layer handles all interactive prompting and TTY detection
- SyncEngine/MemoryEngine/HookEngine as structs with associated functions (consistent pattern)
- Non-fatal errors for secondary features (memory sync, hook translation) — don't block primary sync
- Directory-level watching with debouncing for cross-platform editor compatibility

### Key Lessons
1. Gap closure phases are essential — UAT surfaced real issues that unit tests missed (Ctrl+C hang, conditional section filtering for symlink adapters)
2. Permissive defaults (None-means-enabled) can mask config errors — consider stricter validation in v2
3. Windows needs explicit testing for symlink-dependent features — copy fallback strategy works but has gaps (memory symlink)

### Cost Observations
- Model mix: 100% opus (balanced profile)
- Sessions: ~10
- Notable: 20 plans completed in ~48 minutes total execution time (2.4 min/plan average)

---

## Milestone: v1.1 — Adapter Expansion & Plugin SDK

**Shipped:** 2026-03-09
**Phases:** 6 | **Plans:** 13 | **Sessions:** ~6

### What Was Built
- Refactored ToolAdapter trait system — new adapters require only a single trait impl
- Windsurf and Codex adapters with detection, deduplication, and content size warnings
- `aisync add-tool` command for interactive mid-lifecycle tool adoption with partial sync
- Two-crate Plugin SDK (aisync-types + aisync-adapter) for community adapter development
- Declarative TOML adapter authoring — full adapters without writing Rust
- Compile-time registration via inventory — community crates auto-discovered at build time
- Adapter authoring documentation with working example crate

### What Worked
- Refactor-first approach (Phase 6 before Phase 7) eliminated shotgun surgery for new adapters
- Crate extraction maintained full backward compatibility via re-exports
- Three-tier deduplication (builtin > TOML > inventory) solved multi-source adapter conflicts cleanly
- Milestone audit caught 0 gaps — all 21 requirements verified across 3 independent sources

### What Was Inefficient
- ROADMAP.md progress table had column alignment issues (Phase 7-11 rows had shifted columns)
- Phase 8 add-tool `parse_tool_name()` only handles builtin names — TOML adapter names rejected in non-interactive mode
- Some phase completion markers in ROADMAP.md were inconsistent (some marked `[x]`, some not)

### Patterns Established
- Two-layer SDK pattern: TOML for declarative, Rust trait for complex adapters
- Box::leak for &'static str returns in program-lifetime adapters (acceptable trade-off)
- Arc<dyn ToolAdapter> wrapping for Plugin variant dispatch
- Re-export chains (pub use) for backward compatibility during crate extraction
- Non-fatal error handling for TOML adapter loading (eprintln, don't block sync)

### Key Lessons
1. Crate extraction is low-risk when done with re-exports — zero breaking changes to dependents
2. inventory crate works cleanly with Rust 2024 edition — compile-time registration is production-ready
3. Three-tier deduplication with HashSet is the correct pattern for multi-source plugin systems

### Cost Observations
- Model mix: 100% opus (balanced profile)
- Sessions: ~6
- Notable: 13 plans in ~59min total (4.5 min/plan average)

---

## Milestone: v1.2 — Real-World Hardening

**Shipped:** 2026-03-09
**Phases:** 5 | **Plans:** 9 | **Sessions:** ~4

### What Was Built
- Multi-file rule sync engine — `.ai/rules/*.md` with YAML frontmatter to Cursor `.mdc`, Windsurf `.md`, and concatenated content for single-file tools
- MCP server config sync — canonical `.ai/mcp.toml` generates per-tool JSON with automatic secret stripping
- Security scanner with regex-based API key detection (AWS, GitHub, Slack, generic) and non-blocking warnings
- Command sync — `.ai/commands/` slash commands to Claude Code and Cursor with stale file cleanup
- Init completeness — zero-drift after init, ghost tool filtering, correct sync messaging
- Type foundation — RuleFile, McpConfig, CommandFile types and three new adapter trait methods

### What Worked
- Dependency graph design (Phase 12 foundation → 13/14/15 parallel → 16 integration) enabled clean layering
- Consistent "engine" pattern: RuleEngine, McpEngine, CommandEngine, SecurityScanner all follow load/process/generate flow
- Shared helper functions (plan_single_file_rules_sync, plan_directory_commands_sync) eliminated adapter duplication
- `aisync-` prefix convention for managed files cleanly separates user-created from synced files
- Hand-parsed YAML frontmatter avoided pulling in serde_yml dependency for a simple key-value schema

### What Was Inefficient
- Audit was run before Phase 16 execution — showed 4 "gaps" that were simply not-yet-started work
- ROADMAP.md progress table had column shifts for Phase 12-15 rows (missing milestone column)
- Phase 16 was lighter than expected (2 bug fixes + 1 auto-sync) — could have been a single plan

### Patterns Established
- New sync dimension pattern: types in aisync-types → trait method in aisync-adapter → dispatch in adapter.rs → execution in sync.rs
- SecurityScanner with LazyLock regex compilation — zero-cost after first use, thread-safe
- Non-fatal error handling for import operations (missing/invalid files return empty config)
- WarnUnsupportedDimension as generic warning pipeline for security and transport warnings

### Key Lessons
1. Run milestone audit AFTER all phases complete, not before the last phase — stale audits create noise
2. The engine pattern (load → process → generate) scales well across sync dimensions — keep it
3. Forward-only sync for v1.x is the right call — bidirectional multi-file sync adds significant complexity for little gain at this stage

### Cost Observations
- Model mix: 100% opus (balanced profile)
- Sessions: ~4
- Notable: 9 plans in ~4 hours total (26 min/plan average), including all 5 sync dimensions

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Sessions | Phases | Key Change |
|-----------|----------|--------|------------|
| v1.0 | ~10 | 5 | Initial milestone — established phased delivery pattern |
| v1.1 | ~6 | 6 | Plugin SDK + adapter expansion — refactor-first approach validated |
| v1.2 | ~4 | 5 | Real-world hardening — dependency graph design for parallel phases |

### Cumulative Quality

| Milestone | Tests | Coverage | Tech Debt Items |
|-----------|-------|----------|-----------------|
| v1.0 | 188 | N/A | 7 (all minor) |
| v1.1 | 339 | N/A | 2 (all minor) |
| v1.2 | 339+ | N/A | 2 (carried from v1.1) |

### Top Lessons (Verified Across Milestones)

1. Gap closure phases catch real issues — always run UAT before marking a phase complete
2. Keep core library non-interactive; push all prompting to CLI layer
3. Refactor before expanding — invest in trait/interface cleanup before adding new implementations
4. Re-export chains enable zero-breaking-change crate extraction
5. The "engine" pattern (load → process → generate) scales across sync dimensions — consistent architecture
