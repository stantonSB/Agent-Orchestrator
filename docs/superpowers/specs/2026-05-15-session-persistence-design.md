# Session Persistence Design

Persist sessions across app restarts so users can review terminal history from previous sessions after quitting or restarting.

## Requirements

- Restore the session list in the sidebar on app restart with original name/project grouping
- Show persisted sessions with a new "exited" status (distinct from "finished")
- Preserve the last 500 lines of terminal output per session for review
- Persisted sessions are read-only (no PTY, no input)
- Close button removes persisted sessions permanently (same UX as live sessions)
- No resume command shown — users find that themselves from the scrollback

## Storage

**Location**: Tauri `app_data_dir()` → `~/Library/Application Support/com.agent-orchestrator.app/persistence/`

```
persistence/
  sessions.json          # Array of session metadata
  scrollback/
    {session-id}.txt     # Plain text, last 500 lines of terminal output
```

**`sessions.json` schema:**

```json
[
  {
    "id": "uuid-string",
    "name": "fix-auth-bug",
    "cwd": "/Users/stan/projects/my-app",
    "session_type": "claude",
    "is_git_repo": true,
    "created_at_epoch_ms": 1715000000000,
    "status_at_close": "working"
  }
]
```

Writes are atomic: write to `.tmp` file then rename. One scrollback file per session, named by session ID. All write operations to `sessions.json` are serialized through a `Mutex<()>` on `AppState` to prevent concurrent read-modify-write races.

## Save Triggers

1. **App close**: Frontend intercepts `tauri://close-requested` with `event.preventDefault()`, iterates all live sessions, serializes each xterm buffer (last 500 lines), calls `save_sessions` IPC, then programmatically closes the window via `appWindow.close()`. The backend's `on_window_event(CloseRequested)` handler is updated to skip `pty.shutdown()` if persistence save is still in progress — or more simply, the backend shutdown moves to a `RunEvent::Exit` handler which fires after the window closes, guaranteeing the frontend save completes first.

2. **Session exit** (PTY process exits naturally): Frontend saves that individual session's metadata + scrollback immediately via `save_single_session` IPC. Protects against later app crashes.

3. **Session close by user** (click X on exited session): Calls `delete_persisted_session` IPC to remove persistence files.

## Restore Flow

1. Backend reads `persistence/sessions.json` on startup
2. Frontend calls `list_persisted_sessions` IPC on app init
3. Frontend adds returned sessions to Zustand store with status `"exited"` and `persisted: true`
4. When user selects a persisted session, frontend calls `get_session_scrollback` IPC to lazy-load the scrollback text
5. Scrollback text is written into a read-only xterm instance

## Backend Changes

### New module: `persistence.rs`

Add `mod persistence;` to `lib.rs`.

Functions:
- `save_sessions(persistence_dir, sessions, scrollbacks)` — atomic write of `sessions.json` + scrollback files
- `save_single_session(persistence_dir, session, scrollback)` — read-modify-write `sessions.json` (under mutex) + write scrollback file
- `load_sessions(persistence_dir) -> Vec<PersistedSession>` — read `sessions.json`, return empty vec if missing
- `load_scrollback(persistence_dir, session_id) -> Option<String>` — read one scrollback file
- `delete_session(persistence_dir, session_id)` — remove from `sessions.json` (under mutex) + delete scrollback file

### New struct: `PersistedSession`

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct PersistedSession {
    pub id: String,
    pub name: String,
    pub cwd: String,
    pub session_type: SessionType,  // Reuses existing enum from pty_manager
    pub is_git_repo: bool,
    pub created_at_epoch_ms: u64,   // u64 not u128 — safe for JS Number precision
    pub status_at_close: String,
}
```

Note: `created_at_epoch_ms` uses `u64` (not `u128`) because JSON/JavaScript `Number` loses precision above 2^53. A millisecond timestamp fits comfortably in `u64`.

As part of this work, also migrate the existing `SessionListEntry.created_at_epoch_ms` in `pty_manager.rs` and `commands::SessionInfo.created_at_epoch_ms` from `u128` to `u64` for consistency and to fix the same latent JS precision bug for live sessions.

`session_type` reuses the existing `SessionType` enum from `pty_manager` (or a shared copy) rather than a bare `String`, ensuring type safety across the IPC boundary.

### New IPC commands in `commands.rs`

All new commands must be registered in `tauri::generate_handler![]` in `lib.rs`.

| Command | Input | Output | Purpose |
|---------|-------|--------|---------|
| `save_sessions` | `Vec<PersistedSession>` + `HashMap<String, String>` scrollbacks | `()` | Bulk save on app close |
| `save_single_session` | `PersistedSession` + `String` scrollback | `()` | Save on individual session exit |
| `list_persisted_sessions` | none | `Vec<PersistedSession>` | Load on app start |
| `get_session_scrollback` | `session_id: String` | `Option<String>` | Lazy-load scrollback |
| `delete_persisted_session` | `session_id: String` | `()` | Remove persisted session |

### AppState changes (`state.rs`)

Add two new fields:
- `pub persistence_dir: PathBuf` — set during `tauri::Builder::setup` via `app.path().app_data_dir()` + `/persistence`
- `pub persistence_lock: Mutex<()>` — serializes all `sessions.json` write operations

### App data dir access

`persistence_dir` is computed once during setup from `app.path().app_data_dir()`. The `persistence/` and `persistence/scrollback/` subdirs are created on first use.

### Shutdown coordination (`lib.rs`)

Move PTY shutdown from `on_window_event(CloseRequested)` to `RunEvent::Exit` (or `RunEvent::ExitRequested`). This ensures the frontend's save-on-close IPC call completes before PTYs are killed. The frontend calls `event.preventDefault()` on `tauri://close-requested`, performs the save, then calls `appWindow.close()`.

No changes to `pty_manager.rs` — it continues to own only live sessions.

## Frontend Changes

### New "exited" status

- Add `"exited"` to `SessionStatus` type in `src/types/session.ts`
- In `SessionCard.tsx`: add `"exited"` entry to `STATUS_DOT_CLASS` record (gray/muted color), `STATUS_LABEL` record, and update `isRunning()` to return `false` for `"exited"`
- Exited sessions: no input, no resize, no PTY interaction

### Zustand store (`sessionStore.ts`)

- New field on `SessionInfo`: `persisted: boolean` (default `false` for live sessions)
- New field: `scrollbackText?: string` — loaded lazily from disk
- `closeSession()`: if session has `persisted: true`, call `delete_persisted_session` IPC instead of PTY kill
- New action: `loadPersistedSessions()` — called on app init after `list_sessions`, populates store
- New action: `loadScrollback(sessionId)` — fetches scrollback from backend, stores in session

### XTermInstance

- Add `getScrollbackText(lines: number): string` method to `XTermInstanceHandle` (the `useImperativeHandle` ref). This iterates `terminal.buffer.active`, calling `getLine(i).translateToString()` for the last N lines, handling wrapped lines.
- Persisted sessions render a read-only xterm (suppress `onData` and `onResize` callbacks)
- On first view of a persisted session, lazy-load scrollback via `get_session_scrollback` IPC and write into the xterm buffer

### TerminalArea

- Filter persisted sessions to skip PTY output listener registration (`listen("session-output-{id}")`) and input forwarding (`writeToSession`). Persisted sessions get an `XTermInstance` with a `readOnly` prop that suppresses `onData` and `onResize` callbacks. No PTY exit listener needed either.

### Save on close

- Listen to Tauri's `tauri://close-requested` event, call `event.preventDefault()`
- Iterate all sessions (both live and already-exited-but-not-yet-persisted)
- For each: call `getScrollbackText(500)` on the `XTermInstanceHandle` ref
- Call `save_sessions` IPC with metadata array + scrollback map
- Then call `appWindow.close()` to proceed with shutdown

### Save on session exit

- In the existing `session-exit-{id}` event handler, after updating status:
- Call `getScrollbackText(500)` on the session's `XTermInstanceHandle` ref
- Call `save_single_session` IPC to persist immediately

### useInitializeSessions hook

- After loading live sessions from `list_sessions`, also call `list_persisted_sessions`
- Add persisted sessions to the store with `status: "exited"`, `persisted: true`
- Map `is_git_repo` from the `PersistedSession` data (not hard-coded)
- Pre-existing bug: the existing `list_sessions` IPC response (`commands::SessionInfo`) does not include `is_git_repo`, and `useInitializeSessions` hard-codes it to `true`. As part of this work, add `is_git_repo` to `SessionListEntry` and `commands::SessionInfo`, and use it in `useInitializeSessions` for live sessions too.
- If a persisted session ID conflicts with a live session, the live session wins (discard the persisted entry)

## Testing

- **Backend unit tests**: `persistence.rs` — save/load/delete round-trips, atomic write correctness, missing file handling, mutex serialization
- **Frontend unit tests**: Zustand store actions for persisted sessions, scrollback loading, close behavior, `getScrollbackText` extraction
- **Integration**: Manual test — create sessions, quit app, reopen, verify sidebar shows exited sessions with scrollback
