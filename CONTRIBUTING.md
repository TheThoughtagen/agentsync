# Contributing to AgentSync

Thanks for your interest in contributing! This guide will help you get started.

## Reporting Bugs & Requesting Features

Open an issue on [GitHub Issues](https://github.com/pmannion/agentsync/issues) with:

- **Bugs:** Steps to reproduce, expected vs actual behavior, OS/Rust version
- **Features:** Use case, proposed behavior, alternatives considered

## Development Setup

```sh
git clone https://github.com/pmannion/agentsync.git
cd agentsync
cargo build --workspace
cargo test --workspace
```

Requires Rust 1.85+ (edition 2024).

## Code Style

- Format with `cargo fmt --all`
- Lint with `cargo clippy --workspace -- -D warnings`
- Both are enforced in CI

## Pull Request Process

1. Fork the repo and create a feature branch from `main`
2. Make your changes with clear, focused commits
3. Ensure all checks pass: `cargo fmt`, `cargo clippy`, `cargo test`
4. Open a PR against `main` with a description of what and why
5. Address review feedback

## Commit Messages

Use conventional commit style:

```
feat: add windsurf adapter
fix: handle missing config gracefully
docs: update installation instructions
test: add sync strategy edge cases
refactor: simplify detection engine
```

## Testing

- Add tests for new functionality
- Run the full suite before submitting: `cargo test --workspace`
- Integration tests live in `crates/aisync/tests/integration/`
- Unit tests are colocated in source files

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for an overview of the codebase structure and how to add new tool adapters.

## Questions?

Open a discussion or issue -- happy to help!
