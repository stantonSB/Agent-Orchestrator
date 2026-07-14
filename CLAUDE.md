# Agent Orchestrator

A Tauri 2 desktop app for running parallel Claude Code terminal sessions with real-time status monitoring.

## Quick Reference

```bash
npm run tauri dev       # Dev mode with hot-reload
npx vitest run          # Frontend tests
cd src-tauri && cargo test  # Backend tests
npm run tauri build     # Build .app + DMG
```

## Architecture

**Rust backend** (`src-tauri/src/`):
- `pty_manager.rs` — Dedicated OS thread owning all PTY state. Communicates via mpsc channels (portable-pty handles aren't Send/Sync). Spawns reader threads and startup timers per session.
- `status_parser.rs` — Event-driven state machine (6 states: Starting, Working, Idle, NeedsAttention, Finished, Error). Purely hook-driven, no output parsing.
- `status_server.rs` — `tiny_http` server on `127.0.0.1:0`. Endpoint: `POST /status/{ao_session_id}`. Receives hook events from Claude Code.
- `hook_installer.rs` — Auto-installs `~/.claude/agent-orchestrator-notify.sh` and the idle threshold in `~/.claude.json`; removes hooks that older versions merged into `~/.claude/settings.json`. Hooks (Notification, Stop, SubagentStart/Stop, PreToolUse) are injected per-session via `claude --settings` (see `session_hook_settings` / `derive_argv`).
- `commands.rs` — Tauri IPC commands: `create_session`, `close_session`, `write_to_session`, `resize_session`, `rename_session`, `list_sessions`, `git_pull_main`.
- `state.rs` — `AppState` holding PtyManagerHandle + StatusServer.

**React frontend** (`src/`):
- `stores/sessionStore.ts` — Zustand store. Holds sessions map, active session, toast notifications. Manages Tauri IPC calls and event listeners.
- `components/TerminalArea/` — Renders all XTermInstance components simultaneously (CSS show/hide, not mount/unmount). Buffers output before terminal mounts.
- `components/XTermInstance/` — xterm.js wrapper. Tokyo Night theme, 10k scrollback, WebLinksAddon.
- `components/SessionPanel/` — Sidebar grouping sessions by project (cwd). Collapsible ProjectGroups.
- `components/SessionCard/` — Status dot, name, timer, activity pulse, context menu.
- `hooks/useGlobalKeybindings.ts` — Cmd+T, Cmd+W, Cmd+1-9.

## Key Conventions

- **Status detection is hook-driven only.** Never parse terminal output to determine session status. The StatusTracker receives events from Claude Code's Notification/Stop hooks via the HTTP server.
- **PTY thread owns all PTY state.** Never access PTY handles from other threads. Use the mpsc channel interface in `pty_manager.rs`.
- **CSS show/hide for terminals.** Terminals stay mounted when inactive to preserve scrollback. Toggle visibility via `isActive` prop, never unmount.
- **Environment capture.** The app captures login-shell env (`$SHELL -li -c env`) on startup for macOS .app compatibility. Don't assume PATH is set.
- **Sessions are identified by UUIDs.** The `AO_SESSION_ID` env var links PTY sessions to the status server.

## File Layout

```
src/                    # React frontend
  stores/               # Zustand state management
  components/           # React components (each in own directory)
  hooks/                # Custom React hooks
  lib/                  # Tauri IPC wrappers
  types/                # TypeScript types
src-tauri/src/          # Rust backend
docs/
  installation.md       # Download, Gatekeeper, first launch
  architecture.md       # System design and component deep-dives
  development.md        # Setup, build, test, release
  how-status-works.md   # Hook protocol and state machine
  keyboard-shortcuts.md # All keyboard shortcuts
  troubleshooting.md    # Common issues and fixes
  future-phases/        # Backlog: tech-debt.md, nested-subagent-terminals.md
```

## Status Hook Protocol

The app's HTTP server accepts:

```
POST /status/{ao_session_id}
Content-Type: application/json

# Notification hook (idle_prompt, permission_prompt, elicitation_dialog):
{"session_id": "...", "notification_type": "idle_prompt"}

# Stop hook (fires immediately on task completion):
{"session_id": "...", "hook_event_name": "Stop", "cwd": "..."}
```

Response codes: 200 (transition occurred), 204 (no transition), 400 (bad request), 404 (unknown session), 405 (not POST).
