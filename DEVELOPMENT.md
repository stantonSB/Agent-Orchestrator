# Development Guide

## Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Xcode Command Line Tools](https://developer.apple.com/xcode/) (macOS)

## Setup

```bash
npm install
```

## Development

```bash
# Start the app in development mode (hot-reload enabled)
npm run tauri dev
```

## Testing

```bash
# Frontend tests (Vitest + React Testing Library)
npx vitest run

# Backend tests (Rust)
cd src-tauri && cargo test
```

## Building

### Build the app + DMG installer (macOS)

```bash
npm run tauri build
```

Output files:

- **App bundle:** `src-tauri/target/release/bundle/macos/Agent Orchestrator.app`
- **DMG installer:** `src-tauri/target/release/bundle/dmg/Agent Orchestrator_<version>_aarch64.dmg`

> The architecture suffix will be `aarch64` on Apple Silicon or `x86_64` on Intel Macs.

### Frontend only

```bash
# Type-check and build the frontend
npm run build

# Preview the frontend build
npm run preview
```

## Releasing

```bash
# Bump version, tag, and push
npm run release:patch   # 0.1.1 → 0.1.2
npm run release:minor   # 0.1.1 → 0.2.0
npm run release:major   # 0.1.1 → 1.0.0
```

## IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
