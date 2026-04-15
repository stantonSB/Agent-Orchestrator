# Agent Orchestrator — Phase 1 Design Spec

## Overview

A macOS desktop application for managing multiple parallel Claude Code sessions. Built with Tauri (Rust backend) and React (frontend). Each session runs Claude Code in its own PTY with `--worktree --dangerously-skip-permissions`, embedded via xterm.js for a real terminal experience. A sidebar panel provides at-a-glance status for all sessions.

Inspired by [Scape](https://www.scape.work), scoped to agent management and orchestration for Phase 1.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Framework | Tauri + React | Lightweight, Rust backend ideal for process management |
| Terminal | xterm.js | Real terminal emulator, no need to reparse Claude output |
| Styling | CSS Modules | Scoped styles, no runtime overhead |
| Layout | Single-window, two-pane | Matches Scape reference, simplest to build |
| Session persistence | Ephemeral (Phase 1) | Persistent sessions deferred to Phase 2 |
| Platform | macOS only | Cross-platform deferred |
| Progress indicator | Activity heartbeat + duration | Honest signal — Claude has no completion percentage |

## Architecture

### Application Shell & Layout

Single Tauri frameless window with custom title bar.

```
+-----------------------------------------------+
| [Title Bar / Drag Region]        [_] [□] [X]  |
+-------------------------------+---------------+
|                               |  Session Panel |
|                               |  (~30% width)  |
|   Terminal Area               |                |
|   (~70% width)                |  [+ New Session]|
|                               |                |
|   Active session's xterm.js   |  SessionCard 1 |
|   terminal rendered here.     |  SessionCard 2 |
|                               |  SessionCard 3 |
|   When no sessions exist,     |  ...           |
|   shows "Create Session"      |                |
|   prompt.                     |                |
|                               |                |
+-------------------------------+---------------+
```

### Data Flow

```
User types in xterm.js
  -> Frontend sends keystrokes via Tauri IPC command
    -> Rust backend writes to PTY stdin

PTY stdout produces output
  -> Rust backend reads from PTY
    -> Emits Tauri event with output bytes
      -> Frontend feeds bytes into xterm.js
      -> Status parser inspects output to update session status
```

### Rust Backend

**Responsibilities:**
- PTY lifecycle management (spawn, read/write, resize, kill)
- Session state tracking
- Status parsing from stdout patterns
- Clean shutdown of all PTYs on app quit

**Core data structures:**

```rust
struct Session {
    id: String,
    name: String,
    status: SessionStatus,
    pty: Box<dyn MasterPty>,   // from portable-pty
    child: Box<dyn Child>,
    created_at: Instant,
}

enum SessionStatus {
    Starting,
    Working,
    Idle,
    NeedsAttention,
    Finished,
    Error,
}
```

**Key crate:** `portable-pty` for cross-platform PTY management.

**Session storage:** `HashMap<SessionId, Session>` behind a `Mutex`, accessed via Tauri commands.

**Tauri commands (IPC):**
- `create_session(name: String) -> SessionId` — spawns PTY + Claude process
- `close_session(id: SessionId)` — SIGTERM, wait, SIGKILL if needed, cleanup
- `write_to_session(id: SessionId, data: Vec<u8>)` — forward keystrokes to PTY stdin
- `resize_session(id: SessionId, cols: u16, rows: u16)` — resize PTY
- `list_sessions() -> Vec<SessionInfo>` — return all session metadata

**Tauri events (backend -> frontend):**
- `session-output-{id}` — raw bytes from PTY stdout
- `session-status-{id}` — status change events
- `session-exit-{id}` — process exited (with exit code)

**Status parsing logic:**
- Runs on the Rust side as output flows through
- **Working**: stdout actively streaming (bytes received recently)
- **Idle**: no output for 5+ seconds after a completed response
- **Needs Attention**: detect prompt patterns (e.g., permission prompts, question marks at end of output, "Do you want to proceed" patterns)
- **Starting**: from spawn until first meaningful output
- **Finished**: process exit with code 0
- **Error**: process exit with non-zero code

### React Frontend

**State management:** Zustand store (lightweight, no boilerplate).

**Core state:**

```typescript
interface SessionInfo {
  id: string;
  name: string;
  status: "starting" | "working" | "idle" | "needs_attention" | "finished" | "error";
  createdAt: number;
}

interface AppState {
  sessions: Map<string, SessionInfo>;
  activeSessionId: string | null;
  createSession: (name: string) => Promise<void>;
  closeSession: (id: string) => Promise<void>;
  setActiveSession: (id: string) => void;
}
```

**Component tree:**

```
App
├── TitleBar
├── TerminalArea
│   └── XTermInstance (one per session, only active one mounted in DOM)
├── SessionPanel
│   ├── NewSessionButton
│   └── SessionCard (per session)
└── NewSessionModal
```

**Terminal instance management:**
- Each session gets its own xterm.js `Terminal` instance on creation
- Only the active session's terminal is attached to the DOM
- Inactive terminals are kept in memory (preserving scrollback)
- Tauri event listeners feed output into the correct terminal regardless of visibility

**Styling:**
- CSS Modules, dark theme throughout
- Terminal-aesthetic colors (dark backgrounds, muted borders, monospace where appropriate)
- Status dot colors: blue (starting), green/pulsing (working), gray (idle), orange (needs attention), muted checkmark (finished), red (error)

### Session Lifecycle

1. User clicks "+ New Session"
2. Modal prompts for a session name
3. On confirm: frontend calls `create_session(name)` Tauri command
4. Rust backend spawns PTY with `claude --worktree --dangerously-skip-permissions`
5. Session appears in sidebar with "Starting" status, auto-selects
6. Terminal attaches to DOM, output begins streaming
7. Status transitions as Claude runs
8. User can close via right-click -> "Close" on the session card
9. On close: SIGTERM sent, PTY cleaned up, session removed from list
10. If process exits on its own: status moves to Finished/Error, session stays in list (grayed out) until dismissed

### Error Handling

- **`claude` not found on PATH**: error toast, session set to Error status immediately
- **Worktree creation failure**: error appears in terminal output naturally (Claude handles this)
- **PTY spawn failure**: error toast + Error status
- **App quit**: Tauri shutdown hook sends SIGTERM to all active PTYs
- **Window sizing**: minimum 900x600 enforced; xterm.js resizes with window, PTY dimensions updated via `resize_session`

### Activity Indicator

No fake progress bars. Each session card shows:
- **Animated pulse/bar** when status is Working (output actively streaming)
- **Static indicator** for all other statuses
- **Running duration** (e.g., "3m 42s") showing how long the session has been active

---

## Implementation Waves

The work is split into waves. Tasks within a wave can be done **in parallel** by separate agents/developers. Each wave must complete before the next begins (sequential dependency between waves).

### Wave 1: Project Scaffolding & Core Infrastructure

These tasks establish the foundation. They have some interdependencies but can be partially parallelized.

| Task | Description | Can Parallelize With |
|------|-------------|---------------------|
| **1A: Tauri + React scaffold** | Initialize Tauri project with React frontend, configure build for macOS, set up CSS Modules, install dependencies (xterm.js, zustand, portable-pty) | — (do first) |
| **1B: Rust PTY module** | Implement PTY spawn/read/write/resize/kill using portable-pty. No Tauri IPC yet — just the core module with unit tests. | After 1A scaffold exists |
| **1C: React app shell** | TitleBar component, two-pane layout (terminal area + sidebar), CSS Modules setup, dark theme foundations | After 1A scaffold exists |

**1B and 1C can run in parallel** once 1A is done. 1A is small (mostly boilerplate) and should be done first.

### Wave 2: IPC Bridge & Terminal Integration

Connect the Rust backend to the React frontend.

| Task | Description | Can Parallelize With |
|------|-------------|---------------------|
| **2A: Tauri commands & events** | Wire up `create_session`, `close_session`, `write_to_session`, `resize_session`, `list_sessions` commands. Wire up `session-output-{id}`, `session-status-{id}`, `session-exit-{id}` events. | — |
| **2B: xterm.js integration** | Create `XTermInstance` component. Hook it up to Tauri events for output. Send keystrokes back via IPC. Handle terminal resize. | After 2A |

**2A must come first** since 2B depends on the IPC interface existing.

### Wave 3: Session Management UI

Build out the session panel and lifecycle flows.

| Task | Description | Can Parallelize With |
|------|-------------|---------------------|
| **3A: Zustand store + session state** | Implement the app state store, session CRUD operations, active session tracking, event listeners for status/exit events | 3B |
| **3B: Session panel components** | SessionPanel, SessionCard, NewSessionButton, NewSessionModal. Static UI with props — doesn't need real data yet. | 3A |
| **3C: Wire it together** | Connect store to components, hook up "New Session" flow end-to-end (click button -> name modal -> spawn PTY -> terminal appears -> sidebar updates) | After 3A + 3B |

**3A and 3B can run in parallel.** 3C integrates them.

### Wave 4: Status Engine & Polish

| Task | Description | Can Parallelize With |
|------|-------------|---------------------|
| **4A: Status parser** | Implement stdout pattern matching in Rust for status detection (Working/Idle/Needs Attention). Tune timeouts and patterns. | 4B |
| **4B: Activity indicators & duration** | Animated pulse for Working status, duration timer on session cards, status dot colors/labels | 4A |
| **4C: Session close & cleanup** | Right-click context menu on session cards, close confirmation, SIGTERM/SIGKILL flow, dismissed finished sessions | 4A, 4B |
| **4D: Error handling & edge cases** | Claude not on PATH detection, spawn failure toasts, minimum window size, clean shutdown on app quit | 4A, 4B, 4C |

**4A, 4B, and 4C can all run in parallel.** 4D is the final polish pass.

### Wave Summary

```
Wave 1: Scaffold ──> [PTY module ║ App shell]
Wave 2: IPC bridge ──> Terminal integration
Wave 3: [Store ║ Panel UI] ──> Wire together
Wave 4: [Status parser ║ Activity UI ║ Session close] ──> Error handling & polish
```

**Total: 4 waves, 13 tasks.** Maximum parallelism is 2-3 agents working simultaneously within waves 1, 3, and 4.

---

## Deferred to Future Phases

- **Phase 2**: Persistent sessions across app restarts, split-pane multi-terminal view, token cost tracking
- **Phase 3+**: File tree sidebar, toolkit panel, watchdog sessions, model selection per session, cross-platform (Windows/Linux)
