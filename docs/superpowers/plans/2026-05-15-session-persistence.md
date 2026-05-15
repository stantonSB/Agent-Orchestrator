# Session Persistence Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist sessions across app restarts so users can review terminal history from previous sessions.

**Architecture:** New `persistence.rs` Rust module handles atomic JSON file I/O for session metadata and scrollback text files. Frontend intercepts app close to save xterm buffers, and restores persisted sessions on startup as read-only "exited" entries. No database — just JSON + text files in Tauri's `app_data_dir()`.

**Tech Stack:** Rust (serde_json file I/O, std::fs atomic writes), TypeScript/React (Zustand store, xterm.js buffer API, Tauri event listeners)

**Spec:** `docs/superpowers/specs/2026-05-15-session-persistence-design.md`

---

## File Structure

### New files
- `src-tauri/src/persistence.rs` — All persistence logic: `PersistedSession` struct, save/load/delete functions
- `src/hooks/useSaveOnClose.ts` — Hook that intercepts `tauri://close-requested` and saves all sessions
- `src/__tests__/persistence.test.ts` — Frontend tests for persistence store actions

### Modified files
- `src-tauri/src/lib.rs` — Add `mod persistence;`, register new IPC commands, move shutdown to `RunEvent::Exit`
- `src-tauri/src/state.rs` — Add `persistence_dir: PathBuf` and `persistence_lock: Mutex<()>` to `AppState`
- `src-tauri/src/commands.rs` — Add 5 new IPC commands, fix `SessionInfo.created_at_epoch_ms` to `u64`, add `is_git_repo`
- `src-tauri/src/pty_manager.rs` — Fix `created_at_epoch_ms` to `u64`, add `is_git_repo` to `Session`/`SessionListEntry`/`PtyRequest::Create`, add `Serialize`/`Deserialize` to `SessionType`
- `src-tauri/Cargo.toml` — Add `tempfile` dev-dependency
- `src/types/session.ts` — Add `"exited"` to `SessionStatus`, add `persisted` and `scrollbackText` fields to `SessionInfo`
- `src/components/SessionCard/SessionCard.tsx` — Add `"exited"` entries to `STATUS_DOT_CLASS`, `STATUS_LABEL`, update `isRunning()`
- `src/components/SessionCard/SessionCard.module.css` — Add `.statusExited` style
- `src/components/XTermInstance/XTermInstance.tsx` — Add `getScrollbackText()` to handle, add `readOnly` prop
- `src/components/TerminalArea/TerminalArea.tsx` — Add `persisted` to `TerminalSession`, skip PTY listeners for persisted, pass `readOnly`
- `src/stores/sessionStore.ts` — Add `loadPersistedSessions()`, `loadScrollback()`, update `closeSession()` and `dismissSession()` for persisted sessions, add save-on-exit logic to `setupEventListeners()`
- `src/hooks/useInitializeSessions.ts` — Call `loadPersistedSessions()` after live session init, use `is_git_repo` from backend
- `src/App.tsx` — Mount `useSaveOnClose` hook, fix `activeIsRunning` to handle `"exited"` status, pass `persisted` to TerminalArea

---

## Chunk 1: Backend Persistence Module

### Task 1: Migrate existing u128 to u64, add is_git_repo, add Serialize/Deserialize to SessionType

**Files:**
- Modify: `src-tauri/src/pty_manager.rs` — `SessionType` (line 66), `PtyRequest::Create` (line 95), `Session` (line 162), `SessionListEntry` (line 144), session construction (line 566), List handler (line 669), `create()` method (line 266)
- Modify: `src-tauri/src/commands.rs` — `SessionInfo` (line 9), `list_sessions` (line 133)

- [ ] **Step 1: Add Serialize/Deserialize to SessionType enum**

In `src-tauri/src/pty_manager.rs` line 66, update derives:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionType {
    Claude,
    Terminal,
}
```

- [ ] **Step 2: Add is_git_repo to PtyRequest::Create**

In `src-tauri/src/pty_manager.rs` line 95, add `is_git_repo` to the `Create` variant:

```rust
Create {
    name: String,
    cwd: PathBuf,
    command: String,
    args: Vec<String>,
    session_type: SessionType,
    is_git_repo: bool,
    cols: u16,
    rows: u16,
    reply: mpsc::Sender<PtyResponse>,
},
```

- [ ] **Step 3: Add is_git_repo to Session struct, change created_at_epoch_ms to u64**

In `src-tauri/src/pty_manager.rs` line 162, update `Session`:

```rust
struct Session {
    id: SessionId,
    name: String,
    cwd: PathBuf,
    session_type: SessionType,
    is_git_repo: bool,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn std::io::Write + Send>,
    created_at: Instant,
    created_at_epoch_ms: u64,
    _reader_handle: thread::JoinHandle<()>,
}
```

- [ ] **Step 4: Update SessionListEntry — u64, SessionType enum, is_git_repo**

In `src-tauri/src/pty_manager.rs` line 144:

```rust
#[derive(Debug, Clone)]
pub struct SessionListEntry {
    pub id: SessionId,
    pub name: String,
    pub cwd: PathBuf,
    pub created_at_epoch_ms: u64,
    pub session_type: SessionType,
    pub is_git_repo: bool,
}
```

- [ ] **Step 5: Update PtyManagerHandle::create() to pass is_git_repo**

In `src-tauri/src/pty_manager.rs` line 277, the `create()` method builds `PtyRequest::Create`. Add `is_git_repo` to the request:

```rust
self.request(|reply| PtyRequest::Create {
    name,
    cwd,
    command,
    args,
    session_type,
    is_git_repo,
    cols,
    rows,
    reply,
})
```

- [ ] **Step 6: Update session construction in manager loop**

In the Create handler in the manager loop (around line 566-579), destructure `is_git_repo` from the request and store it. Cast timestamp to `u64`:

```rust
sessions.insert(
    id.clone(),
    Session {
        id: id.clone(),
        name,
        cwd,
        session_type,
        is_git_repo,
        master: pair.master,
        writer,
        created_at: Instant::now(),
        created_at_epoch_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
        _reader_handle: reader_handle,
    },
);
```

Make sure the `Create` match arm destructures `is_git_repo`:

```rust
PtyRequest::Create { name, cwd, command, args, session_type, is_git_repo, cols, rows, reply } => {
```

- [ ] **Step 7: Update List handler to use SessionType directly and include is_git_repo**

In the List handler (line 669-681):

```rust
PtyRequest::List { reply } => {
    let entries: Vec<SessionListEntry> = sessions
        .values()
        .map(|s| SessionListEntry {
            id: s.id.clone(),
            name: s.name.clone(),
            cwd: s.cwd.clone(),
            created_at_epoch_ms: s.created_at_epoch_ms,
            session_type: s.session_type,
            is_git_repo: s.is_git_repo,
        })
        .collect();
    let _ = reply.send(PtyResponse::Sessions(entries));
}
```

- [ ] **Step 8: Update commands::SessionInfo to u64, add is_git_repo**

In `src-tauri/src/commands.rs` line 9:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub name: String,
    pub cwd: PathBuf,
    pub created_at_epoch_ms: u64,
    pub session_type: String,
    pub is_git_repo: bool,
}
```

- [ ] **Step 9: Update list_sessions mapping**

In `src-tauri/src/commands.rs`, update the `From` impl or manual mapping in `list_sessions` to include `is_git_repo` and serialize `session_type` from the enum:

```rust
SessionInfo {
    id: entry.id,
    name: entry.name,
    cwd: entry.cwd,
    created_at_epoch_ms: entry.created_at_epoch_ms,
    session_type: match entry.session_type {
        pty_manager::SessionType::Claude => "claude".to_string(),
        pty_manager::SessionType::Terminal => "terminal".to_string(),
    },
    is_git_repo: entry.is_git_repo,
}
```

- [ ] **Step 10: Verify backend compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles with no errors.

- [ ] **Step 11: Commit**

```bash
git add src-tauri/src/pty_manager.rs src-tauri/src/commands.rs
git commit -m "fix: migrate created_at_epoch_ms to u64, add is_git_repo to session list, add serde to SessionType"
```

---

### Task 2: Create persistence.rs module with tests

**Files:**
- Create: `src-tauri/src/persistence.rs`
- Modify: `src-tauri/Cargo.toml` (add tempfile dev-dep)

- [ ] **Step 1: Create persistence.rs with all functions and inline tests**

Create `src-tauri/src/persistence.rs`:

```rust
use crate::pty_manager::SessionType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSession {
    pub id: String,
    pub name: String,
    pub cwd: String,
    pub session_type: SessionType,
    pub is_git_repo: bool,
    pub created_at_epoch_ms: u64,
    pub status_at_close: String,
}

/// Ensure persistence directories exist.
pub fn ensure_dirs(persistence_dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(persistence_dir.join("scrollback"))
}

/// Load all persisted sessions. Returns empty vec if file is missing or unreadable.
pub fn load_sessions(persistence_dir: &Path) -> Vec<PersistedSession> {
    let path = persistence_dir.join("sessions.json");
    match fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Load scrollback text for a single session. Returns None if file is missing.
pub fn load_scrollback(persistence_dir: &Path, session_id: &str) -> Option<String> {
    let path = scrollback_path(persistence_dir, session_id);
    fs::read_to_string(path).ok()
}

/// Atomically save sessions and their scrollback.
/// Merges with any already-persisted sessions (from save_single_session calls)
/// to avoid overwriting them.
pub fn save_sessions(
    persistence_dir: &Path,
    sessions: &[PersistedSession],
    scrollbacks: &HashMap<String, String>,
) -> std::io::Result<()> {
    ensure_dirs(persistence_dir)?;

    // Load existing persisted sessions and merge
    let mut existing = load_sessions(persistence_dir);
    let new_ids: std::collections::HashSet<&str> =
        sessions.iter().map(|s| s.id.as_str()).collect();
    // Keep existing sessions that are NOT being overwritten by this save
    existing.retain(|s| !new_ids.contains(s.id.as_str()));
    existing.extend(sessions.iter().cloned());

    let json = serde_json::to_string_pretty(&existing)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    atomic_write(&persistence_dir.join("sessions.json"), json.as_bytes())?;

    for (id, text) in scrollbacks {
        atomic_write(&scrollback_path(persistence_dir, id), text.as_bytes())?;
    }

    Ok(())
}

/// Append/update a single session in the persisted list and save its scrollback.
/// Caller must hold the persistence lock.
pub fn save_single_session(
    persistence_dir: &Path,
    session: &PersistedSession,
    scrollback: &str,
) -> std::io::Result<()> {
    ensure_dirs(persistence_dir)?;

    let mut sessions = load_sessions(persistence_dir);
    sessions.retain(|s| s.id != session.id);
    sessions.push(session.clone());

    let json = serde_json::to_string_pretty(&sessions)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    atomic_write(&persistence_dir.join("sessions.json"), json.as_bytes())?;

    atomic_write(
        &scrollback_path(persistence_dir, &session.id),
        scrollback.as_bytes(),
    )
}

/// Delete a persisted session and its scrollback file.
/// Caller must hold the persistence lock.
pub fn delete_session(persistence_dir: &Path, session_id: &str) -> std::io::Result<()> {
    let mut sessions = load_sessions(persistence_dir);
    sessions.retain(|s| s.id != session_id);

    let json = serde_json::to_string_pretty(&sessions)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    atomic_write(&persistence_dir.join("sessions.json"), json.as_bytes())?;

    let _ = fs::remove_file(scrollback_path(persistence_dir, session_id));
    Ok(())
}

fn scrollback_path(persistence_dir: &Path, session_id: &str) -> PathBuf {
    persistence_dir
        .join("scrollback")
        .join(format!("{}.txt", session_id))
}

fn atomic_write(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp_path = path.with_extension("tmp");
    let mut file = fs::File::create(&tmp_path)?;
    file.write_all(data)?;
    file.sync_all()?;
    fs::rename(&tmp_path, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(id: &str, name: &str) -> PersistedSession {
        PersistedSession {
            id: id.to_string(),
            name: name.to_string(),
            cwd: "/tmp/test".to_string(),
            session_type: SessionType::Claude,
            is_git_repo: true,
            created_at_epoch_ms: 1715000000000,
            status_at_close: "working".to_string(),
        }
    }

    #[test]
    fn test_save_and_load_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let pd = dir.path().join("persistence");

        let sessions = vec![make_session("aaa", "s1"), make_session("bbb", "s2")];
        let mut scrollbacks = HashMap::new();
        scrollbacks.insert("aaa".to_string(), "line1\nline2\n".to_string());
        scrollbacks.insert("bbb".to_string(), "output\n".to_string());

        save_sessions(&pd, &sessions, &scrollbacks).unwrap();

        let loaded = load_sessions(&pd);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "aaa");
        assert_eq!(loaded[1].id, "bbb");
        assert_eq!(
            load_scrollback(&pd, "aaa"),
            Some("line1\nline2\n".to_string())
        );
        assert_eq!(load_scrollback(&pd, "ccc"), None);
    }

    #[test]
    fn test_save_sessions_merges_with_existing() {
        let dir = tempfile::tempdir().unwrap();
        let pd = dir.path().join("persistence");

        // First, persist session "aaa" via save_single_session
        save_single_session(&pd, &make_session("aaa", "first"), "scroll-a").unwrap();

        // Then do a bulk save with session "bbb" only
        let sessions = vec![make_session("bbb", "second")];
        let mut scrollbacks = HashMap::new();
        scrollbacks.insert("bbb".to_string(), "scroll-b".to_string());
        save_sessions(&pd, &sessions, &scrollbacks).unwrap();

        // Both should exist
        let loaded = load_sessions(&pd);
        assert_eq!(loaded.len(), 2);
        let ids: Vec<&str> = loaded.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"aaa"));
        assert!(ids.contains(&"bbb"));
    }

    #[test]
    fn test_save_single_session() {
        let dir = tempfile::tempdir().unwrap();
        let pd = dir.path().join("persistence");

        save_single_session(&pd, &make_session("aaa", "first"), "scroll-1").unwrap();
        save_single_session(&pd, &make_session("bbb", "second"), "scroll-2").unwrap();

        let loaded = load_sessions(&pd);
        assert_eq!(loaded.len(), 2);
    }

    #[test]
    fn test_save_single_session_replaces_existing() {
        let dir = tempfile::tempdir().unwrap();
        let pd = dir.path().join("persistence");

        save_single_session(&pd, &make_session("aaa", "original"), "scroll-1").unwrap();

        let updated = PersistedSession {
            name: "updated".to_string(),
            status_at_close: "finished".to_string(),
            ..make_session("aaa", "")
        };
        save_single_session(&pd, &updated, "scroll-new").unwrap();

        let loaded = load_sessions(&pd);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "updated");
        assert_eq!(
            load_scrollback(&pd, "aaa"),
            Some("scroll-new".to_string())
        );
    }

    #[test]
    fn test_delete_session() {
        let dir = tempfile::tempdir().unwrap();
        let pd = dir.path().join("persistence");

        save_single_session(&pd, &make_session("aaa", "first"), "scroll-1").unwrap();
        save_single_session(&pd, &make_session("bbb", "second"), "scroll-2").unwrap();

        delete_session(&pd, "aaa").unwrap();

        let loaded = load_sessions(&pd);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "bbb");
        assert_eq!(load_scrollback(&pd, "aaa"), None);
        assert!(load_scrollback(&pd, "bbb").is_some());
    }

    #[test]
    fn test_load_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let pd = dir.path().join("nonexistent");
        assert!(load_sessions(&pd).is_empty());
        assert_eq!(load_scrollback(&pd, "xyz"), None);
    }
}
```

- [ ] **Step 2: Add tempfile dev-dependency**

In `src-tauri/Cargo.toml`, add:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Run tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass (existing + new persistence tests).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/persistence.rs src-tauri/Cargo.toml
git commit -m "feat: add persistence module with save/load/delete and tests"
```

---

### Task 3: Update AppState, add IPC commands, wire into lib.rs

**Files:**
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add persistence fields to AppState**

In `src-tauri/src/state.rs`, add `PathBuf` import and new fields:

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::pty_manager::PtyManagerHandle;
use crate::status_parser::StatusTracker;
use crate::status_server::StatusServer;

pub struct AppState {
    pub pty: PtyManagerHandle,
    pub status_server: StatusServer,
    pub status_trackers: Arc<Mutex<HashMap<String, StatusTracker>>>,
    pub persistence_dir: PathBuf,
    pub persistence_lock: Mutex<()>,
}
```

- [ ] **Step 2: Add 5 new IPC commands to commands.rs**

At the bottom of `src-tauri/src/commands.rs`, add:

```rust
use crate::persistence::{self, PersistedSession};

#[tauri::command]
pub fn save_sessions(
    state: tauri::State<'_, AppState>,
    sessions: Vec<PersistedSession>,
    scrollbacks: HashMap<String, String>,
) -> Result<(), String> {
    let _lock = state.persistence_lock.lock().unwrap();
    persistence::save_sessions(&state.persistence_dir, &sessions, &scrollbacks)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_single_session(
    state: tauri::State<'_, AppState>,
    session: PersistedSession,
    scrollback: String,
) -> Result<(), String> {
    let _lock = state.persistence_lock.lock().unwrap();
    persistence::save_single_session(&state.persistence_dir, &session, &scrollback)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_persisted_sessions(
    state: tauri::State<'_, AppState>,
) -> Vec<PersistedSession> {
    persistence::load_sessions(&state.persistence_dir)
}

#[tauri::command]
pub fn get_session_scrollback(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Option<String> {
    persistence::load_scrollback(&state.persistence_dir, &session_id)
}

#[tauri::command]
pub fn delete_persisted_session(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let _lock = state.persistence_lock.lock().unwrap();
    persistence::delete_session(&state.persistence_dir, &session_id)
        .map_err(|e| e.to_string())
}
```

Also add `use std::collections::HashMap;` at the top of commands.rs if not already present.

- [ ] **Step 3: Add mod persistence and register commands in lib.rs**

In `src-tauri/src/lib.rs`, add `pub mod persistence;` after line 6:

```rust
pub mod persistence;
```

- [ ] **Step 4: Compute persistence_dir in setup and add to AppState**

In `src-tauri/src/lib.rs`, inside the `.setup(|app| { ... })` closure, before `app.manage(AppState { ... })` (line 112), compute the persistence dir:

```rust
let persistence_dir = app.path().app_data_dir()
    .expect("failed to resolve app data dir")
    .join("persistence");
```

Update `app.manage(AppState { ... })` (line 112-116) to include:

```rust
app.manage(AppState {
    pty: pty_handle,
    status_server,
    status_trackers: status_trackers_for_state,
    persistence_dir,
    persistence_lock: Mutex::new(()),
});
```

Add `use std::sync::Mutex;` to the imports if not already covered.

- [ ] **Step 5: Register new commands in invoke_handler**

In `src-tauri/src/lib.rs`, update the invoke_handler (line 122-131):

```rust
.invoke_handler(tauri::generate_handler![
    commands::create_session,
    commands::close_session,
    commands::write_to_session,
    commands::resize_session,
    commands::rename_session,
    commands::list_sessions,
    commands::check_is_git_repo,
    commands::get_session_status,
    commands::save_sessions,
    commands::save_single_session,
    commands::list_persisted_sessions,
    commands::get_session_scrollback,
    commands::delete_persisted_session,
])
```

- [ ] **Step 6: Move shutdown from on_window_event to RunEvent::Exit**

Replace the current `.on_window_event` block (lines 132-139) and `.run()` call (line 140-141).

Remove the window event handler's shutdown logic — keep the handler but make it a no-op:

```rust
.on_window_event(|_window, _event| {
    // Shutdown moved to RunEvent::Exit to allow frontend save-on-close
})
```

Change the `.run(tauri::generate_context!())` pattern to the two-step `.build()?.run()` pattern:

```rust
.build(tauri::generate_context!())
.expect("error while building tauri application")
.run(|app_handle, event| {
    if let tauri::RunEvent::Exit = event {
        if let Some(state) = app_handle.try_state::<AppState>() {
            state.pty.shutdown();
            state.status_server.stop();
        }
    }
});
```

- [ ] **Step 7: Verify backend compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles with no errors.

- [ ] **Step 8: Run all backend tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/state.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add persistence IPC commands, move shutdown to RunEvent::Exit"
```

---

## Chunk 2: Frontend Types, SessionCard, and XTermInstance

### Task 4: Add "exited" status to frontend types

**Files:**
- Modify: `src/types/session.ts`

- [ ] **Step 1: Add "exited" to SessionStatus and new fields to SessionInfo**

In `src/types/session.ts`:

```typescript
export type SessionStatus =
  | "starting"
  | "working"
  | "idle"
  | "needs_attention"
  | "finished"
  | "error"
  | "terminal"
  | "exited";
```

Add optional fields to `SessionInfo`:

```typescript
export interface SessionInfo {
  id: string;
  name: string;
  status: SessionStatus;
  createdAt: number;
  cwd: string;
  sessionType: "claude" | "terminal";
  isGitRepo: boolean;
  persisted?: boolean;
  scrollbackText?: string;
}
```

- [ ] **Step 2: Commit**

```bash
git add src/types/session.ts
git commit -m "feat: add 'exited' status and persistence fields to SessionInfo"
```

---

### Task 5: Update SessionCard for "exited" status

**Files:**
- Modify: `src/components/SessionCard/SessionCard.tsx` (lines 19-41)
- Modify: `src/components/SessionCard/SessionCard.module.css`

- [ ] **Step 1: Add "exited" entries to status maps and update isRunning**

In `src/components/SessionCard/SessionCard.tsx`:

Add to `STATUS_DOT_CLASS` (line 19-27):
```typescript
exited: styles.statusExited,
```

Add to `STATUS_LABEL` (line 29-37):
```typescript
exited: "Exited",
```

Update `isRunning` (line 39-41):
```typescript
function isRunning(status: SessionStatus): boolean {
  return status !== "finished" && status !== "error" && status !== "exited";
}
```

- [ ] **Step 2: Add .statusExited CSS class**

In `src/components/SessionCard/SessionCard.module.css`, find the other status color classes and add:

```css
.statusExited {
  background-color: #6b7280;
}
```

- [ ] **Step 3: Verify TypeScript compiles**

Run: `npx tsc --noEmit`
Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add src/components/SessionCard/SessionCard.tsx src/components/SessionCard/SessionCard.module.css
git commit -m "feat: add 'exited' status styling to SessionCard"
```

---

### Task 6: Add getScrollbackText and readOnly to XTermInstance

**Files:**
- Modify: `src/components/XTermInstance/XTermInstance.tsx`

- [ ] **Step 1: Add readOnly prop**

In `src/components/XTermInstance/XTermInstance.tsx`, add `readOnly` to `XTermInstanceProps` (line 27-39):

```typescript
interface XTermInstanceProps {
  sessionId: string;
  cwd: string;
  onData?: (data: string) => void;
  onResize?: (cols: number, rows: number) => void;
  mockMode?: boolean;
  isActive: boolean;
  readOnly?: boolean;
}
```

- [ ] **Step 2: Add getScrollbackText to XTermInstanceHandle**

Update the interface (line 14-21):

```typescript
export interface XTermInstanceHandle {
  write: (data: string | Uint8Array) => void;
  fit: () => void;
  findNext: (query: string) => boolean;
  findPrevious: (query: string) => boolean;
  clearSearch: () => void;
  focus: () => void;
  getScrollbackText: (lines: number) => string;
}
```

- [ ] **Step 3: Implement readOnly by nulling callbacks before useTerminal**

In the component function (line 46), suppress callbacks when readOnly. The `onData` and `onResize` callbacks are registered inside `useTerminal` (lines 172-179 of `useTerminal.ts`) via `term.onData()` and `term.onResize()`. The simplest approach is to null them before passing to `useTerminal`:

```typescript
export const XTermInstance = forwardRef<XTermInstanceHandle, XTermInstanceProps>(
  function XTermInstance({ sessionId: _sessionId, cwd, onData, onResize, mockMode, isActive, readOnly }, ref) {
    const { containerRef, write, fit, getTerminal, findNext, findPrevious, clearSearch } = useTerminal({
      onData: readOnly ? undefined : onData,
      onResize: readOnly ? undefined : onResize,
      mockMode,
      cwd,
    });
```

- [ ] **Step 4: Implement getScrollbackText in useImperativeHandle**

Update the `useImperativeHandle` block (line 55-62):

```typescript
useImperativeHandle(ref, () => ({
  write,
  fit,
  findNext,
  findPrevious,
  clearSearch,
  focus: () => getTerminal()?.focus(),
  getScrollbackText: (lines: number) => {
    const term = getTerminal();
    if (!term) return "";
    const buffer = term.buffer.active;
    const totalLines = buffer.length;
    const startLine = Math.max(0, totalLines - lines);
    const result: string[] = [];
    for (let i = startLine; i < totalLines; i++) {
      const line = buffer.getLine(i);
      if (line) {
        result.push(line.translateToString(true));
      }
    }
    return result.join("\n");
  },
}), [write, fit, findNext, findPrevious, clearSearch, getTerminal]);
```

- [ ] **Step 5: Verify TypeScript compiles**

Run: `npx tsc --noEmit`
Expected: No errors.

- [ ] **Step 6: Commit**

```bash
git add src/components/XTermInstance/XTermInstance.tsx
git commit -m "feat: add getScrollbackText and readOnly mode to XTermInstance"
```

---

## Chunk 3: Frontend Store, TerminalArea, and App Integration

### Task 7: Update TerminalArea for persisted sessions

**Files:**
- Modify: `src/components/TerminalArea/TerminalArea.tsx`

- [ ] **Step 1: Add persisted to TerminalSession interface**

In `src/components/TerminalArea/TerminalArea.tsx` line 20-24:

```typescript
export interface TerminalSession {
  id: string;
  name: string;
  cwd: string;
  persisted?: boolean;
}
```

- [ ] **Step 2: Skip PTY listener registration for persisted sessions**

In the `useEffect` that registers listeners (line 70-131), add a guard in both loops. In the output listener loop (line 76-98), after `if (outputListeners.current.has(sid)) continue;`, add:

```typescript
// Skip PTY listeners for persisted (read-only) sessions
const session = sessions.find(s => s.id === sid);
if (session?.persisted) continue;
```

Add the same guard in the exit listener loop (line 101-112):

```typescript
const session = sessions.find(s => s.id === sid);
if (session?.persisted) continue;
```

- [ ] **Step 3: Pass readOnly prop to XTermInstance for persisted sessions**

In the render section (line 255-268), add `readOnly`:

```typescript
{sessions.map((session) => (
  <XTermInstance
    key={session.id}
    ref={setRef(session.id)}
    sessionId={session.id}
    cwd={session.cwd}
    isActive={session.id === activeSessionId}
    mockMode={mockMode}
    readOnly={session.persisted}
    onData={(data) => handleSessionData(session.id, data)}
    onResize={(cols, rows) =>
      handleSessionResize(session.id, cols, rows)
    }
  />
))}
```

Note: Since `readOnly` nulls out onData/onResize in XTermInstance before passing to useTerminal, the callbacks passed here will be ignored for persisted sessions. No additional guards needed in `handleSessionData`/`handleSessionResize`.

- [ ] **Step 4: Verify TypeScript compiles**

Run: `npx tsc --noEmit`
Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add src/components/TerminalArea/TerminalArea.tsx
git commit -m "feat: skip PTY listeners and mark persisted sessions as readOnly"
```

---

### Task 8: Update Zustand store with persistence actions

**Files:**
- Modify: `src/stores/sessionStore.ts`

- [ ] **Step 1: Add persistence actions to SessionState interface**

In `src/stores/sessionStore.ts` (line 7-36), add to the interface:

```typescript
loadPersistedSessions: () => Promise<void>;
loadScrollback: (sessionId: string) => Promise<void>;
```

- [ ] **Step 2: Update closeSession to handle persisted sessions**

Replace the `closeSession` action (around line 251-254):

```typescript
closeSession: async (id: string) => {
  const session = get().sessions.get(id);
  if (session?.persisted) {
    try {
      await invoke("delete_persisted_session", { sessionId: id });
    } catch (err) {
      console.error("Failed to delete persisted session:", err);
    }
    get().removeSession(id);
    return;
  }
  await invoke("close_session", { id });
  get().removeSession(id);
},
```

- [ ] **Step 3: Update dismissSession to handle persisted sessions**

The existing `dismissSession` (around line 162-181) removes a finished/error session from the store. For persisted sessions, it also needs to delete from disk. Add a guard at the start:

```typescript
dismissSession: (id: string) => {
  const session = get().sessions.get(id);
  if (session?.persisted) {
    // Delete from disk (fire-and-forget)
    invoke("delete_persisted_session", { sessionId: id }).catch((err) => {
      console.error("Failed to delete persisted session:", err);
    });
  }
  // ... rest of existing dismissSession logic (remove from store, clean up listeners, etc.)
```

- [ ] **Step 4: Implement loadPersistedSessions action**

Add in the store creator:

```typescript
loadPersistedSessions: async () => {
  try {
    const persisted = await invoke<Array<{
      id: string;
      name: string;
      cwd: string;
      session_type: string;
      is_git_repo: boolean;
      created_at_epoch_ms: number;
      status_at_close: string;
    }>>("list_persisted_sessions");

    const { sessions, addSession } = get();

    for (const raw of persisted) {
      if (sessions.has(raw.id)) continue;

      const sessionType = raw.session_type === "terminal" ? "terminal" as const : "claude" as const;
      const session: SessionInfo = {
        id: raw.id,
        name: raw.name,
        cwd: raw.cwd,
        createdAt: raw.created_at_epoch_ms,
        status: "exited",
        sessionType,
        isGitRepo: raw.is_git_repo,
        persisted: true,
      };
      addSession(session);
    }
  } catch (err) {
    console.error("Failed to load persisted sessions:", err);
  }
},
```

- [ ] **Step 5: Implement loadScrollback action**

```typescript
loadScrollback: async (sessionId: string) => {
  const session = get().sessions.get(sessionId);
  if (!session || session.scrollbackText !== undefined) return;

  try {
    const text = await invoke<string | null>("get_session_scrollback", { sessionId });
    if (text !== null) {
      set((state) => {
        const sessions = new Map(state.sessions);
        const s = sessions.get(sessionId);
        if (s) {
          sessions.set(sessionId, { ...s, scrollbackText: text });
        }
        return { sessions };
      });
    }
  } catch (err) {
    console.error("Failed to load scrollback:", err);
  }
},
```

- [ ] **Step 6: Add save-on-exit logic to setupEventListeners**

In the existing `setupEventListeners` action (around line 267-326), inside the `session-exit-{id}` event handler, after the status update logic, add persistence:

```typescript
// Persist session on exit (fire-and-forget to avoid blocking the exit handler)
const exitedSession = get().sessions.get(sessionId);
if (exitedSession && !exitedSession.persisted) {
  // Defer scrollback capture slightly to let final output arrive
  setTimeout(async () => {
    try {
      // Access terminal refs through a global registry (see Task 9)
      const scrollback = window.__aoGetScrollback?.(sessionId) ?? "";
      await invoke("save_single_session", {
        session: {
          id: exitedSession.id,
          name: exitedSession.name,
          cwd: exitedSession.cwd,
          session_type: exitedSession.sessionType,
          is_git_repo: exitedSession.isGitRepo,
          created_at_epoch_ms: exitedSession.createdAt,
          status_at_close: get().sessions.get(sessionId)?.status ?? exitedSession.status,
        },
        scrollback,
      });
    } catch (err) {
      console.error(`Failed to persist session ${sessionId} on exit:`, err);
    }
  }, 500);
}
```

Note: `window.__aoGetScrollback` is a simple global function registered by `TerminalArea` (see Task 9). This avoids the complex listener-duplication problems of a separate hook.

- [ ] **Step 7: Verify TypeScript compiles**

Run: `npx tsc --noEmit`
Expected: May need type declaration for `window.__aoGetScrollback` — add to a `global.d.ts` or inline type assertion.

- [ ] **Step 8: Commit**

```bash
git add src/stores/sessionStore.ts
git commit -m "feat: add persistence actions to Zustand store, save on session exit"
```

---

### Task 9: Update App.tsx, useInitializeSessions, TerminalArea refs, and useSaveOnClose

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/hooks/useInitializeSessions.ts`
- Modify: `src/components/TerminalArea/TerminalArea.tsx` (expose scrollback getter)
- Create: `src/hooks/useSaveOnClose.ts`

- [ ] **Step 1: Fix activeIsRunning in App.tsx to handle "exited"**

In `src/App.tsx` line 40-42, update `activeIsRunning`:

```typescript
const activeIsRunning = activeSession
  ? activeSession.status !== "finished" && activeSession.status !== "error" && activeSession.status !== "exited"
  : false;
```

- [ ] **Step 2: Pass persisted flag to TerminalArea sessions**

In `src/App.tsx`, the `sessionList` is passed directly to `TerminalArea` at line 133. The `SessionInfo` type now has `persisted`, which maps to `TerminalSession.persisted`. Since `TerminalArea` accepts `TerminalSession[]` and `SessionInfo` satisfies that interface (id, name, cwd, plus optional persisted), this works without changes. Verify that the type assignment is compatible.

- [ ] **Step 3: Register global scrollback getter in TerminalArea**

In `src/components/TerminalArea/TerminalArea.tsx`, add a global function that the store's save-on-exit logic can call:

```typescript
// Register a global function for the store to access scrollback
useEffect(() => {
  (window as any).__aoGetScrollback = (sessionId: string): string => {
    const handle = refsMap.current.get(sessionId);
    return handle?.getScrollbackText(500) ?? "";
  };
  return () => {
    delete (window as any).__aoGetScrollback;
  };
}, []);
```

Also register a `__aoGetAllScrollbacks` for the save-on-close hook:

```typescript
(window as any).__aoGetAllScrollbacks = (): Record<string, string> => {
  const result: Record<string, string> = {};
  for (const [id, handle] of refsMap.current) {
    result[id] = handle.getScrollbackText(500);
  }
  return result;
};
```

- [ ] **Step 4: Add scrollback loading for persisted sessions when they become active**

In `src/components/TerminalArea/TerminalArea.tsx`, add a `useEffect` to load and display scrollback when a persisted session becomes active:

```typescript
// Load scrollback for persisted sessions when they become active
const loadScrollback = useSessionStore((s) => s.loadScrollback);

useEffect(() => {
  if (!activeSessionId) return;
  const session = sessions.find(s => s.id === activeSessionId);
  if (!session?.persisted) return;

  const doLoad = async () => {
    await loadScrollback(activeSessionId);
    // After loading, get the text from the store and write to terminal
    const updated = useSessionStore.getState().sessions.get(activeSessionId);
    if (updated?.scrollbackText) {
      const handle = refsMap.current.get(activeSessionId);
      if (handle) {
        handle.write(updated.scrollbackText);
      }
    }
  };

  doLoad();
}, [activeSessionId, sessions, loadScrollback]);
```

Add `import { useSessionStore } from "../../stores/sessionStore";` at the top.

- [ ] **Step 5: Update useInitializeSessions to load persisted sessions and use is_git_repo**

Replace `src/hooks/useInitializeSessions.ts`:

```typescript
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "../stores/sessionStore";
import type { SessionInfo } from "../types/session";

export function useInitializeSessions() {
  const addSession = useSessionStore((s) => s.addSession);
  const setupEventListeners = useSessionStore((s) => s.setupEventListeners);
  const loadPersistedSessions = useSessionStore((s) => s.loadPersistedSessions);

  useEffect(() => {
    async function init() {
      try {
        const existing = await invoke<Array<{
          id: string;
          name: string;
          cwd: string;
          created_at_epoch_ms: number;
          session_type: string;
          is_git_repo: boolean;
        }>>("list_sessions");
        for (const raw of existing) {
          const sessionType = raw.session_type === "terminal" ? "terminal" as const : "claude" as const;
          const session: SessionInfo = {
            id: raw.id,
            name: raw.name,
            cwd: raw.cwd,
            createdAt: raw.created_at_epoch_ms,
            status: sessionType === "terminal" ? "terminal" : "idle",
            sessionType,
            isGitRepo: raw.is_git_repo,
          };
          addSession(session);
          setupEventListeners(session.id);
        }

        // Load persisted sessions after live ones (live wins on ID conflict)
        await loadPersistedSessions();
      } catch (err) {
        console.error("Failed to initialize sessions:", err);
      }
    }
    init();
  }, []);
}
```

- [ ] **Step 6: Create useSaveOnClose hook**

Create `src/hooks/useSaveOnClose.ts`:

```typescript
import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "../stores/sessionStore";

export function useSaveOnClose() {
  useEffect(() => {
    const appWindow = getCurrentWindow();

    const unlisten = appWindow.onCloseRequested(async (event) => {
      event.preventDefault();

      try {
        const state = useSessionStore.getState();
        const sessions = Array.from(state.sessions.values());
        const persistSessions: Array<{
          id: string;
          name: string;
          cwd: string;
          session_type: string;
          is_git_repo: boolean;
          created_at_epoch_ms: number;
          status_at_close: string;
        }> = [];
        const scrollbacks: Record<string, string> = {};

        // Get all scrollbacks from the global getter
        const allScrollbacks = (window as any).__aoGetAllScrollbacks?.() ?? {};

        for (const session of sessions) {
          // Skip already-persisted sessions — they're already on disk
          if (session.persisted) continue;

          persistSessions.push({
            id: session.id,
            name: session.name,
            cwd: session.cwd,
            session_type: session.sessionType,
            is_git_repo: session.isGitRepo,
            created_at_epoch_ms: session.createdAt,
            status_at_close: session.status,
          });

          scrollbacks[session.id] = allScrollbacks[session.id] ?? "";
        }

        if (persistSessions.length > 0) {
          await invoke("save_sessions", {
            sessions: persistSessions,
            scrollbacks,
          });
        }
      } catch (err) {
        console.error("Failed to save sessions on close:", err);
      }

      // Use destroy() to actually close without re-triggering onCloseRequested
      await appWindow.destroy();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);
}
```

- [ ] **Step 7: Mount useSaveOnClose in App.tsx**

In `src/App.tsx`, add import and mount:

```typescript
import { useSaveOnClose } from "./hooks/useSaveOnClose";
```

Inside the `App` component, after `useInitializeSessions()`:

```typescript
useSaveOnClose();
```

- [ ] **Step 8: Verify TypeScript compiles**

Run: `npx tsc --noEmit`
Expected: No errors.

- [ ] **Step 9: Commit**

```bash
git add src/App.tsx src/hooks/useInitializeSessions.ts src/hooks/useSaveOnClose.ts src/components/TerminalArea/TerminalArea.tsx src/stores/sessionStore.ts
git commit -m "feat: wire persistence into app - save on close, save on exit, restore on startup"
```

---

## Chunk 4: Testing and Verification

### Task 10: Frontend persistence tests

**Files:**
- Create: `src/__tests__/persistence.test.ts`

- [ ] **Step 1: Write tests for store persistence actions**

Create `src/__tests__/persistence.test.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { useSessionStore } from "../stores/sessionStore";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { invoke } from "@tauri-apps/api/core";
const mockInvoke = vi.mocked(invoke);

describe("Session persistence", () => {
  beforeEach(() => {
    useSessionStore.setState({
      sessions: new Map(),
      activeSessionId: null,
    });
    vi.clearAllMocks();
  });

  describe("loadPersistedSessions", () => {
    it("loads persisted sessions with exited status", async () => {
      mockInvoke.mockResolvedValueOnce([
        {
          id: "abc",
          name: "test-session",
          cwd: "/tmp",
          session_type: "claude",
          is_git_repo: true,
          created_at_epoch_ms: 1715000000000,
          status_at_close: "working",
        },
      ]);

      await useSessionStore.getState().loadPersistedSessions();

      const session = useSessionStore.getState().sessions.get("abc");
      expect(session).toBeDefined();
      expect(session!.status).toBe("exited");
      expect(session!.persisted).toBe(true);
      expect(session!.name).toBe("test-session");
    });

    it("skips persisted sessions that conflict with live sessions", async () => {
      useSessionStore.getState().addSession({
        id: "abc",
        name: "live-session",
        cwd: "/tmp",
        status: "working",
        createdAt: 1715000000000,
        sessionType: "claude",
        isGitRepo: true,
      });

      mockInvoke.mockResolvedValueOnce([
        {
          id: "abc",
          name: "persisted-session",
          cwd: "/tmp",
          session_type: "claude",
          is_git_repo: true,
          created_at_epoch_ms: 1715000000000,
          status_at_close: "finished",
        },
      ]);

      await useSessionStore.getState().loadPersistedSessions();

      const session = useSessionStore.getState().sessions.get("abc");
      expect(session!.name).toBe("live-session");
      expect(session!.persisted).toBeUndefined();
    });
  });

  describe("closeSession for persisted sessions", () => {
    it("calls delete_persisted_session for persisted sessions", async () => {
      mockInvoke.mockResolvedValue(undefined);

      useSessionStore.getState().addSession({
        id: "abc",
        name: "old-session",
        cwd: "/tmp",
        status: "exited",
        createdAt: 1715000000000,
        sessionType: "claude",
        isGitRepo: true,
        persisted: true,
      });

      await useSessionStore.getState().closeSession("abc");

      expect(mockInvoke).toHaveBeenCalledWith("delete_persisted_session", {
        sessionId: "abc",
      });
      expect(useSessionStore.getState().sessions.has("abc")).toBe(false);
    });
  });

  describe("loadScrollback", () => {
    it("loads scrollback text for a session", async () => {
      useSessionStore.getState().addSession({
        id: "abc",
        name: "test",
        cwd: "/tmp",
        status: "exited",
        createdAt: 1715000000000,
        sessionType: "claude",
        isGitRepo: true,
        persisted: true,
      });

      mockInvoke.mockResolvedValueOnce("line1\nline2\nline3");

      await useSessionStore.getState().loadScrollback("abc");

      const session = useSessionStore.getState().sessions.get("abc");
      expect(session!.scrollbackText).toBe("line1\nline2\nline3");
    });

    it("does not reload if scrollback already loaded", async () => {
      useSessionStore.getState().addSession({
        id: "abc",
        name: "test",
        cwd: "/tmp",
        status: "exited",
        createdAt: 1715000000000,
        sessionType: "claude",
        isGitRepo: true,
        persisted: true,
        scrollbackText: "already loaded",
      });

      await useSessionStore.getState().loadScrollback("abc");

      expect(mockInvoke).not.toHaveBeenCalled();
    });
  });
});
```

- [ ] **Step 2: Run frontend tests**

Run: `npx vitest run`
Expected: All tests pass (existing + new).

- [ ] **Step 3: Commit**

```bash
git add src/__tests__/persistence.test.ts
git commit -m "test: add frontend persistence tests"
```

---

### Task 11: Full build and integration verification

**Files:** None (verification only)

- [ ] **Step 1: Run all backend tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass.

- [ ] **Step 2: Run all frontend tests**

Run: `npx vitest run`
Expected: All tests pass.

- [ ] **Step 3: Verify full build compiles**

Run: `npm run tauri build 2>&1 | tail -30`
Expected: Build succeeds.

- [ ] **Step 4: Final commit if any fixups needed**

```bash
git add -A && git commit -m "fix: resolve build issues from session persistence"
```
