# Phase 14: MCP Server Config & Security - Research

**Researched:** 2026-03-09
**Domain:** MCP server configuration sync, secret detection, TOML/JSON translation
**Confidence:** HIGH

## Summary

Phase 14 implements MCP server config sync: reading a canonical `.ai/mcp.toml`, translating to tool-native JSON formats (`.claude/.mcp.json` and `.cursor/mcp.json`), and stripping hardcoded secrets. The infrastructure is largely in place -- `McpConfig`/`McpServer` types exist in `aisync-types`, the `plan_mcp_sync` trait method exists with a default no-op, and the `WriteMcpConfig` sync action + executor already work. The core work is implementing `plan_mcp_sync` on Claude Code and Cursor adapters, adding MCP config loading in sync.rs, building the security scanner, adding the MCP import flow during init, and handling Windsurf/transport warnings.

Both Claude Code and Cursor use identical JSON schema (`{"mcpServers": {"name": {"command": "...", "args": [...], "env": {...}}}}`) with the top-level key `mcpServers`. Claude Code writes to `.claude/.mcp.json`; Cursor writes to `.cursor/mcp.json`. Windsurf uses global-only MCP config (not project-scoped), so it must be skipped with a warning.

**Primary recommendation:** Follow the exact pattern established in Phase 13 for rule sync -- load canonical config in `sync.rs`, pass to adapter `plan_mcp_sync`, generate tool-specific JSON. Add a standalone `SecurityScanner` module for regex-based secret detection that runs during both sync and init.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| MCP-01 | User can define MCP servers in `.ai/mcp.toml` with server name, command, args, and env references | McpConfig/McpServer types already serde-enabled for TOML; need loader in sync flow |
| MCP-02 | `aisync sync` generates `.claude/.mcp.json` from canonical MCP config | Implement `plan_mcp_sync` on ClaudeCodeAdapter; output `WriteMcpConfig` action |
| MCP-03 | `aisync sync` generates `.cursor/mcp.json` from canonical MCP config | Implement `plan_mcp_sync` on CursorAdapter; output `WriteMcpConfig` action |
| MCP-04 | MCP sync strips hardcoded env values and replaces with `${VAR}` references | Pre-process McpConfig before passing to adapters; replace values matching secret patterns with `${KEY_NAME}` |
| MCP-05 | `aisync init` imports existing tool MCP configs into `.ai/mcp.toml` (merging across tools) | Parse `.claude/.mcp.json` and `.cursor/mcp.json`, merge servers, write TOML |
| MCP-06 | Windsurf MCP is skipped with a warning (global-only config, not project-scoped) | WindsurfAdapter `plan_mcp_sync` returns `WarnUnsupportedDimension` |
| MCP-07 | MCP sync scopes to stdio transport only; warns when a server uses unsupported transport for a target tool | Detect `type: "http"` or `url:` fields; emit `WarnUnsupportedDimension` per server per tool |
| SEC-01 | Security scanner detects hardcoded API keys in MCP configs using regex patterns | New `security` module with regex patterns for AWS, GitHub, Slack, generic API keys |
| SEC-02 | Security warnings are displayed during sync and init, showing which files contain potential secrets | Scanner returns warning list; CLI layer displays them |
| SEC-03 | Security scanner warns but does not block | Scanner is advisory-only; sync/init proceeds after displaying warnings |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1.x | Serialize/deserialize McpConfig to/from TOML and JSON | Already in workspace |
| serde_json | 1.x | Generate tool-native JSON configs | Already in workspace |
| toml | 0.8.x | Parse `.ai/mcp.toml` canonical config | Already in workspace |
| regex | 1.x | Secret detection patterns | Standard Rust regex crate, likely already a transitive dep |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| lazy_static or std::sync::LazyLock | std | Compile regex patterns once | For `SecurityScanner` pattern cache |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| regex | Simple string contains/starts_with | Regex is more precise for secret patterns; contains would have false positives |
| LazyLock (std) | once_cell::sync::Lazy | LazyLock is stable in Rust 1.80+; prefer std |

**Installation:**
```bash
# regex may need explicit addition to Cargo.toml
cargo add regex --package aisync-core
```

## Architecture Patterns

### Recommended Project Structure
```
crates/aisync-core/src/
  mcp.rs           # McpEngine: load .ai/mcp.toml, sanitize env values
  security.rs      # SecurityScanner: regex-based secret detection
  sync.rs          # Add MCP loading + plan_mcp_sync calls (like rules)
  init.rs          # Add import_mcp() method (like import_rules)
  adapters/
    claude_code.rs # Implement plan_mcp_sync -> .claude/.mcp.json
    cursor.rs      # Implement plan_mcp_sync -> .cursor/mcp.json
    windsurf.rs    # plan_mcp_sync -> WarnUnsupportedDimension
    opencode.rs    # plan_mcp_sync -> WarnUnsupportedDimension (no MCP support)
    codex.rs       # plan_mcp_sync -> WarnUnsupportedDimension (no MCP support)
```

### Pattern 1: MCP Config Loading (McpEngine)
**What:** Load and parse `.ai/mcp.toml` using existing `McpConfig` type
**When to use:** During sync flow, before calling adapters

```rust
// crates/aisync-core/src/mcp.rs
pub struct McpEngine;

impl McpEngine {
    /// Load MCP config from .ai/mcp.toml. Returns empty config if file doesn't exist.
    pub fn load(project_root: &Path) -> Result<McpConfig, AisyncError> {
        let path = project_root.join(".ai/mcp.toml");
        if !path.exists() {
            return Ok(McpConfig { servers: BTreeMap::new() });
        }
        let content = std::fs::read_to_string(&path)?;
        let config: McpConfig = toml::from_str(&content)?;
        Ok(config)
    }
}
```

### Pattern 2: Tool-Native JSON Generation
**What:** Transform McpConfig into the `{"mcpServers": {...}}` JSON format
**When to use:** In each adapter's `plan_mcp_sync`

Both Claude Code and Cursor use identical top-level JSON structure:
```json
{
  "mcpServers": {
    "server-name": {
      "command": "npx",
      "args": ["-y", "package-name"],
      "env": {
        "API_KEY": "${API_KEY}"
      }
    }
  }
}
```

Key differences:
- Claude Code: `.claude/.mcp.json` (hidden directory)
- Cursor: `.cursor/mcp.json`
- Both use `mcpServers` (camelCase) as the top-level key

```rust
// Shared helper for generating tool-native MCP JSON
fn generate_mcp_json(mcp_config: &McpConfig) -> Result<String, serde_json::Error> {
    let mut servers = serde_json::Map::new();
    for (name, server) in &mcp_config.servers {
        let mut obj = serde_json::Map::new();
        obj.insert("command".into(), serde_json::Value::String(server.command.clone()));
        if !server.args.is_empty() {
            obj.insert("args".into(), serde_json::json!(server.args));
        }
        if !server.env.is_empty() {
            obj.insert("env".into(), serde_json::json!(server.env));
        }
        servers.insert(name.clone(), serde_json::Value::Object(obj));
    }
    let root = serde_json::json!({ "mcpServers": servers });
    serde_json::to_string_pretty(&root)
}
```

### Pattern 3: Secret Sanitization
**What:** Replace hardcoded secret values with `${ENV_VAR_NAME}` references
**When to use:** Before writing output JSON, applied to McpConfig env values

```rust
// In mcp.rs or security.rs
pub fn sanitize_env(mcp: &mut McpConfig) {
    for server in mcp.servers.values_mut() {
        for (key, value) in server.env.iter_mut() {
            if SecurityScanner::looks_like_secret(value) {
                *value = format!("${{{}}}", key);
            }
        }
    }
}
```

### Pattern 4: Security Scanner
**What:** Regex-based detection of hardcoded API keys
**When to use:** During sync (scan output) and init (scan imported configs)

```rust
// crates/aisync-core/src/security.rs
use std::sync::LazyLock;
use regex::Regex;

pub struct SecurityScanner;

pub struct SecurityWarning {
    pub file: String,
    pub key: String,
    pub pattern: String,  // Which pattern matched (e.g., "AWS Access Key")
}

static SECRET_PATTERNS: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| vec![
    ("AWS Access Key", Regex::new(r"AKIA[0-9A-Z]{16}").unwrap()),
    ("AWS Secret Key", Regex::new(r"[0-9a-zA-Z/+]{40}").unwrap()),  // be careful with false positives
    ("GitHub Token", Regex::new(r"gh[ps]_[A-Za-z0-9_]{36,}").unwrap()),
    ("GitHub Fine-grained", Regex::new(r"github_pat_[A-Za-z0-9_]{22,}").unwrap()),
    ("Slack Token", Regex::new(r"xox[bpors]-[0-9A-Za-z-]+").unwrap()),
    ("Anthropic API Key", Regex::new(r"sk-ant-api\d+-[A-Za-z0-9_-]+").unwrap()),
    ("OpenAI API Key", Regex::new(r"sk-[A-Za-z0-9]{48,}").unwrap()),
    ("Generic API Key", Regex::new(r"(?i)(api[_-]?key|secret[_-]?key|access[_-]?token)\s*[:=]\s*['\"]?[A-Za-z0-9_\-]{20,}").unwrap()),
]);
```

### Pattern 5: MCP Import During Init
**What:** Parse existing `.claude/.mcp.json` and `.cursor/mcp.json`, merge into `.ai/mcp.toml`
**When to use:** During `aisync init`, after scaffold

```rust
pub fn import_mcp(project_root: &Path) -> Result<usize, AisyncError> {
    let mut merged = McpConfig { servers: BTreeMap::new() };

    // Import from Claude Code
    if let Ok(config) = parse_mcp_json(project_root.join(".claude/.mcp.json")) {
        for (name, server) in config.servers {
            merged.servers.entry(name).or_insert(server);
        }
    }

    // Import from Cursor
    if let Ok(config) = parse_mcp_json(project_root.join(".cursor/mcp.json")) {
        for (name, server) in config.servers {
            merged.servers.entry(name).or_insert(server);
        }
    }

    if merged.servers.is_empty() {
        return Ok(0);
    }

    // Sanitize before writing
    SecurityScanner::sanitize_env(&mut merged);

    let toml_str = toml::to_string_pretty(&merged)?;
    std::fs::write(project_root.join(".ai/mcp.toml"), toml_str)?;
    Ok(merged.servers.len())
}
```

### Pattern 6: Transport Detection
**What:** Detect non-stdio transports (HTTP, SSE) and emit warnings
**When to use:** In `plan_mcp_sync`, check each server before including it

```rust
fn is_stdio_server(server: &McpServer) -> bool {
    // Servers without a "type" or "url" field are assumed stdio
    // The current McpServer struct only has command/args/env,
    // so HTTP servers would need a way to represent them.
    // Since McpServer requires `command`, HTTP-only servers
    // won't deserialize from mcp.toml (they'd need url/type fields)
    true  // If it parsed into McpServer, it has a command -> stdio
}
```

**Important insight:** The current `McpServer` type only has `command`, `args`, `env`. HTTP servers (like Linear's `"type": "http", "url": "..."`) would NOT parse into this struct. This means:
- For **sync**: Only stdio servers can be in `.ai/mcp.toml` (they need `command`)
- For **import**: We need to handle JSON entries that have `type`/`url` instead of `command` -- either skip them with a warning or extend `McpServer`

### Anti-Patterns to Avoid
- **Don't merge JSON with existing tool configs:** Always overwrite the entire MCP config file. Merging would require tracking which servers are aisync-managed vs user-added. The tool configs should be fully generated from `.ai/mcp.toml`.
- **Don't store secrets in `.ai/mcp.toml`:** The sanitize step should run during import, not just during output. The canonical file should have `${VAR}` references, never actual keys.
- **Don't block on security warnings:** SEC-03 explicitly says warn-but-don't-block. Return warnings as data, display in CLI layer.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Secret pattern matching | Custom string scanning | `regex` crate with compiled patterns | Regex handles complex patterns (AKIA prefix, variable-length tokens); hand-rolled would miss edge cases |
| JSON pretty-printing | Manual string building | `serde_json::to_string_pretty` | Handles escaping, nested structures, consistent formatting |
| TOML serialization | Manual TOML formatting | `toml::to_string_pretty` | Already used in workspace; handles table syntax correctly |

**Key insight:** The entire MCP sync pipeline is a straightforward data transformation: TOML -> internal types -> JSON. All serialization libraries are already in the workspace. The novel work is the security scanner and the import merge logic.

## Common Pitfalls

### Pitfall 1: Overwriting User MCP Entries
**What goes wrong:** User manually adds an MCP server to `.cursor/mcp.json`, aisync sync overwrites it
**Why it happens:** Full-file generation replaces everything
**How to avoid:** Document clearly that `.claude/.mcp.json` and `.cursor/mcp.json` are fully managed by aisync when MCP sync is active. All servers should be in `.ai/mcp.toml`. This is the same pattern as rule sync.
**Warning signs:** Users complaining about lost MCP servers after sync

### Pitfall 2: Secret Regex False Positives
**What goes wrong:** Generic API key patterns match legitimate non-secret values (base64 strings, hashes, UUIDs)
**Why it happens:** Patterns like `[A-Za-z0-9]{40}` are too broad
**How to avoid:** Keep patterns specific (AWS: `AKIA` prefix, GitHub: `gh[ps]_` prefix, Anthropic: `sk-ant-api` prefix). Only scan env VALUES in MCP configs, not arbitrary file content. The 40-char AWS secret key pattern should be applied only when the key name contains "secret" or "aws".
**Warning signs:** Every MCP config triggering security warnings on benign values

### Pitfall 3: Import Merge Conflicts
**What goes wrong:** Same server name in Claude Code and Cursor configs with different settings
**Why it happens:** Users configure the same MCP server differently per tool
**How to avoid:** First-seen-wins strategy (document the priority order). Display a warning when a merge conflict is detected. Priority: Claude Code > Cursor (arbitrary but consistent).
**Warning signs:** Imported config missing expected servers

### Pitfall 4: HTTP/SSE Servers During Import
**What goes wrong:** Import encounters `{"type": "http", "url": "..."}` servers that don't have a `command` field
**Why it happens:** Linear, Supabase, and other services use HTTP transport
**How to avoid:** During import, skip servers without a `command` field and emit a warning. The `McpServer` type requires `command` -- HTTP servers are out of scope for v1.2 (MCP-11 deferred to v1.3).
**Warning signs:** Import panics or silently drops servers

### Pitfall 5: `.claude/.mcp.json` vs Project Root `.mcp.json`
**What goes wrong:** Claude Code has two MCP config locations -- project-level `.mcp.json` at root and `.claude/.mcp.json` in the .claude directory
**Why it happens:** Claude Code changed its config location over versions
**How to avoid:** Write to `.claude/.mcp.json` (current standard). During import, check BOTH locations. The root `.mcp.json` is the older format.
**Warning signs:** MCP servers not appearing in Claude Code after sync

### Pitfall 6: TOML Table Array Syntax
**What goes wrong:** `toml::to_string_pretty(&McpConfig)` may not produce the expected TOML format for nested server tables
**Why it happens:** TOML serializes `BTreeMap<String, McpServer>` as `[servers.name]` tables
**How to avoid:** Verify the TOML output looks correct. The existing test `test_mcp_config_toml_roundtrip` confirms this works. Expected format:
```toml
[servers.context7]
command = "npx"
args = ["-y", "@upstash/context7-mcp@latest"]

[servers.playwright]
command = "npx"
args = ["@playwright/mcp@latest"]
```
**Warning signs:** TOML parse errors on manually edited files

## Code Examples

### Canonical .ai/mcp.toml Format
```toml
[servers.context7]
command = "npx"
args = ["-y", "@upstash/context7-mcp@latest"]

[servers.playwright]
command = "npx"
args = ["@playwright/mcp@latest"]

[servers.supabase]
command = "npx"
args = ["-y", "supabase-mcp-server"]

[servers.supabase.env]
SUPABASE_URL = "${SUPABASE_URL}"
SUPABASE_KEY = "${SUPABASE_KEY}"
```

### Generated .claude/.mcp.json / .cursor/mcp.json
```json
{
  "mcpServers": {
    "context7": {
      "command": "npx",
      "args": ["-y", "@upstash/context7-mcp@latest"]
    },
    "playwright": {
      "command": "npx",
      "args": ["@playwright/mcp@latest"]
    },
    "supabase": {
      "command": "npx",
      "args": ["-y", "supabase-mcp-server"],
      "env": {
        "SUPABASE_URL": "${SUPABASE_URL}",
        "SUPABASE_KEY": "${SUPABASE_KEY}"
      }
    }
  }
}
```

### Security Scanner Usage in Sync Flow
```rust
// In sync.rs plan_all_internal, after loading MCP config:
let mut mcp_config = McpEngine::load(project_root)?;

// Run security scan before sanitization (to detect what's there)
let warnings = SecurityScanner::scan_mcp_config(&mcp_config);
// Warnings are collected but don't block

// Sanitize: replace hardcoded values with ${VAR} references
McpEngine::sanitize_env(&mut mcp_config);

// Then pass to adapters:
if !mcp_config.servers.is_empty() {
    match adapter.plan_mcp_sync(project_root, &mcp_config) {
        Ok(mcp_actions) => actions.extend(mcp_actions),
        Err(e) => { /* warn */ }
    }
}
```

### Importing MCP JSON to McpConfig
```rust
fn parse_mcp_json(path: &Path) -> Result<McpConfig, AisyncError> {
    let content = std::fs::read_to_string(path)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;

    let mut servers = BTreeMap::new();
    if let Some(mcp_servers) = json.get("mcpServers").and_then(|v| v.as_object()) {
        for (name, server_val) in mcp_servers {
            // Skip non-stdio servers (no command field)
            let command = match server_val.get("command").and_then(|v| v.as_str()) {
                Some(cmd) => cmd.to_string(),
                None => continue, // HTTP/SSE server, skip
            };

            let args: Vec<String> = server_val.get("args")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            let env: BTreeMap<String, String> = server_val.get("env")
                .and_then(|v| v.as_object())
                .map(|obj| obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect())
                .unwrap_or_default();

            servers.insert(name.clone(), McpServer { command, args, env });
        }
    }

    Ok(McpConfig { servers })
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Root `.mcp.json` for Claude Code | `.claude/.mcp.json` (project-scoped) | 2025 | aisync should write to `.claude/.mcp.json` but check root `.mcp.json` during import |
| Claude Code `mcpServers` only | Same format, still `mcpServers` | Stable | No change needed |
| Cursor `mcpServers` format | Same format | Stable | No change needed |

**Confirmed JSON format (from real configs on disk):**
- Claude Code: `{ "mcpServers": { "name": { "command": "...", "args": [...], "env": {...} } } }` in `.claude/.mcp.json` or `.mcp.json`
- Cursor: identical format in `.cursor/mcp.json`
- Both support `"type": "http"` + `"url": "..."` for HTTP transport (Linear, etc.) -- these lack `command` field
- Windsurf: global-only MCP config (`~/.codeium/windsurf/mcp_config.json`), no project-level support

## Open Questions

1. **Should `.ai/mcp.toml` support HTTP transport servers?**
   - What we know: Current `McpServer` type requires `command`. HTTP servers use `type`+`url` instead.
   - What's unclear: Should we extend `McpServer` with optional `url`/`server_type` fields?
   - Recommendation: NO for v1.2. MCP-07 says "scopes to stdio transport only". HTTP support is deferred (MCP-11 in v1.3). During import, skip HTTP servers with a warning.

2. **Where should security warnings be surfaced?**
   - What we know: SEC-02 says "displayed during sync and init"
   - What's unclear: Should warnings be in the `SyncAction` list, returned separately, or printed by a side channel?
   - Recommendation: Add a new `SyncAction::SecurityWarning` variant (or use the existing `WarnUnsupportedHooks` pattern) so warnings flow through the standard action pipeline and the CLI layer handles display. Alternatively, return warnings as a separate `Vec<SecurityWarning>` alongside the `SyncReport`. The separate return is cleaner since security warnings aren't sync actions.

3. **Should aisync sync overwrite existing tool MCP configs entirely?**
   - What we know: Rule sync uses `aisync-` prefix to coexist with user files. MCP configs are single files per tool.
   - What's unclear: Users may have manually added servers to `.cursor/mcp.json` that aren't in `.ai/mcp.toml`.
   - Recommendation: YES, full overwrite. This is the same approach as the `project.mdc` instruction file. Document that `.ai/mcp.toml` is the single source of truth. Users should run `aisync init` first to import existing configs.

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `aisync-types/src/lib.rs` -- McpConfig/McpServer types, WriteMcpConfig action
- Codebase analysis: `aisync-adapter/src/lib.rs` -- plan_mcp_sync trait method with default no-op
- Codebase analysis: `aisync-core/src/sync.rs` -- sync orchestration pattern (rules sync as template)
- Codebase analysis: `aisync-core/src/init.rs` -- import_rules pattern (template for import_mcp)
- Real MCP configs on disk: `.claude/.mcp.json` and `.cursor/mcp.json` from multiple projects (confirmed JSON format)

### Secondary (MEDIUM confidence)
- Claude Code `.mcp.json` location: Confirmed via filesystem scan (both root `.mcp.json` and `.claude/.mcp.json` exist in real projects)
- Windsurf global-only MCP: Based on project REQUIREMENTS.md decision (MCP-06)

### Tertiary (LOW confidence)
- None -- all findings verified from codebase and real config files

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in workspace, patterns established
- Architecture: HIGH -- directly follows Phase 13 rule sync pattern, types/traits pre-built
- Pitfalls: HIGH -- verified from real MCP configs found on disk (including hardcoded secrets)
- Security scanner: MEDIUM -- regex patterns are well-known but tuning for false positives needs testing

**Research date:** 2026-03-09
**Valid until:** 2026-04-09 (stable -- MCP JSON formats are well-established)
