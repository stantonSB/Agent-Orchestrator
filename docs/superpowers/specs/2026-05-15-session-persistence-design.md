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

Writes are atomic: write to `.tmp` file then rename. One scrollback file per session, named by session ID.

## Save Triggers

1. **App close** (`WindowEvent::CloseRequested` / Tauri close event): Frontend iterates all live sessions, serializes each xterm buffer (last 500 lines), calls `save_sessions` IPC with metadata + scrollback, then allows the window to close.

2. **Session exit** (PTY process exits naturally): Frontend saves that individual session's metadata + scrollback immediately via `save_session` IPC. Protects against later app crashes.

3. **Session close by user** (click X on exited session): Calls `delete_persisted_session` IPC to remove persistence files.

## Restore Flow

1. Backend reads `persistence/sessions.json` on startup
2. Frontend calls `list_persisted_sessions` IPC on app init
3. Frontend adds returned sessions to Zustand store with status `"exited"` and `persisted: true`
4. When user selects a persisted session, frontend calls `get_session_scrollback` IPC to lazy-load the scrollback text
5. Scrollback text is written into a read-only xterm instance

## Backend Changes

### New module: `persistence.rs`

Functions:
- `save_sessions(app_data_dir, sessions, scrollbacks)` — atomic write of `sessions.json` + scrollback files
- `save_single_session(app_data_dir, session, scrollback)` — append one session to `sessions.json` + write its scrollback file
- `load_sessions(app_data_dir) -> Vec<PersistedSession>` — read `sessions.json`, return empty vec if missing
- `load_scrollback(app_data_dir, session_id) -> Option<String>` — read one scrollback file
- `delete_session(app_data_dir, session_id)` — remove from `sessions.json` + delete scrollback file

### New struct: `PersistedSession`

```rust
#[derive(Serialize, Deserialize, Clone)]
struct PersistedSession {
    id: String,
    name: String,
    cwd: String,
    session_type: String,
    is_git_repo: bool,
    created_at_epoch_ms: u128,
    status_at_close: String,
}
```

### New IPC commands in `commands.rs`

| Command | Input | Output | Purpose |
|---------|-------|--------|---------|
| `save_sessions` | `Vec<PersistedSession>` + `HashMap<String, String>` scrollbacks | `()` | Bulk save on app close |
| `save_single_session` | `PersistedSession` + `String` scrollback | `()` | Save on individual session exit |
| `list_persisted_sessions` | none | `Vec<PersistedSession>` | Load on app start |
| `get_session_scrollback` | `session_id: String` | `Option<String>` | Lazy-load scrollback |
| `delete_persisted_session` | `session_id: String` | `()` | Remove persisted session |

### App data dir access

Obtained via `app.path().app_data_dir()`. Create `persistence/` and `persistence/scrollback/` subdirs on first use.

No changes to `pty_manager.rs` — it continues to own only live sessions.

## Frontend Changes

### New "exited" status

- Add `"exited"` to `SessionStatus` type in `src/types/session.ts`
- Add gray/muted status dot color in `SessionCard` for exited sessions
- Exited sessions: no input, no resize, no PTY interaction

### Zustand store (`sessionStore.ts`)

- New field on `SessionInfo`: `persisted: boolean` (default `false` for live sessions)
- New field: `scrollbackText?: string` — loaded lazily from disk
- `closeSession()`: if session has `persisted: true`, call `delete_persisted_session` IPC instead of PTY kill
- New action: `loadPersistedSessions()` — called on app init after `list_sessions`, populates store
- New action: `loadScrollback(sessionId)` — fetches scrollback from backend, stores in session

### XTermInstance

- Persisted sessions render a read-only xterm (disable stdin writes)
- On first view of a persisted session, lazy-load scrollback via `get_session_scrollback` IPC and write into the xterm buffer

### TerminalArea

- No structural changes — already uses CSS show/hide. Persisted sessions are just another xterm instance that's hidden until selected.

### Save on close

- Listen to Tauri's `tauri://close-requested` event in the frontend
- Iterate all sessions (both live and already-exited-but-not-yet-persisted)
- For each: serialize xterm buffer (last 500 lines via `terminal.buffer.active`)
- Call `save_sessions` IPC with metadata array + scrollback map
- Then allow the window to close

### Save on session exit

- In the existing `session-exit-{id}` event handler, after updating status:
- Serialize the exiting session's xterm buffer
- Call `save_single_session` IPC to persist immediately

### useInitializeSessions hook

- After loading live sessions from `list_sessions`, also call `list_persisted_sessions`
- Add persisted sessions to the store with `status: "exited"`, `persisted: true`
- If a persisted session ID conflicts with a live session, the live session wins (discard the persisted entry)

## Testing

- **Backend unit tests**: `persistence.rs` — save/load/delete round-trips, atomic write correctness, missing file handling, concurrent access safety
- **Frontend unit tests**: Zustand store actions for persisted sessions, scrollback loading, close behavior
- **Integration**: Manual test — create sessions, quit app, reopen, verify sidebar shows exited sessions with scrollback
