# Agent Orchestrator

A desktop application for managing multiple AI agent terminal sessions with real-time status monitoring. Built with Tauri 2, React 19, and TypeScript.

## Installation (macOS)

Download the latest `.dmg` from the [Releases](https://github.com/stantonSB/Agent-Orchestrator/releases) page.

Since the app is not yet code-signed with an Apple Developer certificate, macOS Gatekeeper will block it on first launch. After installing, run this command once to allow it:

```bash
xattr -dr com.apple.quarantine /Applications/Agent\ Orchestrator.app
```

Then open the app normally. This only needs to be done once after downloading.

> **Why?** When macOS downloads a file via a browser, it attaches a quarantine attribute. Unsigned apps with this attribute are blocked by Gatekeeper. The command above removes the quarantine flag.

## Prerequisites (Development)

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Xcode Command Line Tools](https://developer.apple.com/xcode/) (macOS)

## Setup

```bash
# Install dependencies
npm install
```

## Development

```bash
# Start the app in development mode (hot-reload enabled)
npm run tauri dev
```

## Building

### Build the app + DMG installer (macOS)

```bash
npm run tauri build
```

Output files:

- **App bundle:** `src-tauri/target/release/bundle/macos/Agent Orchestrator.app`
- **DMG installer:** `src-tauri/target/release/bundle/dmg/Agent Orchestrator_0.1.0_aarch64.dmg`

> The architecture suffix will be `aarch64` on Apple Silicon or `x86_64` on Intel Macs.

### Frontend only

```bash
# Type-check and build the frontend
npm run build

# Preview the frontend build
npm run preview
```

## Testing

```bash
# Run frontend tests
npx vitest run
```

## Tech Stack

- **Frontend:** React 19, TypeScript, Vite, xterm.js, Zustand
- **Backend:** Rust, Tauri 2, portable-pty
- **Packaging:** Tauri bundler (DMG + .app)

## IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
