# Architecture

## Overview

Agent Orchestrator is a Tauri 2 desktop app with a Rust backend and a React frontend rendered in a WebView.

```
┌─────────────────────────────────────────────────────┐
│  Tauri WebView (React 19 + xterm.js + Zustand)      │
│  ┌──────────────────────┐ ┌───────────────────────┐ │
│  │  Terminal Area        │ │  Session Panel        │ │
│  │  (XTermInstance ×N)   │ │  (ProjectGroup ×N)    │ │
│  │  CSS show/hide        │ │  (SessionCard ×N)     │ │
│  └──────────┬───────────┘ └───────────────────────┘ │
│             │ IPC (invoke)                           │
├─────────────┼───────────────────────────────────────┤
│  Rust Backend                                        │
│  ┌──────────▼───────────┐ ┌───────────────────────┐ │
│  │  PTY Manager Thread   │ │  Status HTTP Server   │ │
│  │  (mpsc channels)      │ │  (tiny_http, :0)      │ │
│  │  portable-pty         │ │  POST /status/{id}    │ │
│  └──────────────────────┘ └───────────────────────┘ │
│  ┌──────────────────────┐ ┌───────────────────────┐ │
│  │  Hook Installer       │ │  Env Capture          │ │
│  │  (~/.claude/ files)   │ │  ($SHELL -li -c env)  │ │
│  └──────────────────────┘ └───────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

**Rust backend** (`src-tauri/src/`) handles PTY management, status tracking, hook installation, and environment capture. **React frontend** (`src/`) renders terminals, session sidebar, and manages UI state via Zustand. Communication between them happens through Tauri's IPC invoke mechanism.

## PTY Manager

**File:** `src-tauri/src/pty_manager.rs`

The PTY manager runs on a **dedicated OS thread**. This is required because `portable-pty` handles are not `Send` or `Sync` — they cannot be shared across threads.

All external code communicates with the PTY manager through an **mpsc channel**. Callers send `PtyRequest` messages and receive responses via oneshot channels. This pattern keeps all PTY state on a single thread while allowing the rest of the app to interact with it safely.

For each session, the manager:
- Spawns a PTY with the user's shell environment (see Environment Capture below)
- Creates a **reader thread** that forwards PTY output to the frontend via Tauri events
- Starts a **startup timer** (5 seconds) — if no hook event arrives in time, the session transitions from Starting → Idle

Session types: `Claude` (runs `claude` CLI) or `Terminal` (plain shell).

## Status Server

**File:** `src-tauri/src/status_server.rs`

A `tiny_http` server bound to `127.0.0.1:0` (OS-assigned port). It receives hook events from Claude Code via HTTP POST and routes them to the correct session's `StatusTracker`.

**Endpoint:** `POST /status/{ao_session_id}`

The server runs on its own thread and processes requests in an accept loop. Each request is matched to a session by the `ao_session_id` path parameter, which corresponds to the `AO_SESSION_ID` environment variable set when the PTY was created.

Response codes:
- `200` — status transition occurred
- `204` — no transition (event didn't change state)
- `400` — bad request (invalid JSON, missing fields)
- `404` — unknown session ID
- `405` — not a POST request

See [How Status Works](how-status-works.md) for the full event flow.

## Status Parser (State Machine)

**File:** `src-tauri/src/status_parser.rs`

Each session has a `StatusTracker` that implements a state machine with 6 states:

```
Starting ──────────────────────────────────────────────┐
  │ idle_prompt / stop hook / 5s timeout → Idle        │
  │ permission_prompt / elicitation_dialog → NeedsAttn  │
  │                                                     │
Idle ◄──────────────────────────────────────────────────┤
  │ user presses Enter → Working                       │
  │                                                     │
Working                                                 │
  │ idle_prompt / stop hook → Finished                 │
  │ user presses Escape → Finished                     │
  │ permission_prompt / elicitation_dialog → NeedsAttn  │
  │                                                     │
NeedsAttention                                          │
  │ idle_prompt → Finished                             │
  │ user presses Enter → Working                       │
  │                                                     │
Finished                                                │
  │ user presses Enter → Working                       │
  │                                                     │
Any state ── process exits ──→ Finished or Error       │
└──────────────────────────────────────────────────────┘
```

The state machine is **purely hook-driven** — it never parses terminal output to determine status.

## Hook Installer

**File:** `src-tauri/src/hook_installer.rs`

On startup, the app ensures Claude Code hooks are installed. Three things are set up:

1. **`~/.claude/agent-orchestrator-notify.sh`** — a bash script that forwards hook events via `curl` to the status server. It no-ops silently when the app isn't running.

2. **`~/.claude/settings.json`** — `Notification` and `Stop` hook entries are merged in, pointing to the script above.

3. **`~/.claude.json`** — `messageIdleNotifThresholdMs` is set to 500ms.

Installation is idempotent — if hooks are already installed, no changes are made.

## Frontend

**Source:** `src/`

| File/Directory | Responsibility |
|----------------|----------------|
| `stores/sessionStore.ts` | Zustand store: sessions map, active session, toast state. Manages all Tauri IPC calls and event listeners. |
| `components/TerminalArea/` | Renders all `XTermInstance` components simultaneously using CSS show/hide (not mount/unmount) to preserve scrollback. |
| `components/XTermInstance/` | xterm.js wrapper. Tokyo Night theme, 10k-line scrollback, WebLinksAddon, file path click support. |
| `components/SessionPanel/` | Sidebar grouping sessions by project (working directory). Contains `ProjectGroup` and `SessionCard`. |
| `components/SessionCard/` | Status dot, session name, duration timer, activity pulse, context menu. |
| `hooks/useGlobalKeybindings.ts` | Keyboard shortcuts: Cmd+T, Cmd+W, Cmd+1-9. |
| `lib/tauri-ipc.ts` | Typed wrappers around Tauri invoke calls. |

**Key design decision:** Terminals are never unmounted when switching sessions — they are hidden via CSS. This preserves scrollback history and terminal state. The `isActive` prop controls visibility.

## Environment Capture

**File:** `src-tauri/src/pty_manager.rs` (`shell_env()` function)

macOS `.app` bundles launched from Finder inherit a minimal environment (`PATH=/usr/bin:/bin:/usr/sbin:/sbin`). The user's shell profile variables (custom PATH entries, NODE_EXTRA_CA_CERTS, etc.) are not present.

On startup, the app runs `$SHELL -li -c env` once, parses the output into a key-value map, and caches it for the process lifetime via `OnceLock`. All PTY sessions are spawned with this captured environment. If the capture fails, the app falls back to its own (minimal) environment.
