# Domain Pitfalls

**Domain:** AI tool context synchronization CLI (file sync, symlinks, config management)
**Project:** aisync
**Researched:** 2026-03-05
**Confidence:** MEDIUM (based on extensive domain knowledge; web verification unavailable)

---

## Critical Pitfalls

Mistakes that cause rewrites, data loss, or fundamental architecture changes.

### Pitfall 1: Bidirectional Sync Infinite Loop

**What goes wrong:** When `aisync watch` detects a change in `.ai/instructions.md`, it writes to `.cursor/rules/instructions.mdc`. The file watcher also watches `.cursor/rules/`, so it detects that write as an "external edit" and tries to reverse-sync back to `.ai/`. This triggers another forward sync. The daemon spirals into an infinite loop, burning CPU and potentially corrupting files with partial writes.

**Why it happens:** Bidirectional sync is fundamentally a distributed consensus problem. Most sync tools start with "just watch both sides" without solving the feedback loop.

**Consequences:** CPU spike, corrupted files from rapid partial writes, user trust destroyed on first experience.

**Prevention:**
- Implement a **write lock / origin tracking** system. When aisync writes a file, record its path + timestamp + hash in an in-memory set. When a file change event arrives, check if aisync itself caused it and suppress the event.
- Use a **debounce window** (200-500ms) after any write before processing new events for that path.
- Add a **generation counter** or embed a comment marker (e.g., `<!-- aisync:gen:42 -->`) in generated files to detect self-authored changes.
- Write integration tests that specifically trigger bidirectional changes and assert no loop occurs.

**Detection:** CPU spikes during watch mode. Log output showing the same file being synced repeatedly. Users reporting "fan spinning" after saving a file.

**Phase:** Must be solved in the watch/daemon phase. Do NOT ship `aisync watch` without loop prevention. The one-shot `aisync sync` command is immune since it exits immediately.

---

### Pitfall 2: Windows Symlink Permission Hell

**What goes wrong:** On Windows, creating symlinks requires either Administrator privileges or Developer Mode enabled (since Windows 10 build 14972). Most developers do not run terminals as Administrator, and many corporate machines have Developer Mode locked down by IT policy. `aisync init` fails with a cryptic OS error, and the user has no idea why.

**Why it happens:** Windows treats symlinks as a security-sensitive operation (originally due to junction point attacks). Unlike macOS/Linux where symlinks are unprivileged, Windows has real restrictions.

**Consequences:** Tool is unusable on Windows out of the box. Users file issues, lose trust, and never return. "Cross-platform" claim in README is a lie.

**Prevention:**
- **Detect symlink capability at runtime** before attempting. Try creating a test symlink in a temp directory; if it fails, fall back gracefully.
- **Implement copy-with-watch fallback** as a first-class sync strategy, not an afterthought. When symlinks fail, copy files and set up a watcher for changes. Document this in `aisync.toml` as `sync_strategy = "copy"` vs `"symlink"`.
- **Surface clear error messages:** "Symlinks require Developer Mode on Windows. Enable it in Settings > Developer Settings, or aisync will use file copying instead. [Learn more: URL]"
- **Test on Windows CI** from phase 1. Do not defer Windows testing to "later."

**Detection:** CI failures on Windows runners. Users reporting "permission denied" or "A required privilege is not held by the client" errors.

**Phase:** Must be designed into the sync strategy abstraction from the very first phase. If the adapter trait assumes symlinks, refactoring to support copy fallback later requires touching every adapter.

---

### Pitfall 3: Symlink Targets Become Dangling After Git Operations

**What goes wrong:** User runs `aisync init`, which creates symlinks like `.claude/instructions.md -> ../../.ai/instructions.md`. User switches git branches. The `.ai/` directory has different contents (or doesn't exist) on the new branch. Now every symlink is dangling. Tools like Claude Code see a broken symlink and either error or silently ignore it. User switches back; symlinks may or may not recover depending on whether git preserved them.

**Why it happens:** Git does not track symlinks as symlinks by default on all platforms. Git stores the symlink target path as file content. Branch switching can leave symlinks in inconsistent states, especially if `.ai/` is gitignored.

**Consequences:** Silent data loss (tool reads empty/missing config). User confusion about why their AI tool "forgot" its instructions after switching branches. Intermittent behavior that's hard to reproduce.

**Prevention:**
- **Always validate symlink targets** in `aisync status`. Check that every managed symlink points to an existing file and report broken links prominently.
- **Design for `.ai/` being git-tracked** (not gitignored) as the default. If the canonical source is always in the repo, branch switches will update it correctly.
- **Add a git post-checkout hook option** that runs `aisync sync` automatically after branch switches.
- **Store symlink targets as relative paths** (already planned) so they survive directory moves, but test edge cases with `..` traversal across missing directories.

**Detection:** `aisync status` showing "broken" symlinks. Users reporting configs vanish after `git checkout`.

**Phase:** Core sync engine phase. The symlink creation logic must include validation, and `aisync status` must check symlink health.

---

### Pitfall 4: Race Conditions in File Watcher Event Batching

**What goes wrong:** The `notify` crate on macOS uses FSEvents, which batches events. A user saves `instructions.md` in their editor, which often does: write to temp file, delete original, rename temp to original (atomic save). This produces 3+ events in rapid succession. If aisync processes each event independently, it may try to read the file after the delete but before the rename, seeing a missing file and either erroring or syncing empty content.

**Why it happens:** Editors use atomic save strategies (write-tmp-rename) for crash safety. File system event APIs expose each intermediate step as separate events. Different OSes batch differently: FSEvents (macOS) has inherent latency; inotify (Linux) is immediate but verbose; ReadDirectoryChangesW (Windows) is somewhere in between.

**Consequences:** Intermittent sync failures. File content occasionally synced as empty. Errors that only happen with certain editors (vim's swap files, VS Code's atomic saves, JetBrains' safe-write).

**Prevention:**
- **Debounce all file events** with a 100-300ms window per path. Only process the final state after the debounce settles.
- **Verify file exists and is readable** before processing any event. If the file doesn't exist after debounce, treat it as a delete event.
- **Read file content with retry** (1-2 retries with small delay) to handle brief windows where the file is being replaced.
- **Test with multiple editors** that use different save strategies: vim (swap files), VS Code (atomic), nano (in-place), and JetBrains (safe-write with backup).

**Detection:** Logs showing "file not found" errors during watch mode. Users reporting that saves from vim/VS Code sometimes don't sync.

**Phase:** Watch/daemon phase. This is purely a file watcher concern and does not affect one-shot `aisync sync`.

---

### Pitfall 5: Tool Config Format Drift and Silent Breakage

**What goes wrong:** Cursor updates and changes its `.cursorrules` format to `.cursor/rules/*.mdc` (this actually happened). Windsurf renames `.windsurfrules` to something else. Claude Code changes where it reads `CLAUDE.md` from. The aisync adapter for that tool silently writes to the old location. The tool ignores the old location. The user thinks sync is working but the AI tool isn't reading the instructions.

**Why it happens:** AI coding tools are in rapid flux (2024-2026). Config formats change between minor versions with little notice. There's no stability contract for where these tools read their config.

**Consequences:** Sync appears to work (files exist, no errors) but the AI tool doesn't see the instructions. This is worse than an error because the user doesn't know something is wrong.

**Prevention:**
- **Version-detect each tool** where possible. Check for `.cursor/` directory structure, read version files, or check installed tool versions.
- **Validate after sync** by checking that the target paths the tool actually reads from contain the expected content. `aisync status` should verify not just that symlinks exist, but that they point to locations the tool will read.
- **Pin adapter versions** in `aisync.toml` and warn when the detected tool version is newer than the adapter was tested against.
- **Monitor upstream tools** and maintain a compatibility matrix in documentation.
- **Design adapters to be independently updatable.** Each adapter should be a self-contained module with its own format version tracking, so updating one adapter doesn't require a full release.

**Detection:** `aisync status` showing "synced" but tool behavior not reflecting instructions. User reports "sync works but Cursor ignores my rules."

**Phase:** Every phase that adds a new adapter. Each adapter needs a validation step, not just a write step. Should be enforced by the adapter trait itself (`fn validate(&self) -> Result<SyncHealth>`).

---

## Moderate Pitfalls

### Pitfall 6: TOML Config Schema Evolution Without Migration

**What goes wrong:** v0.1 ships `aisync.toml` with a certain schema. v0.3 needs new fields or restructured sections. Users upgrade aisync but their config file is the old format. The tool either crashes with an unhelpful serde error ("missing field `adapters`") or silently uses defaults that don't match the user's intent.

**Why it happens:** Config schema evolution is boring to plan for, so it gets skipped. Serde's strict deserialization in Rust means any structural change can break old configs.

**Prevention:**
- **Include a `version` field** in `aisync.toml` from day one: `schema_version = 1`.
- **Use `#[serde(default)]`** liberally so missing fields get sensible defaults rather than errors.
- **Write migration functions** that upgrade schema versions: `migrate_v1_to_v2(toml) -> toml`. Run migrations automatically on load.
- **Never remove fields** in a minor version. Deprecate first, remove in next major.
- **Test deserialization of old config versions** as part of CI.

**Detection:** Users reporting parse errors after upgrading. GitHub issues titled "aisync broke after update."

**Phase:** Phase 1 (scaffolding). The config schema must include `schema_version` from the very first release.

---

### Pitfall 7: .gitignore Conflicts and Accidental Commits of Tool-Specific Files

**What goes wrong:** `aisync init` creates symlinks like `.claude/instructions.md` and `.cursor/rules/instructions.mdc`. Some of these directories are already in `.gitignore` (users commonly gitignore `.cursor/`). Or the reverse: aisync creates files that should be gitignored but aren't, and the user accidentally commits tool-native configs that were supposed to be generated artifacts.

**Why it happens:** There's no universal convention for which AI tool config directories should be tracked vs. ignored. Different projects have different policies. Aisync touches files in directories it doesn't own.

**Consequences:** Merge conflicts when multiple developers have different tools. Committed files that should be generated. Gitignored files that should be tracked.

**Prevention:**
- **Audit `.gitignore` during `aisync init`** and present the user with a clear summary: "These managed files are currently gitignored: [list]. These are currently tracked: [list]. Recommended policy: [recommendation]."
- **Provide an `aisync gitignore` command** that generates the recommended `.gitignore` entries.
- **Document a clear convention:** `.ai/` is tracked (source of truth), tool-native files (`.cursor/rules/`, `.claude/`) are gitignored (generated artifacts). Let users override this.
- **Never modify `.gitignore` automatically** without explicit user consent.

**Detection:** Users asking "should I commit `.cursor/rules/`?" in issues. Merge conflicts in tool-native config files.

**Phase:** Init/scaffolding phase. The `.gitignore` policy must be established during `aisync init`.

---

### Pitfall 8: Notify Crate Platform Divergence

**What goes wrong:** The `notify` crate abstracts over FSEvents (macOS), inotify (Linux), and ReadDirectoryChangesW (Windows). But these backends behave very differently:
- **FSEvents** reports events with a delay (coalesced) and may report the parent directory changing instead of the specific file.
- **inotify** doesn't watch recursively by default; you must add watches for each subdirectory.
- **ReadDirectoryChangesW** has buffer size limits; if too many changes happen at once, events are dropped and you get a generic "rescan needed" event.

The developer tests on macOS, everything works. Linux CI passes because tests are fast. Windows user has a large project and events get silently dropped.

**Why it happens:** File system event APIs are fundamentally different across OSes. `notify` abstracts the API but can't abstract the semantics.

**Prevention:**
- **Use `notify` in `RecommendedWatcher` mode** (which it defaults to) but understand its limitations per platform.
- **Handle the `Rescan` event** (notify v6+) which signals that events may have been lost. When received, do a full diff-based sync.
- **Implement a periodic reconciliation** (every 30-60s during watch mode) that compares actual file state to expected state, catching any missed events.
- **Test on all three platforms in CI.** Use GitHub Actions matrix with ubuntu, macos, and windows runners.
- **Pin `notify` to a specific major version** and test upgrades carefully. The crate had significant API changes between v4, v5, and v6.

**Detection:** Watch mode working on macOS but not Linux/Windows. Events being missed under high file churn.

**Phase:** Watch/daemon phase. Must be tested cross-platform before shipping.

---

### Pitfall 9: Content Translation Fidelity Loss

**What goes wrong:** `.ai/instructions.md` is Markdown. Cursor's `.mdc` format has frontmatter and specific conventions. Windsurf's format may have different expectations. When translating, content gets mangled: markdown headings misinterpreted, code blocks broken, conditional sections stripped or duplicated, special characters escaped incorrectly.

**Why it happens:** Each tool has its own config format with its own quirks. Translation between formats is lossy. The developer tests with simple examples but real-world instructions have complex markdown, embedded code, and edge cases.

**Consequences:** Tools receive garbled instructions. Code blocks in instructions get syntax-broken. Conditional sections meant for one tool leak into another tool's config.

**Prevention:**
- **Use a proper Markdown parser** (pulldown-cmark or comrak in Rust) rather than regex-based text manipulation.
- **Define a clear intermediate representation** that instructions pass through during translation. Don't go directly from format A to format B.
- **Implement round-trip tests:** parse -> translate -> translate back -> diff. Content should survive a round trip with minimal loss.
- **Support a "passthrough" mode** where certain sections are marked as raw and copied verbatim without translation.
- **Test with real-world instructions** from actual projects, including instructions with code blocks, tables, links, and special characters.

**Detection:** Users reporting that their instructions look wrong in a specific tool. Diff between `.ai/instructions.md` and the tool-native file showing unexpected changes.

**Phase:** Adapter implementation phase. Each adapter needs translation tests with complex real-world content.

---

### Pitfall 10: Symlink Depth and Path Resolution Confusion

**What goes wrong:** Symlinks use relative paths. `.claude/instructions.md -> ../../../.ai/instructions.md` assumes a specific directory depth. If a user's project structure nests differently, or if they move the project directory, or if a tool resolves symlinks differently (some follow the chain, some read the link target), the paths break silently.

**Why it happens:** Relative symlink paths are fragile. Different tools resolve symlinks differently: some use `realpath()` (following the full chain), some use `readlink()` (reading one level), some don't follow symlinks at all.

**Consequences:** Broken config that silently produces empty/missing instructions. Hard to debug because `ls -la` shows the symlink looks correct but the target doesn't resolve from the tool's working directory.

**Prevention:**
- **Canonicalize paths at creation time.** Use `std::fs::canonicalize` to resolve the absolute path of both source and target, then compute the minimal relative path.
- **Verify the symlink resolves after creation** by reading through it and comparing content to the source.
- **Test with projects at various filesystem depths** and with directory names containing spaces, unicode, and special characters.
- **Document which tools follow symlinks** and which require actual file copies. Some tools (especially on Windows) may not follow symlinks at all.

**Detection:** `aisync status` showing symlinks as valid but tools not reading the content. Users reporting issues only on specific directory structures.

**Phase:** Core sync engine phase. Path resolution must be tested extensively before adapters build on it.

---

## Minor Pitfalls

### Pitfall 11: First-Run Experience is Confusing

**What goes wrong:** User runs `aisync init` on an existing project that already has `.claude/CLAUDE.md` and `.cursor/rules/`. The tool doesn't know whether to import existing configs into `.ai/` or overwrite them with an empty canonical structure. It either destroys existing config or ignores it, confusing the user.

**Prevention:**
- **Detect existing tool configs** before scaffolding and present an import flow: "Found existing Claude Code config. Import into .ai/? [Y/n]"
- **Never overwrite without confirmation.** Default to non-destructive behavior.
- **Create backups** of any files that will be replaced: `.claude/instructions.md.bak.20260305`.

**Phase:** Init/scaffolding phase.

---

### Pitfall 12: Lock File Stale After Crash

**What goes wrong:** `aisync watch` creates a lock file to prevent multiple daemon instances. The daemon crashes or is killed with SIGKILL. The lock file persists. Next run refuses to start because it thinks another instance is running.

**Prevention:**
- **Use PID-based lock files** that record the process ID. On startup, check if the PID in the lock file is still running.
- **Use `flock()`/advisory locks** instead of sentinel files where possible (not available on all platforms).
- **Add `aisync watch --force`** flag that removes stale locks.
- **Clean up locks on SIGTERM and SIGINT** using signal handlers (via `ctrlc` crate or `tokio::signal`).

**Phase:** Watch/daemon phase.

---

### Pitfall 13: Memory/Hook File Naming Collisions

**What goes wrong:** Two tools both want to contribute a memory file called `architecture.md`. Or a user has `.ai/memory/testing.md` and one tool generates its own `testing.md` in its memory directory. During bidirectional sync, one overwrites the other.

**Prevention:**
- **Namespace memory files by source** when reverse-syncing: `claude-architecture.md`, `cursor-architecture.md`.
- **Use content-based dedup** to detect when two files are identical and merge them.
- **Define clear ownership rules:** `.ai/memory/` files are canonical and always win in conflicts. Tool-specific additions get namespaced prefixes.

**Phase:** Bidirectional sync phase.

---

### Pitfall 14: Overengineering the Adapter Trait Too Early

**What goes wrong:** Developer designs an elaborate adapter trait with 15 methods covering every possible sync scenario before implementing a single adapter. The trait doesn't match reality once the first real adapter is built. Half the methods are unused, and critical operations the trait didn't anticipate require breaking changes.

**Prevention:**
- **Implement two adapters first** (Claude Code + OpenCode) with minimal shared abstraction.
- **Extract the trait from working code** rather than designing it upfront.
- **Start with 3-4 core trait methods:** `detect()`, `sync_to()`, `sync_from()`, `validate()`. Add methods only when a third adapter needs them.

**Phase:** Phase 1 adapter work. Resist the urge to design the "perfect" trait before writing real adapters.

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Init/Scaffolding | Overwriting existing tool configs (Pitfall 11) | Import flow with backup |
| Config schema | No version field, migration pain later (Pitfall 6) | `schema_version = 1` from day one |
| Symlink creation | Windows permissions (Pitfall 2), path resolution (Pitfall 10) | Copy fallback strategy, canonicalize paths |
| Adapter trait | Overengineered abstraction (Pitfall 14) | Extract from 2 working adapters |
| Content translation | Format fidelity loss (Pitfall 9) | Markdown parser, round-trip tests |
| Watch daemon | Infinite sync loop (Pitfall 1), event batching (Pitfall 4) | Write tracking, debounce, editor-specific tests |
| Watch daemon | Stale lock files (Pitfall 12) | PID-based locks, signal handlers |
| Watch cross-platform | Notify divergence (Pitfall 8) | CI matrix, periodic reconciliation |
| Bidirectional sync | Naming collisions (Pitfall 13), loop (Pitfall 1) | Namespacing, origin tracking |
| Git integration | Dangling symlinks on checkout (Pitfall 3), gitignore conflicts (Pitfall 7) | Validation in status, clear conventions |
| Adapter updates | Tool format drift (Pitfall 5) | Version detection, validate-after-sync |

## Sources

- Domain knowledge of file synchronization patterns (Unison, Syncthing, rsync architectural lessons)
- Rust `notify` crate behavior across platforms (known issues from crate documentation and issue tracker)
- Windows symlink restrictions (Microsoft Developer Mode documentation)
- Editor save behavior patterns (vim, VS Code, JetBrains safe-write mechanisms)
- AI tool config format history (Cursor .cursorrules -> .cursor/rules migration, Claude Code CLAUDE.md conventions)

**Note:** Web search was unavailable during this research session. Findings are based on domain expertise. Confidence is MEDIUM -- specific `notify` crate version details and current AI tool config format specifics should be verified against current documentation before implementation.
