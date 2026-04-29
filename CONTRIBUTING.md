# Contributing to Agent Orchestrator

Thanks for your interest in contributing! Here's how to get started.

## Development Setup

1. **Prerequisites**: Node.js 18+, Rust 1.77+, Claude Code CLI
2. Clone the repo and install dependencies:
   ```bash
   git clone https://github.com/stantonSB/Agent-Orchestrator.git
   cd Agent-Orchestrator
   npm install
   ```
3. Run in development mode:
   ```bash
   npm run tauri dev
   ```

See [Development Guide](docs/development.md) for full setup details.

## Making Changes

1. Fork the repo and create a branch from `main`
2. Make your changes
3. Run tests:
   ```bash
   npx vitest run              # Frontend tests
   cd src-tauri && cargo test   # Backend tests
   ```
4. Open a pull request against `main`

## Code Conventions

- **Rust backend** lives in `src-tauri/src/` — follow existing patterns
- **React frontend** lives in `src/` — components each get their own directory
- **Status detection is hook-driven only** — never parse terminal output
- **PTY thread owns all PTY state** — use the mpsc channel interface
- **CSS show/hide for terminals** — never unmount inactive terminals

See [CLAUDE.md](CLAUDE.md) for full architectural conventions.

## Reporting Issues

Open an issue on GitHub with:
- What you expected to happen
- What actually happened
- Steps to reproduce
- macOS version and app version

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
