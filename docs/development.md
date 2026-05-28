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

Pushing the tag triggers the release workflow (`.github/workflows/release.yml`), which runs three jobs:

1. **`build`** — Matrix build (aarch64 + x86_64). Uses tauri-action in build-only mode, renames DMGs to `AgentOrchestrator-v{VERSION}-{arch}.dmg`, and uploads them as GitHub Actions artifacts.
2. **`release`** — Downloads both DMG artifacts, creates the GitHub Release, and uploads both DMGs as release assets.
3. **`update-homebrew`** — Downloads the DMGs, computes SHA256 hashes, clones the [homebrew-agent-orchestrator](https://github.com/stantonSB/homebrew-agent-orchestrator) tap repo, updates the cask formula, and pushes the commit.

### Release secrets

| Secret | Purpose |
|--------|---------|
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` | Code signing |
| `APPLE_API_KEY`, `APPLE_API_ISSUER`, `APPLE_API_PRIVATE_KEY` | Notarization via App Store Connect API |
| `HOMEBREW_TAP_TOKEN` | Fine-grained PAT scoped to `homebrew-agent-orchestrator` (Contents: Read and write) for auto-updating the cask formula |

## IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Project Structure

```
src/                          # React frontend
  App.tsx                     # Root component (layout, modals, keybindings)
  main.tsx                    # Entry point
  stores/
    sessionStore.ts           # Zustand store (sessions, subagents, IPC, events, persistence)
  components/
    TerminalArea/             # Terminal rendering (CSS show/hide), image drag & drop, search
      TerminalArea.tsx        # Main terminal container
      DropOverlay.tsx         # Visual overlay during image drag
      useImageDrop.ts         # Drag & drop logic (Finder files + browser images)
    XTermInstance/             # xterm.js wrapper + addons
      XTermInstance.tsx       # Terminal component with read-only support
      useTerminal.ts          # Terminal lifecycle, addons (Search, WebLinks, Unicode)
      filePathLinkProvider.ts # Cmd+click to open file paths in VS Code
    SessionPanel/             # Sidebar with project groups
    SessionCard/              # Status dot, name, timer, rename, context menu
    ProjectGroup/             # Collapsible project section
    SubagentList/             # Subagent status dots, names, and timers
    NewSessionModal/          # Create session: name, directory, mode, pull-latest
    SettingsModal/            # Settings: default session name pattern
    SearchBar/                # In-terminal text search (Cmd+F)
    CloseConfirmDialog/       # Confirmation for closing/dismissing sessions
    QuitConfirmDialog/        # Confirmation when quitting with active sessions
    TitleBar/                 # Custom title bar with settings button
    ContextMenu/              # Right-click context menu
    Toast/                    # Individual toast notification
    ToastContainer/           # Toast notification stack
    NewSessionButton/         # Sidebar new session button
    ActivityPulse/            # Animated pulse for working sessions
    DurationTimer/            # Session elapsed time display
  hooks/
    useGlobalKeybindings.ts   # Cmd+T, Cmd+W, Cmd+1-9, Cmd+Shift+[/], Cmd+,
    useInitializeSessions.ts  # Restore persisted sessions on app start
    useSaveOnClose.ts         # Save session state + scrollback on quit
  lib/
    tauri-ipc.ts              # Typed Tauri invoke wrappers
  types/
    session.ts                # Session, SubagentStatus, SessionMode types
    tauri-events.ts           # Tauri event payload types

src-tauri/src/                # Rust backend
  main.rs                     # Tauri app entry point
  lib.rs                      # Plugin registration
  commands.rs                 # Tauri IPC command handlers (sessions, persistence, git)
  pty_manager.rs              # PTY lifecycle (dedicated thread, session modes)
  status_parser.rs            # Session status state machine
  status_server.rs            # HTTP server for hook events
  hook_installer.rs           # Auto-install Claude Code hooks
  state.rs                    # AppState (PtyManager + StatusServer + Persistence)
  persistence.rs              # Session persistence (save/load/delete, atomic writes)
  subagent_tracker.rs         # Track nested subagent sessions (start/stop/FIFO matching)
```
