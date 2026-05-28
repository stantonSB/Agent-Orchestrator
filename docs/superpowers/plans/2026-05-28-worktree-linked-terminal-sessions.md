# Worktree-Linked Terminal Sessions Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow users to create terminal sessions linked to an active Claude session's git worktree, with parent-child nesting in the sidebar and cascading close.

**Architecture:** The hook script sends `X-Cwd` header with every request so the status server learns each Claude session's worktree path. The frontend tracks `parentSessionId` and `worktreeCwd` on `SessionInfo` (frontend-only, not persisted). The modal shows a worktree dropdown when Terminal mode is selected. Child sessions render indented under parents and close when parents close.

**Tech Stack:** Rust (Tauri backend), React + Zustand (frontend), xterm.js, Vitest

---

## Chunk 1: Backend — Hook Script, Installer Content Check, StatusTracker, Status Server

### Task 1: Add `worktree_cwd` to StatusTracker

**Files:**
- Modify: `src-tauri/src/status_parser.rs:41-52`
- Modify: `src-tauri/src/status_parser_tests.rs` (for new tests)

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/status_parser_tests.rs`, add:

```rust
#[test]
fn test_worktree_cwd_default_none() {
    let tracker = StatusTracker::new();
    assert_eq!(tracker.worktree_cwd(), None);
}

#[test]
fn test_set_worktree_cwd() {
    let mut tracker = StatusTracker::new();
    let changed = tracker.set_worktree_cwd("/projects/app/.claude/worktrees/breezy-frog");
    assert!(changed);
    assert_eq!(tracker.worktree_cwd(), Some("/projects/app/.claude/worktrees/breezy-frog"));
}

#[test]
fn test_set_worktree_cwd_returns_false_if_already_set() {
    let mut tracker = StatusTracker::new();
    tracker.set_worktree_cwd("/projects/app/.claude/worktrees/breezy-frog");
    let changed = tracker.set_worktree_cwd("/projects/app/.claude/worktrees/breezy-frog");
    assert!(!changed);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test status_parser_tests::test_worktree_cwd -- --nocapture`
Expected: FAIL — `worktree_cwd` method doesn't exist

- [ ] **Step 3: Implement StatusTracker changes**

In `src-tauri/src/status_parser.rs`, add `worktree_cwd` field and methods to `StatusTracker`:

```rust
pub struct StatusTracker {
    status: SessionStatus,
    subagent_map: SubagentMap,
    worktree_cwd: Option<String>,
}

impl StatusTracker {
    pub fn new() -> Self {
        Self {
            status: SessionStatus::Starting,
            subagent_map: SubagentMap::new(),
            worktree_cwd: None,
        }
    }

    pub fn worktree_cwd(&self) -> Option<&str> {
        self.worktree_cwd.as_deref()
    }

    /// Set the worktree cwd. Returns true if this is the first time it was set
    /// (i.e., it changed from None to Some).
    pub fn set_worktree_cwd(&mut self, cwd: &str) -> bool {
        if self.worktree_cwd.is_some() {
            return false;
        }
        self.worktree_cwd = Some(cwd.to_string());
        true
    }
    // ... existing methods unchanged ...
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test status_parser_tests::test_worktree_cwd -- --nocapture`
Expected: PASS (all 3 tests)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/status_parser.rs src-tauri/src/status_parser_tests.rs
git commit -m "feat: add worktree_cwd field to StatusTracker"
```

---

### Task 2: Update hook script with X-Cwd header

**Files:**
- Modify: `src-tauri/src/hook_installer.rs:13-20` (HOOK_SCRIPT constant)

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/hook_installer.rs` tests section, add:

```rust
#[test]
fn test_hook_script_contains_x_cwd_header() {
    assert!(
        HOOK_SCRIPT.contains("X-Cwd"),
        "hook script should include X-Cwd header"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test test_hook_script_contains_x_cwd_header`
Expected: FAIL — current HOOK_SCRIPT doesn't contain "X-Cwd"

- [ ] **Step 3: Update the HOOK_SCRIPT constant**

In `src-tauri/src/hook_installer.rs`, replace the `HOOK_SCRIPT` constant:

```rust
const HOOK_SCRIPT: &str = r#"#!/bin/bash
# Forward Claude Code notifications to Agent Orchestrator.
# No-ops silently when Agent Orchestrator is not running.
if [ -n "$AO_STATUS_PORT" ] && [ -n "$AO_SESSION_ID" ]; then
    curl -s -X POST "http://127.0.0.1:${AO_STATUS_PORT}/status/${AO_SESSION_ID}" \
        -H "Content-Type: application/json" \
        -H "X-Cwd: $(pwd)" \
        -d @- 2>/dev/null || true
fi
"#;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test test_hook_script_contains_x_cwd_header`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/hook_installer.rs
git commit -m "feat: add X-Cwd header to hook script for worktree discovery"
```

---

### Task 3: Add script content check to `is_already_installed`

**Files:**
- Modify: `src-tauri/src/hook_installer.rs:68-101` (`is_already_installed` function and helpers)

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/hook_installer.rs` tests section, add:

```rust
#[test]
fn test_outdated_script_content_triggers_reinstall() {
    let home = temp_home();
    fs::create_dir_all(claude_dir(&home)).unwrap();

    // Write an old-format script (no X-Cwd header)
    let old_script = r#"#!/bin/bash
if [ -n "$AO_STATUS_PORT" ] && [ -n "$AO_SESSION_ID" ]; then
    curl -s -X POST "http://127.0.0.1:${AO_STATUS_PORT}/status/${AO_SESSION_ID}" \
        -H "Content-Type: application/json" -d @- 2>/dev/null || true
fi
"#;
    fs::write(script_path(&home), old_script).unwrap();
    let mut perms = fs::metadata(script_path(&home)).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(script_path(&home), perms).unwrap();

    // Install settings and profile so those checks pass
    merge_hook_settings(&settings_path(&home)).unwrap();
    set_idle_threshold(&profile_path(&home)).unwrap();

    // Should detect outdated script and reinstall
    let result = ensure_hooks_installed_in(home.path());
    assert_eq!(result, HookInstallResult::Installed);

    // Verify new script content
    let content = fs::read_to_string(script_path(&home)).unwrap();
    assert!(content.contains("X-Cwd"), "updated script should contain X-Cwd header");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test test_outdated_script_content_triggers_reinstall`
Expected: FAIL — `is_already_installed` returns true because it doesn't check content

- [ ] **Step 3: Add content check to is_already_installed**

In `src-tauri/src/hook_installer.rs`, find the `is_already_installed` function. Add a content check alongside the existing checks. The function calls `settings_has_our_hook`, `settings_has_our_stop_hook`, etc. Add a `script_has_current_content` helper:

```rust
fn script_has_current_content(script_path: &Path) -> bool {
    match fs::read_to_string(script_path) {
        Ok(content) => content == HOOK_SCRIPT,
        Err(_) => false,
    }
}
```

Then in `is_already_installed`, add `&& script_has_current_content(script_path)` to the check that verifies the script exists and is executable. The function should return `false` if the script content doesn't match `HOOK_SCRIPT`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test test_outdated_script_content_triggers_reinstall`
Expected: PASS

- [ ] **Step 5: Run all hook_installer tests**

Run: `cd src-tauri && cargo test hook_installer`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/hook_installer.rs
git commit -m "feat: check hook script content in is_already_installed for auto-upgrade"
```

---

### Task 4: Extract X-Cwd header in status server and emit callback

**Files:**
- Modify: `src-tauri/src/status_server.rs:22-28` (StatusServer::start signature)
- Modify: `src-tauri/src/status_server.rs:62-71` (accept_loop)
- Modify: `src-tauri/src/status_server.rs:97-234` (handle_request)
- Modify: `src-tauri/src/pty_manager.rs:156-159` (add callback type)
- Modify: `src-tauri/src/lib.rs:69-98` (wire up callback)

- [ ] **Step 1: Add `WorktreeCwdCallback` type to pty_manager.rs**

In `src-tauri/src/pty_manager.rs` after the existing callback type aliases (line ~159), add:

```rust
pub type WorktreeCwdCallback = Box<dyn Fn(SessionId, String) + Send + Sync + 'static>;
```

- [ ] **Step 2: Write the failing test**

In `src-tauri/src/status_server.rs` tests section, add:

```rust
#[test]
fn test_x_cwd_header_with_worktree_path_triggers_callback() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let trackers = make_trackers();
    trackers.lock().unwrap().insert("sess-wt".into(), StatusTracker::new());

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    let wt_cb: Arc<crate::pty_manager::WorktreeCwdCallback> =
        Arc::new(Box::new(move |_id: String, cwd: String| {
            if cwd.contains(".claude/worktrees/") {
                called_clone.store(true, Ordering::SeqCst);
            }
        }));

    let (server, port) = StatusServer::start(
        trackers,
        noop_callback(),
        noop_subagent_callback(),
        wt_cb,
    );

    let body = r#"{"session_id":"cc-1","notification_type":"idle_prompt"}"#;
    let request = format!(
        "POST /status/sess-wt HTTP/1.0\r\nContent-Length: {}\r\nContent-Type: application/json\r\nX-Cwd: /projects/app/.claude/worktrees/breezy-frog\r\n\r\n{}",
        body.len(),
        body
    );
    raw_http(port, &request);

    assert!(called.load(Ordering::SeqCst), "worktree cwd callback should have been called");

    server.stop();
}

#[test]
fn test_x_cwd_header_without_worktree_path_does_not_trigger_callback() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let trackers = make_trackers();
    trackers.lock().unwrap().insert("sess-no-wt".into(), StatusTracker::new());

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    let wt_cb: Arc<crate::pty_manager::WorktreeCwdCallback> =
        Arc::new(Box::new(move |_id: String, _cwd: String| {
            called_clone.store(true, Ordering::SeqCst);
        }));

    let (server, port) = StatusServer::start(
        trackers,
        noop_callback(),
        noop_subagent_callback(),
        wt_cb,
    );

    let body = r#"{"session_id":"cc-1","notification_type":"idle_prompt"}"#;
    let request = format!(
        "POST /status/sess-no-wt HTTP/1.0\r\nContent-Length: {}\r\nContent-Type: application/json\r\nX-Cwd: /projects/app\r\n\r\n{}",
        body.len(),
        body
    );
    raw_http(port, &request);

    assert!(!called.load(Ordering::SeqCst), "callback should NOT fire for non-worktree paths");

    server.stop();
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd src-tauri && cargo test test_x_cwd_header -- --nocapture`
Expected: FAIL — `StatusServer::start` doesn't accept 4th argument

- [ ] **Step 4: Update StatusServer::start to accept worktree cwd callback**

In `src-tauri/src/status_server.rs`:

Update `StatusServer::start` signature to accept the new callback:

```rust
pub fn start(
    trackers: Arc<Mutex<HashMap<String, StatusTracker>>>,
    on_status: Arc<StatusCallback>,
    on_subagents: Arc<SubagentCallback>,
    on_worktree_cwd: Arc<WorktreeCwdCallback>,
) -> (Self, u16) {
```

Update `accept_loop` signature and call to pass `on_worktree_cwd` through:

```rust
fn accept_loop(
    server: Arc<tiny_http::Server>,
    trackers: Arc<Mutex<HashMap<String, StatusTracker>>>,
    on_status: Arc<StatusCallback>,
    on_subagents: Arc<SubagentCallback>,
    on_worktree_cwd: Arc<WorktreeCwdCallback>,
) {
    for request in server.incoming_requests() {
        handle_request(request, &trackers, &on_status, &on_subagents, &on_worktree_cwd);
    }
}
```

Update `handle_request` signature:

```rust
fn handle_request(
    mut request: tiny_http::Request,
    trackers: &Arc<Mutex<HashMap<String, StatusTracker>>>,
    on_status: &Arc<StatusCallback>,
    on_subagents: &Arc<SubagentCallback>,
    on_worktree_cwd: &Arc<WorktreeCwdCallback>,
) {
```

In `handle_request`, after the JSON parsing and before the tracker lookup, extract the X-Cwd header:

```rust
    // Extract X-Cwd header for worktree path detection.
    let x_cwd: Option<String> = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("X-Cwd"))
        .map(|h| h.value.to_string());
```

After the tracker lookup succeeds (inside the `Some(tracker) =>` arm), before the transition logic, add worktree cwd handling:

```rust
    // Store worktree cwd if this is a worktree path
    if let Some(ref cwd) = x_cwd {
        if cwd.contains(".claude/worktrees/") {
            if tracker.set_worktree_cwd(cwd) {
                // First time seeing this worktree — notify frontend
                worktree_cwd_to_emit = Some((ao_session_id.clone(), cwd.clone()));
            }
        }
    }
```

Add `let mut worktree_cwd_to_emit: Option<(String, String)> = None;` before the tracker lock block. After the lock is released, emit:

```rust
    // Emit worktree cwd callback outside the lock
    if let Some((session_id, cwd)) = worktree_cwd_to_emit {
        on_worktree_cwd(session_id, cwd);
    }
```

- [ ] **Step 5: Fix all existing tests that call StatusServer::start**

All existing tests in `status_server.rs` call `StatusServer::start` with 3 args. Add a `noop_worktree_callback` helper and update all test calls:

```rust
fn noop_worktree_callback() -> Arc<crate::pty_manager::WorktreeCwdCallback> {
    Arc::new(Box::new(|_id: String, _cwd: String| {}))
}
```

Update every `StatusServer::start(trackers, noop_callback(), noop_subagent_callback())` to include the 4th arg: `StatusServer::start(trackers, noop_callback(), noop_subagent_callback(), noop_worktree_callback())`.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cd src-tauri && cargo test status_server`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/status_server.rs src-tauri/src/pty_manager.rs
git commit -m "feat: extract X-Cwd header in status server, emit worktree cwd callback"
```

---

### Task 5: Wire up worktree cwd callback in lib.rs

**Files:**
- Modify: `src-tauri/src/lib.rs:69-98`

- [ ] **Step 1: Add the callback and pass to StatusServer::start**

In `src-tauri/src/lib.rs`, after the `handle_for_subagents` clone (line 34), add:

```rust
let handle_for_worktree_cwd = app.handle().clone();
```

After the `on_subagents` callback definition (line ~82), add:

```rust
let on_worktree_cwd: pty_manager::WorktreeCwdCallback =
    Box::new(move |id, cwd| {
        let event_name = format!("session-worktree-cwd-{}", id);
        let _ = handle_for_worktree_cwd.emit(
            &event_name,
            serde_json::json!({ "worktreeCwd": cwd }),
        );
    });
```

Update the `StatusServer::start` call (line ~97-98) to pass the new callback:

```rust
let on_worktree_cwd_arc: Arc<pty_manager::WorktreeCwdCallback> = Arc::new(on_worktree_cwd);
let (status_server, status_port) =
    status_server::StatusServer::start(
        status_trackers.clone(),
        on_status_for_server,
        on_subagents_arc,
        on_worktree_cwd_arc,
    );
```

- [ ] **Step 2: Run full backend test suite**

Run: `cd src-tauri && cargo test`
Expected: All tests PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: wire worktree cwd callback to Tauri event emission"
```

---

## Chunk 2: Frontend — Data Model, Store, Event Listener

### Task 6: Add `parentSessionId` and `worktreeCwd` to SessionInfo type

**Files:**
- Modify: `src/types/session.ts:13-23`

- [ ] **Step 1: Add the fields**

In `src/types/session.ts`, add two optional fields to the `SessionInfo` interface:

```typescript
export interface SessionInfo {
  id: string;
  name: string;
  status: SessionStatus;
  createdAt: number; // unix timestamp ms
  cwd: string; // working directory path
  sessionType: "claude" | "terminal";
  isGitRepo: boolean;
  persisted?: boolean;
  scrollbackText?: string;
  parentSessionId?: string | null;
  worktreeCwd?: string | null;
}
```

- [ ] **Step 2: Run frontend tests to verify nothing breaks**

Run: `npx vitest run`
Expected: All tests PASS (new optional fields don't break existing code)

- [ ] **Step 3: Commit**

```bash
git add src/types/session.ts
git commit -m "feat: add parentSessionId and worktreeCwd to SessionInfo type"
```

---

### Task 7: Update session store — createSession signature, worktree event listener, cascading close

**Files:**
- Modify: `src/stores/sessionStore.ts`
- Modify: `src/stores/sessionStore.test.ts`

- [ ] **Step 1: Write failing tests for the new behavior**

In `src/stores/sessionStore.test.ts`, add these test cases:

```typescript
describe("worktree-linked terminal sessions", () => {
  it("creates a terminal session with parentSessionId", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockResolvedValueOnce("child-terminal-id");

    const store = useSessionStore.getState();
    await store.createSession(
      "Test Terminal",
      "/projects/app/.claude/worktrees/breezy-frog",
      "terminal",
      false,
      false,
      "parent-claude-id"
    );

    const session = useSessionStore.getState().sessions.get("child-terminal-id");
    expect(session?.parentSessionId).toBe("parent-claude-id");
    expect(session?.sessionType).toBe("terminal");
  });

  it("creates a session without parentSessionId by default", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockResolvedValueOnce("regular-id");

    const store = useSessionStore.getState();
    await store.createSession("Regular Session", "/projects/app", "claude");

    const session = useSessionStore.getState().sessions.get("regular-id");
    expect(session?.parentSessionId).toBeUndefined();
  });

  it("cascading close removes children before parent", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockResolvedValue(undefined);

    const store = useSessionStore.getState();
    store.addSession({
      id: "parent-1",
      name: "Claude Parent",
      status: "working",
      createdAt: Date.now(),
      cwd: "/projects/app",
      sessionType: "claude",
      isGitRepo: true,
    });
    store.addSession({
      id: "child-1",
      name: "Terminal Child",
      status: "terminal",
      createdAt: Date.now(),
      cwd: "/projects/app/.claude/worktrees/breezy-frog",
      sessionType: "terminal",
      isGitRepo: false,
      parentSessionId: "parent-1",
    });
    store.addSession({
      id: "child-2",
      name: "Terminal Child 2",
      status: "terminal",
      createdAt: Date.now(),
      cwd: "/projects/app/.claude/worktrees/breezy-frog",
      sessionType: "terminal",
      isGitRepo: false,
      parentSessionId: "parent-1",
    });

    await store.closeSession("parent-1");

    const { sessions } = useSessionStore.getState();
    expect(sessions.has("parent-1")).toBe(false);
    expect(sessions.has("child-1")).toBe(false);
    expect(sessions.has("child-2")).toBe(false);

    // close_session should have been called for children and parent
    expect(invoke).toHaveBeenCalledWith("close_session", { id: "child-1" });
    expect(invoke).toHaveBeenCalledWith("close_session", { id: "child-2" });
    expect(invoke).toHaveBeenCalledWith("close_session", { id: "parent-1" });
  });

  it("updates worktreeCwd when event is received", () => {
    const store = useSessionStore.getState();
    store.addSession({
      id: "claude-1",
      name: "Claude Session",
      status: "working",
      createdAt: Date.now(),
      cwd: "/projects/app",
      sessionType: "claude",
      isGitRepo: true,
    });

    // Simulate the worktree cwd update (same as event handler would do)
    store.updateWorktreeCwd("claude-1", "/projects/app/.claude/worktrees/breezy-frog");

    const session = useSessionStore.getState().sessions.get("claude-1");
    expect(session?.worktreeCwd).toBe("/projects/app/.claude/worktrees/breezy-frog");
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npx vitest run src/stores/sessionStore.test.ts`
Expected: FAIL — `updateWorktreeCwd` doesn't exist, `parentSessionId` not set

- [ ] **Step 3: Implement store changes**

In `src/stores/sessionStore.ts`:

**a) Add `updateWorktreeCwd` to the interface** (after `updateSessionStatus`):

```typescript
updateWorktreeCwd: (id: string, worktreeCwd: string) => void;
```

**b) Add `parentSessionId` parameter to `createSession`** in the interface:

```typescript
createSession: (name: string, cwd: string, sessionMode?: SessionMode, pullLatest?: boolean, isGitRepo?: boolean, parentSessionId?: string) => Promise<void>;
```

**c) Implement `updateWorktreeCwd`** (after the `updateSessionStatus` implementation):

```typescript
updateWorktreeCwd: (id, worktreeCwd) =>
  set((state) => {
    const session = state.sessions.get(id);
    if (!session) return state;
    const next = new Map(state.sessions);
    next.set(id, { ...session, worktreeCwd });
    return { sessions: next };
  }),
```

**d) Update `createSession` implementation** to accept and use `parentSessionId`:

Change the function signature to:

```typescript
createSession: async (name, cwd, sessionMode = "claude", pullLatest = false, isGitRepo = true, parentSessionId?) => {
```

In the terminal session branch, include `parentSessionId` in the session object:

```typescript
session = {
  id,
  name,
  status: "terminal",
  createdAt: Date.now(),
  cwd,
  sessionType: "terminal",
  isGitRepo: false,
  ...(parentSessionId ? { parentSessionId } : {}),
};
```

**e) Update `closeSession` for cascading close:**

Replace the `closeSession` implementation:

```typescript
closeSession: async (id) => {
  const state = get();
  const session = state.sessions.get(id);

  // Find child sessions to cascade-close
  const children = Array.from(state.sessions.values()).filter(
    (s) => s.parentSessionId === id
  );

  if (session?.persisted) {
    try {
      await invoke("delete_persisted_session", { sessionId: id });
    } catch (err) {
      console.error("Failed to delete persisted session:", err);
    }
    // Remove parent and children atomically
    set((s) => {
      const next = new Map(s.sessions);
      next.delete(id);
      for (const child of children) next.delete(child.id);
      const nextSubagents = new Map(s.subagents);
      nextSubagents.delete(id);
      for (const child of children) nextSubagents.delete(child.id);
      let activeSessionId = s.activeSessionId;
      if (activeSessionId === id || children.some((c) => c.id === activeSessionId)) {
        const remaining = Array.from(next.keys());
        activeSessionId = remaining.length > 0 ? remaining[0] : null;
      }
      return { sessions: next, subagents: nextSubagents, activeSessionId };
    });
    return;
  }

  // Cancel subagent cleanup timers for parent and children
  cancelSubagentCleanup(id);
  for (const child of children) cancelSubagentCleanup(child.id);

  // Close children in parallel first
  await Promise.all(
    children.map((child) =>
      invoke("close_session", { id: child.id }).catch((err: unknown) =>
        console.error(`Failed to close child session ${child.id}:`, err)
      )
    )
  );

  // Close parent
  await invoke("close_session", { id });

  // Remove parent and all children atomically
  set((s) => {
    const next = new Map(s.sessions);
    next.delete(id);
    for (const child of children) next.delete(child.id);
    const nextSubagents = new Map(s.subagents);
    nextSubagents.delete(id);
    for (const child of children) nextSubagents.delete(child.id);

    const cleanups = eventCleanups.get(id);
    if (cleanups) {
      cleanups.forEach((unlisten) => unlisten());
      eventCleanups.delete(id);
    }
    for (const child of children) {
      const childCleanups = eventCleanups.get(child.id);
      if (childCleanups) {
        childCleanups.forEach((unlisten) => unlisten());
        eventCleanups.delete(child.id);
      }
    }

    let activeSessionId = s.activeSessionId;
    if (activeSessionId === id || children.some((c) => c.id === activeSessionId)) {
      const remaining = Array.from(next.keys());
      activeSessionId = remaining.length > 0 ? remaining[0] : null;
    }

    return { sessions: next, subagents: nextSubagents, activeSessionId };
  });
},
```

**f) Add worktree cwd event listener in `setupEventListeners`:**

After the subagent listener block (line ~421), add:

```typescript
// Listen for worktree cwd events on Claude sessions
if (session?.sessionType !== "terminal") {
  cleanups.push(
    listen<{ worktreeCwd: string }>(`session-worktree-cwd-${sessionId}`, (event) => {
      get().updateWorktreeCwd(sessionId, event.payload.worktreeCwd);
    })
  );
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npx vitest run src/stores/sessionStore.test.ts`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/stores/sessionStore.ts src/stores/sessionStore.test.ts
git commit -m "feat: add worktree cwd tracking and cascading close to session store"
```

---

## Chunk 3: Frontend — Modal Worktree Dropdown

### Task 8: Add worktree dropdown to NewSessionModal

**Files:**
- Modify: `src/components/NewSessionModal/NewSessionModal.tsx`
- Modify: `src/components/NewSessionModal/NewSessionModal.module.css`
- Modify: `src/App.tsx:118-128` (handleCreateSession)

- [ ] **Step 1: Update the `onCreate` callback type to include `parentSessionId`**

In `src/components/NewSessionModal/NewSessionModal.tsx`, update the `NewSessionModalProps` interface:

```typescript
interface NewSessionModalProps {
  isOpen: boolean;
  onClose: () => void;
  onCreate: (name: string, cwd: string, sessionMode: SessionMode, pullLatest: boolean, isGitRepo: boolean, parentSessionId?: string) => void;
  lastUsedDirectory: string | null;
}
```

- [ ] **Step 2: Add worktree dropdown state and logic**

In the `NewSessionModal` component, import `useSessionStore`:

```typescript
import { useSessionStore } from "../../stores/sessionStore";
```

Add state for selected worktree:

```typescript
const [selectedWorktreeSessionId, setSelectedWorktreeSessionId] = useState<string | null>(null);
```

Add a derived list of available worktree sessions:

```typescript
const sessions = useSessionStore((s) => s.sessions);
const worktreeSessions = useMemo(() => {
  if (sessionMode !== "terminal") return [];
  return Array.from(sessions.values()).filter(
    (s) =>
      s.sessionType === "claude" &&
      s.worktreeCwd &&
      s.status !== "finished" &&
      s.status !== "exited" &&
      s.status !== "error"
  );
}, [sessions, sessionMode]);
```

Add `useMemo` to the imports from react.

Reset `selectedWorktreeSessionId` when `isOpen` changes or `sessionMode` changes (add to the existing `useEffect` for `isOpen`, and add a new effect for mode change):

```typescript
// In the isOpen useEffect:
setSelectedWorktreeSessionId(null);

// New effect for mode change:
useEffect(() => {
  setSelectedWorktreeSessionId(null);
}, [sessionMode]);
```

- [ ] **Step 3: Compute effective directory based on worktree selection**

```typescript
const selectedWorktreeSession = selectedWorktreeSessionId
  ? sessions.get(selectedWorktreeSessionId)
  : null;
const effectiveDirectory = selectedWorktreeSession?.worktreeCwd ?? directory;
```

Update `isValid`:

```typescript
const isValid = effectiveDirectory !== null;
```

- [ ] **Step 4: Update `handleCreate` to pass parentSessionId and use effective directory**

```typescript
const handleCreate = () => {
  const trimmedName = name.trim();
  const finalName = trimmedName || getDefaultSessionName(getNextSessionNumber());
  if (!effectiveDirectory) return;
  localStorage.setItem(STORAGE_KEY, sessionMode);
  if (directory) localStorage.setItem(DIR_STORAGE_KEY, directory);
  onCreate(
    finalName,
    effectiveDirectory,
    sessionMode,
    effectivePullLatest,
    isGitRepo ?? false,
    selectedWorktreeSessionId ?? undefined
  );
};
```

- [ ] **Step 5: Update handleKeyDown to use effectiveDirectory**

In `NewSessionModal.tsx`, the existing `handleKeyDown` function checks `directory` for Enter key submission. Update it to use `effectiveDirectory`:

```typescript
const handleKeyDown = (e: React.KeyboardEvent) => {
  if (e.key === "Escape") {
    onClose();
  }
  if (e.key === "Enter" && effectiveDirectory) {
    handleCreate();
  }
};
```

- [ ] **Step 6: Add the worktree dropdown JSX**

After the session mode `</div>` (line ~188) and before the checkbox row, add:

```tsx
{sessionMode === "terminal" && worktreeSessions.length > 0 && (
  <div className={styles.field}>
    <label className={styles.label} htmlFor="worktree-select">
      Worktree
    </label>
    <select
      id="worktree-select"
      className={styles.select}
      value={selectedWorktreeSessionId ?? ""}
      onChange={(e) => setSelectedWorktreeSessionId(e.target.value || null)}
    >
      <option value="">None</option>
      {worktreeSessions.map((s) => (
        <option key={s.id} value={s.id}>
          {s.name}
        </option>
      ))}
    </select>
  </div>
)}
```

- [ ] **Step 7: Disable directory picker when worktree is selected**

Update the Project Directory field to show the worktree path and disable browse when a worktree is selected:

```tsx
<div className={styles.field}>
  <label className={styles.label}>Project Directory</label>
  <div className={styles.folderRow}>
    <div
      className={`${styles.folderPath} ${effectiveDirectory ? styles.hasValue : ""} ${selectedWorktreeSessionId ? styles.disabled : ""}`}
      title={effectiveDirectory ?? undefined}
    >
      {effectiveDirectory ?? "No directory selected"}
    </div>
    <button
      className={styles.browseButton}
      onClick={handleBrowse}
      type="button"
      disabled={!!selectedWorktreeSessionId}
    >
      Browse
    </button>
  </div>
</div>
```

- [ ] **Step 8: Add `.disabled` CSS class**

In `src/components/NewSessionModal/NewSessionModal.module.css`, add:

```css
.folderPath.disabled {
  opacity: 0.5;
}

.browseButton:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
```

- [ ] **Step 9: Update App.tsx to pass parentSessionId through**

In `src/App.tsx`, update `handleCreateSession`:

```typescript
const handleCreateSession = async (name: string, cwd: string, sessionMode: SessionMode, pullLatest: boolean, isGitRepo: boolean, parentSessionId?: string) => {
  try {
    setIsModalOpen(false);
    await createSession(name, cwd, sessionMode, pullLatest, isGitRepo, parentSessionId);
  } catch (err) {
    addToast(
      `Failed to create session: ${err instanceof Error ? err.message : String(err)}`,
      "error"
    );
  }
};
```

- [ ] **Step 10: Run frontend tests**

Run: `npx vitest run`
Expected: All tests PASS

- [ ] **Step 11: Commit**

```bash
git add src/components/NewSessionModal/NewSessionModal.tsx src/components/NewSessionModal/NewSessionModal.module.css src/App.tsx
git commit -m "feat: add worktree dropdown to new session modal for terminal sessions"
```

---

## Chunk 4: Frontend — Sidebar Nesting and Cascading Close Visual

### Task 9: Filter child sessions before grouping in SessionPanel

**Files:**
- Modify: `src/components/SessionPanel/SessionPanel.tsx:62-116`
- Modify: `src/components/SessionPanel/SessionPanel.test.tsx`

- [ ] **Step 1: Write the failing test**

In `src/components/SessionPanel/SessionPanel.test.tsx`, add:

```typescript
it("does not create separate project group for child terminal sessions", () => {
  const sessions: SessionInfo[] = [
    {
      id: "parent-1",
      name: "Claude Parent",
      status: "working",
      createdAt: 1000,
      cwd: "/projects/app",
      sessionType: "claude",
      isGitRepo: true,
    },
    {
      id: "child-1",
      name: "Terminal Child",
      status: "terminal",
      createdAt: 2000,
      cwd: "/projects/app/.claude/worktrees/breezy-frog",
      sessionType: "terminal",
      isGitRepo: false,
      parentSessionId: "parent-1",
    },
  ];
  render(
    <SessionPanel
      sessions={sessions}
      activeSessionId="parent-1"
      onSessionClick={vi.fn()}
      onNewSession={vi.fn()}
    />
  );
  // Should only show the "app" group, not "breezy-frog"
  expect(screen.getByText("app")).toBeTruthy();
  expect(screen.queryByText("breezy-frog")).toBeNull();
  // Child should still be visible (rendered under parent)
  expect(screen.getByText("Terminal Child")).toBeTruthy();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/components/SessionPanel/SessionPanel.test.tsx`
Expected: FAIL — "breezy-frog" group header appears

- [ ] **Step 3: Filter child sessions and pass children map to ProjectGroup**

In `src/components/SessionPanel/SessionPanel.tsx`:

Update the `projectGroups` memo to filter out child sessions:

```typescript
const projectGroups = useMemo(() => {
  const topLevel = sessions.filter((s) => !s.parentSessionId);
  return groupSessionsByProject(topLevel);
}, [sessions]);
```

Build a children-by-parent map and pass `allSessions` to `ProjectGroup`:

```typescript
const childrenByParent = useMemo(() => {
  const map = new Map<string, SessionInfo[]>();
  for (const s of sessions) {
    if (s.parentSessionId) {
      const existing = map.get(s.parentSessionId) ?? [];
      existing.push(s);
      map.set(s.parentSessionId, existing);
    }
  }
  return map;
}, [sessions]);
```

Pass `childrenByParent` to `ProjectGroup`:

```tsx
<ProjectGroup
  key={group.cwd}
  projectName={group.displayName}
  sessions={group.sessions}
  activeSessionId={activeSessionId}
  isCollapsed={collapsedGroups.has(group.cwd)}
  onToggleCollapse={() => toggleCollapse(group.cwd)}
  onSessionClick={onSessionClick}
  onClose={closeSession}
  onDismiss={dismissSession}
  onRename={renameSession}
  subagentsBySession={subagents}
  childrenByParent={childrenByParent}
/>
```

- [ ] **Step 4: Update ProjectGroup to render child sessions**

In `src/components/ProjectGroup/ProjectGroup.tsx`:

Add `childrenByParent` to the props interface:

```typescript
interface ProjectGroupProps {
  projectName: string;
  sessions: SessionInfo[];
  activeSessionId: string | null;
  isCollapsed: boolean;
  onToggleCollapse: () => void;
  onSessionClick: (id: string) => void;
  onClose: (id: string) => Promise<void>;
  onDismiss: (id: string) => void;
  onRename?: (id: string, name: string) => void;
  subagentsBySession: Map<string, SubagentStatus[]>;
  childrenByParent?: Map<string, SessionInfo[]>;
}
```

In the render, after each `SessionCard` + `SubagentList`, render children:

```tsx
{!isCollapsed && (
  <div className={styles.sessions}>
    {sessions.map((session) => (
      <div key={session.id}>
        <SessionCard
          session={session}
          isActive={session.id === activeSessionId}
          onClick={onSessionClick}
          onClose={onClose}
          onDismiss={onDismiss}
          onRename={onRename}
        />
        <SubagentList subagents={subagentsBySession.get(session.id) ?? []} />
        {childrenByParent?.get(session.id)?.map((child) => (
          <div key={child.id} className={styles.childSession}>
            <SessionCard
              session={child}
              isActive={child.id === activeSessionId}
              onClick={onSessionClick}
              onClose={onClose}
              onDismiss={onDismiss}
              onRename={onRename}
            />
          </div>
        ))}
      </div>
    ))}
  </div>
)}
```

- [ ] **Step 5: Add CSS for child session indentation**

In `src/components/ProjectGroup/ProjectGroup.module.css`, add:

```css
.childSession {
  padding-left: 20px;
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `npx vitest run src/components/SessionPanel/SessionPanel.test.tsx`
Expected: All tests PASS

- [ ] **Step 7: Run full frontend test suite**

Run: `npx vitest run`
Expected: All tests PASS

- [ ] **Step 8: Commit**

```bash
git add src/components/SessionPanel/SessionPanel.tsx src/components/SessionPanel/SessionPanel.test.tsx src/components/ProjectGroup/ProjectGroup.tsx src/components/ProjectGroup/ProjectGroup.module.css
git commit -m "feat: nest child terminal sessions under parent in sidebar"
```

---

## Chunk 5: Integration Verification

### Task 10: Run all tests and verify builds

**Files:** None (verification only)

- [ ] **Step 1: Run backend tests**

Run: `cd src-tauri && cargo test`
Expected: All tests PASS

- [ ] **Step 2: Run frontend tests**

Run: `npx vitest run`
Expected: All tests PASS

- [ ] **Step 3: Verify Rust compiles cleanly**

Run: `cd src-tauri && cargo build`
Expected: Compiles with no errors

- [ ] **Step 4: Verify frontend builds**

Run: `npm run build`
Expected: Builds with no errors (note: this is the Vite frontend build, not `tauri build`)

- [ ] **Step 5: Final commit if any fixes were needed**

Only if fixes were needed during verification:

```bash
git add -A
git commit -m "fix: address integration issues from worktree-linked sessions"
```
