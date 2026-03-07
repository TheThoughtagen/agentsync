# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-03-07

### Added

- `aisync init` command with interactive tool detection and config scaffolding
- `aisync sync` command to push canonical instructions to all configured tools
- `aisync status` command showing per-tool sync state and drift detection
- `aisync watch` command with bidirectional file watching and auto-sync
- Claude Code adapter (CLAUDE.md, .claude/ directory)
- Cursor adapter (.cursor/rules/ directory)
- OpenCode adapter (AGENTS.md, .opencode/ directory)
- Symlink and copy sync strategies
- Managed sections for non-destructive syncing of shared config files
- Content hashing for drift detection
- Hook and memory sync support
- Conditional content blocks per tool
- Shell completions via `aisync completions`

[0.1.0]: https://github.com/pmannion/agentsync/releases/tag/v0.1.0
