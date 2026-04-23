# Agent Orchestrator — Phase 1 Design Spec

## Overview

A macOS desktop application for managing multiple parallel Claude Code sessions. Built with Tauri (Rust backend) and React (frontend). Each session runs Claude Code in its own PTY with `--worktree --dangerously-skip-permissions`, embedded via xterm.js for a real terminal experience. A sidebar panel provides at-a-glance status for all sessions.

Scoped to agent management and orchestration for Phase 1.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Framework | Tauri + React | Lightweight, Rust backend ideal for process management |
| Terminal | xterm.js | Real terminal emulator, no need to reparse Claude output |
| Styling | CSS Modules | Scoped styles, no runtime overhead |
| Layout | Single-window, two-pane | Simplest to build |
| Session persistence | Ephemeral (Phase 1) | Persistent sessions deferred to Phase 2 |
| Platform | macOS only | Cross-platform deferred. `portable-pty` is used as a forward-looking choice — it works on macOS now and eases future cross-platform support |
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

**Threading model:**

PTY handles from `portable-pty` are not `Send`/`Sync`, so they cannot live in a `Mutex<HashMap>` accessed from Tauri's thread pool. Instead, a **dedicated PTY manager thread** owns all PTY state and communicates with Tauri commands via channels:

```
Tauri command thread                PTY manager thread
  |                                   |
  |-- PtyRequest::Create(name,cwd) -->|  (spawns PTY)
  |<-- PtyResponse::Created(id) ------|
  |                                   |
  |-- PtyRequest::Write(id, bytes) -->|  (writes to stdin)
  |                                   |
  |           (PTY manager reads stdout in a loop,
  |            sends output via Tauri events)
```

**Core data structures:**

```rust
// Owned exclusively by the PTY manager thread (not Send/Sync)
struct Session {
    id: String,
    name: String,
    status: SessionStatus,
    pty: Box<dyn MasterPty>,   // from portable-pty
    child: Box<dyn Child>,
    cwd: PathBuf,
    created_at: Instant,
}

// Serializable metadata sent to the frontend
#[derive(Serialize, Clone)]
struct SessionInfo {
    id: String,
    name: String,
    status: SessionStatus,
    created_at: u64, // unix timestamp ms
}

#[derive(Serialize, Clone)]
enum SessionStatus {
    Starting,
    Working,
    Idle,
    NeedsAttention,
    Finished,
    Error,
}

// Channel messages between Tauri commands and PTY manager
enum PtyRequest {
    Create { name: String, cwd: PathBuf },
    Write { id: String, data: Vec<u8> },
    Resize { id: String, cols: u16, rows: u16 },
    Close { id: String },
    ListSessions,
}
```

**Key crate:** `portable-pty` for PTY management (works on macOS, forward-compatible with other platforms).

**Session storage:** `HashMap<SessionId, Session>` owned by the PTY manager thread. Tauri commands send requests via an `mpsc` channel and receive responses via a oneshot channel.

**Tauri commands (IPC):**
- `create_session(name: String, cwd: String) -> SessionId` — spawns PTY + Claude process in the given directory (must be a git repo for `--worktree`)
- `close_session(id: SessionId)` — SIGTERM, wait, SIGKILL if needed, cleanup
- `write_to_session(id: SessionId, data: Vec<u8>)` — forward keystrokes to PTY stdin
- `resize_session(id: SessionId, cols: u16, rows: u16)` — resize PTY
- `list_sessions() -> Vec<SessionInfo>` — return all session metadata (uses the serializable `SessionInfo` struct, not the internal `Session`)
- `rename_session(id: SessionId, name: String)` — update session name

**Tauri events (backend -> frontend):**
- `session-output-{id}` — raw bytes from PTY stdout
- `session-status-{id}` — status change events
- `session-exit-{id}` — process exited (with exit code)

**Status parsing logic:**

Runs on the PTY manager thread as output flows through. Since `--dangerously-skip-permissions` is used, permission prompts will not appear. "Needs Attention" focuses on Claude asking the user a question or requesting clarification.

These heuristics will require empirical tuning during Wave 4. The initial implementation should be conservative (prefer "Working" over false "Needs Attention") and log status transitions for debugging.

- **Starting**: from spawn until the first output chunk is received from Claude
- **Working**: bytes received from stdout within the last 3 seconds (reset on each output chunk)
- **Idle**: no output for 10+ seconds (longer than typical Claude thinking pauses, which are ~2-5s). Timer resets on each output chunk.
- **Needs Attention**: detected when output stops streaming AND the last output chunk ends with a line matching patterns like:
  - Lines ending with `? ` (question prompt)
  - Lines containing `(y/n)`, `(Y/N)`, `[Y/n]`, etc.
  - Lines ending with `> ` (input prompt)
  - The specific string `AskUserQuestion` or similar Claude Code markers
  - Implementation: buffer the last ~500 bytes of output per session, run pattern checks when transitioning from Working to Idle
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
  createSession: (name: string, cwd: string) => Promise<void>;
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
- Inactive terminals are kept in memory with a scrollback buffer limit of **10,000 lines** per terminal to prevent unbounded memory growth
- When switching sessions: the current terminal is detached from the DOM (not destroyed), the new session's terminal is attached. xterm.js supports this pattern — on reattach, the terminal re-renders from its buffer. Use `Terminal.open(container)` for initial attach and manage visibility via the DOM container.
- Tauri event listeners feed output into the correct terminal regardless of visibility

**Styling:**
- CSS Modules, dark theme throughout
- Terminal-aesthetic colors (dark backgrounds, muted borders, monospace where appropriate)
- Status dot colors: blue (starting), green/pulsing (working), gray (idle), orange (needs attention), muted checkmark (finished), red (error)

### Session Lifecycle

1. User clicks "+ New Session"
2. Modal prompts for a session name and a project directory (via native folder picker dialog, defaults to last used directory)
3. On confirm: frontend calls `create_session(name, cwd)` Tauri command
4. Rust backend validates `cwd` is a git repository, then spawns PTY with `claude --worktree --dangerously-skip-permissions` in that directory
5. If `cwd` is not a git repo, session immediately set to Error with a message in the terminal area
6. Session appears in sidebar with "Starting" status, auto-selects
7. Terminal attaches to DOM, output begins streaming
8. User manually types prompts/instructions into the terminal (no initial prompt — the user interacts with Claude directly)
9. User can close via right-click -> "Close" on the session card
10. On close: SIGTERM sent, PTY cleaned up, session removed from list
11. If process exits on its own: status moves to Finished/Error, session stays in list (grayed out) until dismissed

### Error Handling

- **`claude` not found on PATH**: error toast, session set to Error status immediately
- **Directory is not a git repo**: validation in `create_session` before spawning — set Error status with descriptive message
- **Worktree creation failure**: error appears in terminal output naturally (Claude handles this)
- **PTY spawn failure**: error toast + Error status
- **App quit**: Tauri shutdown hook sends SIGTERM to all active PTYs
- **Window sizing**: minimum 900x600 enforced; xterm.js resizes with window, PTY dimensions updated via `resize_session`

### Security Considerations

All sessions are launched with `--dangerously-skip-permissions`, which disables Claude Code's built-in permission checks (file writes, command execution, etc.). This is a deliberate trade-off for Phase 1:

- **Rationale**: The target user is a developer running Claude agents on their own machine against their own repos. Permission prompts in a multi-session orchestrator would defeat the purpose — the user cannot monitor 5+ sessions for permission dialogs simultaneously.
- **Mitigation**: Each session runs in an isolated git worktree, limiting blast radius. The user chooses which directory to target per session.
- **Future consideration**: Phase 2+ could add per-session permission policies or guardrails (e.g., restrict which directories/commands are allowed).

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
| **1B: Rust PTY module** | Implement the PTY manager thread, channel-based communication, PTY spawn/read/write/resize/kill using portable-pty. No Tauri IPC yet — just the core module with unit tests. | After 1A scaffold exists |
| **1C: React app shell** | TitleBar component, two-pane layout (terminal area + sidebar), CSS Modules setup, dark theme foundations | After 1A scaffold exists |

**1B and 1C can run in parallel** once 1A is done. 1A is small (mostly boilerplate) and should be done first.

### Wave 2: IPC Bridge & Terminal Integration

Connect the Rust backend to the React frontend.

| Task | Description | Can Parallelize With |
|------|-------------|---------------------|
| **2A: Tauri commands & events** | Wire up `create_session`, `close_session`, `write_to_session`, `resize_session`, `rename_session`, `list_sessions` commands. Wire up `session-output-{id}`, `session-status-{id}`, `session-exit-{id}` events. Connect Tauri commands to the PTY manager thread via channels. | 2B |
| **2B: xterm.js component shell** | Create `XTermInstance` component with xterm.js initialization, terminal attach/detach logic, resize observer, and a mock data mode for testing without a real backend. Define the TypeScript interface for Tauri events it will consume. | 2A |
| **2C: Connect frontend to backend** | Replace mock data in 2B with real Tauri event listeners and IPC calls from 2A. End-to-end test: spawn a session, see output, type input. | After 2A + 2B |

**2A and 2B can run in parallel** — 2B uses mock data until 2C wires them together.

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
Wave 2: [IPC bridge ║ xterm.js shell] ──> Connect frontend to backend
Wave 3: [Store ║ Panel UI] ──> Wire together
Wave 4: [Status parser ║ Activity UI ║ Session close] ──> Error handling & polish
```

**Total: 4 waves, 14 tasks.** Maximum parallelism is 2-3 agents working simultaneously within each wave.

---

## Deferred to Future Phases

- **Phase 2**: Persistent sessions across app restarts, split-pane multi-terminal view, token cost tracking
- **Phase 3+**: File tree sidebar, toolkit panel, watchdog sessions, model selection per session, cross-platform (Windows/Linux)
