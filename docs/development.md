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

## Project Structure

```
src/                          # React frontend
  App.tsx                     # Root component
  main.tsx                    # Entry point
  stores/
    sessionStore.ts           # Zustand store (sessions, IPC, events)
  components/
    TerminalArea/             # Terminal rendering (CSS show/hide)
    XTermInstance/             # xterm.js wrapper + file path links
    SessionPanel/             # Sidebar with project groups
    SessionCard/              # Session status, name, timer
    ProjectGroup/             # Collapsible project section
    NewSessionModal/          # Create session dialog
    SubagentList/             # Subagent display within sessions
    ...                       # Toast, TitleBar, ContextMenu, etc.
  hooks/
    useGlobalKeybindings.ts   # Cmd+T, Cmd+W, Cmd+1-9
    useInitializeSessions.ts  # Restore sessions on app start
  lib/
    tauri-ipc.ts              # Typed Tauri invoke wrappers
  types/
    session.ts                # Session TypeScript types
    tauri-events.ts           # Tauri event payload types

src-tauri/src/                # Rust backend
  main.rs                     # Tauri app entry point
  lib.rs                      # Plugin registration
  commands.rs                 # Tauri IPC command handlers
  pty_manager.rs              # PTY lifecycle (dedicated thread)
  status_parser.rs            # Session status state machine
  status_server.rs            # HTTP server for hook events
  hook_installer.rs           # Auto-install Claude Code hooks
  state.rs                    # AppState (PtyManager + StatusServer)
  subagent_tracker.rs         # Track nested subagent sessions
```
