# Plain Terminal Sessions Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow users to open plain terminal sessions ($SHELL) alongside Claude sessions via a new "Initialise with Claude" checkbox in the New Session modal.

**Architecture:** Frontend-only toggle approach. The backend already supports custom commands; we thread a `session_type` field through the IPC boundary and PTY manager. Terminal sessions skip status tracking, hook env vars, and startup timers. A new `"terminal"` status variant renders as a static grey dot.

**Tech Stack:** Rust (Tauri backend), React + TypeScript + Zustand (frontend), Vitest (frontend tests), Cargo test (backend tests)

**Spec:** `docs/superpowers/specs/2026-04-24-plain-terminal-sessions-design.md`

---

## Chunk 1: Backend — SessionType enum and PTY manager changes

### Task 1: Add SessionType enum and thread through PtyRequest::Create

**Files:**
- Modify: `src-tauri/src/pty_manager.rs:59-67` (PtyRequest::Create variant)
- Modify: `src-tauri/src/pty_manager.rs:107-114` (SessionListEntry)
- Modify: `src-tauri/src/pty_manager.rs:125-136` (Session struct)
- Modify: `src-tauri/src/pty_manager.rs:159-177` (PtyManagerHandle::create)

- [ ] **Step 1: Define SessionType enum**

Add above the `PtyRequest` enum in `pty_manager.rs`:

```rust
/// Whether this session runs Claude Code or a plain shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    Claude,
    Terminal,
}

impl SessionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionType::Claude => "claude",
            SessionType::Terminal => "terminal",
        }
    }
}
```

- [ ] **Step 2: Add session_type to PtyRequest::Create**

Add `session_type: SessionType,` field to the `PtyRequest::Create` variant, after `args`.

- [ ] **Step 3: Add session_type to Session struct**

Add `session_type: SessionType,` field to the internal `Session` struct.

- [ ] **Step 4: Add session_type to SessionListEntry**

Add `pub session_type: String,` field to `SessionListEntry`.

- [ ] **Step 5: Update PtyManagerHandle::create signature**

Add `session_type: SessionType` parameter and forward it in the request:

```rust
pub fn create(
    &self,
    name: String,
    cwd: PathBuf,
    command: String,
    args: Vec<String>,
    cols: u16,
    rows: u16,
    session_type: SessionType,
) -> PtyResponse {
    self.request(|reply| PtyRequest::Create {
        name,
        cwd,
        command,
        args,
        cols,
        rows,
        session_type,
        reply,
    })
}
```

- [ ] **Step 6: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | head -30`

Expected: Compilation errors in `manager_loop` and tests (they don't destructure/pass the new field yet). That's expected — we fix them in the next tasks.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/pty_manager.rs
git commit -m "feat: add SessionType enum and thread through PTY request types"
```

### Task 2: Conditionally skip status tracking for terminal sessions

**Files:**
- Modify: `src-tauri/src/pty_manager.rs:250-441` (Create handler in manager_loop)
- Modify: `src-tauri/src/pty_manager.rs:529-539` (List handler in manager_loop)

- [ ] **Step 1: Update Create handler destructuring**

In the `PtyRequest::Create` match arm (~line 250), add `session_type` to the destructured fields.

- [ ] **Step 2: Conditionally set hook env vars**

Replace the unconditional `AO_SESSION_ID` and `AO_STATUS_PORT` lines (299-300) with:

```rust
if session_type == SessionType::Claude {
    cmd.env("AO_SESSION_ID", &id);
    cmd.env("AO_STATUS_PORT", status_port.to_string());
}
```

- [ ] **Step 3: Conditionally create StatusTracker**

Replace the unconditional tracker insert (332-335) with:

```rust
if session_type == SessionType::Claude {
    let mut trackers = status_trackers.lock().unwrap();
    trackers.insert(id.clone(), StatusTracker::new());
}
```

- [ ] **Step 4: Conditionally spawn startup timer**

Wrap the startup timer block (392-420) with:

```rust
if session_type == SessionType::Claude {
    // ... existing timer code ...
}
```

- [ ] **Step 5: Store session_type on Session struct**

In the `sessions.insert` call (~line 427), add `session_type,` to the Session struct literal.

- [ ] **Step 6: Include session_type in List response**

In the `PtyRequest::List` handler (~line 529), add to the `SessionListEntry` construction:

```rust
session_type: s.session_type.as_str().to_string(),
```

- [ ] **Step 7: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | head -30`

Expected: Errors only in tests (they don't pass `session_type` to `handle.create()`). Main code should compile.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/pty_manager.rs
git commit -m "feat: conditionally skip status tracking for terminal sessions"
```

### Task 3: Update commands.rs to accept and forward session_type

**Files:**
- Modify: `src-tauri/src/commands.rs:10-27` (SessionInfo struct and From impl)
- Modify: `src-tauri/src/commands.rs:29-56` (create_session command)

- [ ] **Step 1: Add session_type to commands::SessionInfo**

Add `pub session_type: String,` field to the `SessionInfo` struct in `commands.rs`.

- [ ] **Step 2: Update From<SessionListEntry> impl**

Add `session_type: e.session_type,` to the `From` impl.

- [ ] **Step 3: Add session_type parameter to create_session**

Add `session_type: Option<String>,` parameter to the `create_session` function. Parse it into `SessionType`:

```rust
let session_type = match session_type.as_deref() {
    Some("terminal") => crate::pty_manager::SessionType::Terminal,
    _ => crate::pty_manager::SessionType::Claude,
};
```

- [ ] **Step 4: Forward session_type to pty.create()**

Update the `state.pty.create()` call to pass `session_type`:

```rust
match state.pty.create(name, path, command, args, cols, rows, session_type) {
```

- [ ] **Step 5: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | head -30`

Expected: Errors only in PTY manager tests. Commands should compile.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: accept session_type in create_session IPC command"
```

### Task 4: Fix backend tests

**Files:**
- Modify: `src-tauri/src/pty_manager.rs:568-911` (tests module)

- [ ] **Step 1: Update test_manager helper**

No changes needed to `test_manager()` — it doesn't call `create()`.

- [ ] **Step 2: Update all handle.create() calls in tests**

Every test that calls `handle.create()` needs the new `session_type` parameter. Add `SessionType::Claude` as the last argument to all existing calls. There are 8 tests that call `handle.create()`:

- `test_create_and_list`
- `test_output_received`
- `test_exit_callback`
- `test_write_to_session`
- `test_resize`
- `test_rename_session`
- `test_kill_session`
- `test_nonzero_exit_code`
- `test_shutdown_kills_all_sessions` (3 calls in a loop)

For each, change e.g.:
```rust
handle.create("test-session".into(), std::env::temp_dir(), "echo".into(), vec!["hello".into()], 80, 24)
```
to:
```rust
handle.create("test-session".into(), std::env::temp_dir(), "echo".into(), vec!["hello".into()], 80, 24, SessionType::Claude)
```

- [ ] **Step 3: Add test for terminal session (no tracker created)**

```rust
#[test]
fn test_terminal_session_no_tracker() {
    let status_trackers = Arc::new(Mutex::new(HashMap::new()));
    let status_trackers_clone = status_trackers.clone();

    let handle = start(
        Box::new(|_id, _data| {}),
        Box::new(|_id, _code| {}),
        Box::new(|_id, _status| {}),
        status_trackers_clone,
        0,
    );

    let resp = handle.create(
        "terminal-test".into(),
        std::env::temp_dir(),
        "echo".into(),
        vec!["hello".into()],
        80,
        24,
        SessionType::Terminal,
    );
    let id = match resp {
        PtyResponse::Created { id } => id,
        other => panic!("Expected Created, got: {:?}", other),
    };

    // Terminal sessions should NOT have a status tracker
    let trackers = status_trackers.lock().unwrap();
    assert!(
        !trackers.contains_key(&id),
        "Terminal session should not have a status tracker"
    );

    drop(trackers);
    handle.shutdown();
}
```

- [ ] **Step 4: Add test for terminal session listed with correct type**

```rust
#[test]
fn test_terminal_session_list_type() {
    let (handle, _output, _exit) = test_manager();
    let resp = handle.create(
        "terminal-list".into(),
        std::env::temp_dir(),
        "cat".into(),
        vec![],
        80,
        24,
        SessionType::Terminal,
    );
    let id = match resp {
        PtyResponse::Created { id } => id,
        other => panic!("Expected Created, got: {:?}", other),
    };

    let resp = handle.list();
    match resp {
        PtyResponse::Sessions(entries) => {
            let entry = entries.iter().find(|e| e.id == id).unwrap();
            assert_eq!(entry.session_type, "terminal");
        }
        other => panic!("Expected Sessions, got: {:?}", other),
    }
    handle.shutdown();
}
```

- [ ] **Step 5: Run backend tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -20`

Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/pty_manager.rs
git commit -m "test: update PTY tests for session_type, add terminal session tests"
```

## Chunk 2: Frontend — Types, store, modal, and session card

### Task 5: Update TypeScript types

**Files:**
- Modify: `src/types/session.ts`

- [ ] **Step 1: Add "terminal" to SessionStatus**

```typescript
export type SessionStatus =
  | "starting"
  | "working"
  | "idle"
  | "needs_attention"
  | "finished"
  | "error"
  | "terminal";
```

- [ ] **Step 2: Add sessionType to SessionInfo**

```typescript
export interface SessionInfo {
  id: string;
  name: string;
  status: SessionStatus;
  createdAt: number;
  cwd: string;
  sessionType: "claude" | "terminal";
}
```

- [ ] **Step 3: Commit**

```bash
git add src/types/session.ts
git commit -m "feat: add terminal status and sessionType to TypeScript types"
```

### Task 6: Update session store

**Files:**
- Modify: `src/stores/sessionStore.ts:25` (createSession signature in interface)
- Modify: `src/stores/sessionStore.ts:183-209` (createSession implementation)
- Modify: `src/stores/sessionStore.ts:227-260` (setupEventListeners)

- [ ] **Step 1: Write failing test for terminal session creation**

Add to `src/stores/sessionStore.test.ts` in the `createSession` describe block:

```typescript
it("creates a terminal session when initWithClaude is false", async () => {
  const { invoke } = await import("@tauri-apps/api/core");
  vi.mocked(invoke).mockResolvedValueOnce("terminal-id-1");

  const store = useSessionStore.getState();
  await store.createSession("My Terminal", "/path/to/project", true, false, false);

  expect(invoke).toHaveBeenCalledWith("create_session", {
    name: "My Terminal",
    cwd: "/path/to/project",
    sessionType: "terminal",
  });

  const session = useSessionStore.getState().sessions.get("terminal-id-1");
  expect(session?.sessionType).toBe("terminal");
  expect(session?.status).toBe("terminal");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/stores/sessionStore.test.ts 2>&1 | tail -20`

Expected: FAIL — `createSession` doesn't accept `initWithClaude` parameter yet.

- [ ] **Step 3: Update createSession interface and implementation**

Update the interface type:

```typescript
createSession: (name: string, cwd: string, skipPermissions?: boolean, pullLatest?: boolean, initWithClaude?: boolean) => Promise<void>;
```

Update the implementation:

```typescript
createSession: async (name, cwd, skipPermissions = true, pullLatest = false, initWithClaude = true) => {
  if (pullLatest) {
    await invoke("git_pull_main", { cwd });
  }

  let id: string;
  let session: SessionInfo;

  if (initWithClaude) {
    const args = ["--worktree"];
    if (skipPermissions) {
      args.unshift("--dangerously-skip-permissions");
    }
    id = await invoke<string>("create_session", {
      name,
      cwd,
      command: "claude",
      args,
      sessionType: "claude",
    });
    session = {
      id,
      name,
      status: "starting",
      createdAt: Date.now(),
      cwd,
      sessionType: "claude",
    };
  } else {
    id = await invoke<string>("create_session", {
      name,
      cwd,
      sessionType: "terminal",
    });
    session = {
      id,
      name,
      status: "terminal",
      createdAt: Date.now(),
      cwd,
      sessionType: "terminal",
    };
  }

  get().addSession(session);
  get().setActiveSession(id);
  get().setupEventListeners(id);
  set({ lastUsedDirectory: cwd });
},
```

- [ ] **Step 4: Update setupEventListeners to skip status listener for terminals**

The `setupEventListeners` method needs to know the session type. Update it to check the session's type from the store:

```typescript
setupEventListeners: (sessionId) => {
  let cancelled = false;
  const cleanups: Promise<UnlistenFn>[] = [];
  const session = get().sessions.get(sessionId);

  // Only listen for status events on Claude sessions
  if (session?.sessionType !== "terminal") {
    cleanups.push(
      listen<{ status: SessionStatus }>(`session-status-${sessionId}`, (event) => {
        get().updateSessionStatus(sessionId, event.payload.status);
      })
    );
  }

  cleanups.push(
    listen<{ code: number | null }>(`session-exit-${sessionId}`, (event) => {
      const session = get().sessions.get(sessionId);
      if (session?.sessionType === "terminal") {
        // Terminal sessions only show error on non-zero exit
        if (event.payload.code !== null && event.payload.code !== 0) {
          get().updateSessionStatus(sessionId, "error");
        }
      } else {
        const status: SessionStatus = event.payload.code === 0 ? "finished" : "error";
        get().updateSessionStatus(sessionId, status);
      }
    })
  );

  // Only listen for subagent events on Claude sessions
  if (session?.sessionType !== "terminal") {
    cleanups.push(
      listen<SubagentStatus[]>(`session-subagents-${sessionId}`, (event) => {
        get().updateSubagents(sessionId, event.payload);
      })
    );
  }

  eventCleanups.set(sessionId, [() => { cancelled = true; }]);

  Promise.all(cleanups).then((unlistenFns) => {
    if (cancelled) {
      unlistenFns.forEach((unlisten) => unlisten());
      return;
    }
    eventCleanups.set(sessionId, unlistenFns);
  });
},
```

- [ ] **Step 5: Run test to verify it passes**

Run: `npx vitest run src/stores/sessionStore.test.ts 2>&1 | tail -20`

Expected: PASS

- [ ] **Step 6: Update existing tests for sessionType field**

All existing tests that create `SessionInfo` objects need the `sessionType` field. Add `sessionType: "claude"` to every inline `SessionInfo` in `sessionStore.test.ts`. There are ~10 places — every `addSession({...})` call and the `createSession` assertion.

Also update the existing `createSession` test assertion to include `sessionType: "claude"` in the expected invoke args:

```typescript
expect(invoke).toHaveBeenCalledWith("create_session", {
  name: "My Session",
  cwd: "/path/to/project",
  command: "claude",
  args: ["--dangerously-skip-permissions", "--worktree"],
  sessionType: "claude",
});
```

- [ ] **Step 7: Run all tests to verify**

Run: `npx vitest run src/stores/sessionStore.test.ts 2>&1 | tail -20`

Expected: All tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/stores/sessionStore.ts src/stores/sessionStore.test.ts src/types/session.ts
git commit -m "feat: support terminal session creation in store"
```

### Task 7: Update NewSessionModal

**Files:**
- Modify: `src/components/NewSessionModal/NewSessionModal.tsx`
- Modify: `src/components/NewSessionModal/NewSessionModal.module.css`

- [ ] **Step 1: Update onCreate prop type**

Change the `NewSessionModalProps` interface:

```typescript
onCreate: (name: string, cwd: string, skipPermissions: boolean, pullLatest: boolean, initWithClaude: boolean) => void;
```

- [ ] **Step 2: Add initWithClaude state**

Add to the component's state declarations:

```typescript
const [initWithClaude, setInitWithClaude] = useState(true);
```

Add to the `useEffect` reset block:

```typescript
setInitWithClaude(true);
```

- [ ] **Step 3: Update handleCreate to pass initWithClaude**

```typescript
const handleCreate = () => {
  const trimmedName = name.trim();
  if (!trimmedName || !directory) return;
  onCreate(trimmedName, directory, skipPermissions, pullLatest, initWithClaude);
};
```

- [ ] **Step 4: When initWithClaude unchecked, force skipPermissions off**

Add an effect or inline logic — simplest is to compute the effective value:

```typescript
const effectiveSkipPermissions = initWithClaude ? skipPermissions : false;
```

And use `effectiveSkipPermissions` in `handleCreate` instead of `skipPermissions`.

- [ ] **Step 5: Add the checkbox to the JSX**

Insert above the existing "Pull latest from main" checkbox:

```tsx
<label className={styles.checkboxRow}>
  <input
    type="checkbox"
    checked={initWithClaude}
    onChange={(e) => setInitWithClaude(e.target.checked)}
    className={styles.checkbox}
  />
  <span className={`${styles.checkboxLabel} ${styles.checkboxLabelPrimary}`}>
    Initialise with Claude
  </span>
</label>
```

- [ ] **Step 6: Disable skip-permissions when initWithClaude is unchecked**

Update the skip-permissions checkbox:

```tsx
<label className={`${styles.checkboxRow} ${!initWithClaude ? styles.checkboxDisabled : ""}`}>
  <input
    type="checkbox"
    checked={effectiveSkipPermissions}
    onChange={(e) => setSkipPermissions(e.target.checked)}
    disabled={!initWithClaude}
    className={styles.checkbox}
  />
  <span className={styles.checkboxLabel}>Skip permissions</span>
</label>
```

- [ ] **Step 7: Add CSS for primary label and disabled state**

Add to `NewSessionModal.module.css`:

```css
.checkboxLabelPrimary {
  color: #e5e7eb;
  font-weight: 500;
}

.checkboxDisabled {
  opacity: 0.4;
  cursor: default;
}
```

- [ ] **Step 8: Verify it compiles**

Run: `npx vitest run 2>&1 | tail -10`

Expected: Compilation errors in App.tsx (handleCreateSession signature mismatch). We fix that next.

- [ ] **Step 9: Commit**

```bash
git add src/components/NewSessionModal/NewSessionModal.tsx src/components/NewSessionModal/NewSessionModal.module.css
git commit -m "feat: add Initialise with Claude checkbox to New Session modal"
```

### Task 8: Update App.tsx bridge

**Files:**
- Modify: `src/App.tsx:88-91` (handleCreateSession)

- [ ] **Step 1: Update handleCreateSession signature**

```typescript
const handleCreateSession = async (name: string, cwd: string, skipPermissions: boolean, pullLatest: boolean, initWithClaude: boolean) => {
  setIsModalOpen(false);
  await createSession(name, cwd, skipPermissions, pullLatest, initWithClaude);
};
```

- [ ] **Step 2: Verify it compiles**

Run: `npx vitest run 2>&1 | tail -10`

Expected: All tests pass (or only SessionCard test failures due to missing sessionType — fixed next task).

- [ ] **Step 3: Commit**

```bash
git add src/App.tsx
git commit -m "feat: forward initWithClaude through App.tsx to store"
```

### Task 9: Update SessionCard for terminal status

**Files:**
- Modify: `src/components/SessionCard/SessionCard.tsx:19-39`
- Modify: `src/components/SessionCard/SessionCard.module.css`

- [ ] **Step 1: Write failing test for terminal session card**

Add to `src/components/SessionCard/SessionCard.test.tsx`:

```typescript
describe("SessionCard terminal sessions", () => {
  it("renders a status dot (not checkmark) for terminal status", () => {
    const session = makeSession({ status: "terminal", sessionType: "terminal" });
    const { container } = render(
      <SessionCard session={session} isActive={false} onClick={vi.fn()} />
    );
    // Should NOT render the checkmark
    expect(screen.queryByText("✓")).toBeNull();
    // Should render a dot
    expect(container.querySelector('[class*="statusDot"]')).toBeTruthy();
  });

  it("shows 'Terminal' as status label", () => {
    const session = makeSession({ status: "terminal", sessionType: "terminal" });
    render(
      <SessionCard session={session} isActive={false} onClick={vi.fn()} />
    );
    expect(screen.getByText("Terminal")).toBeTruthy();
  });

  it("treats terminal sessions as running (closeable, not dismissable)", () => {
    const onClose = vi.fn();
    const onDismiss = vi.fn();
    const session = makeSession({ status: "terminal", sessionType: "terminal" });
    render(
      <SessionCard
        session={session}
        isActive={false}
        onClick={vi.fn()}
        onClose={onClose}
        onDismiss={onDismiss}
      />
    );

    const card = screen.getByText("My Session").closest("[role='button']")!;
    fireEvent.contextMenu(card);

    // Should show "Close Session", not "Dismiss"
    expect(screen.getByText("Close Session")).toBeTruthy();
    expect(screen.queryByText("Dismiss")).toBeNull();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/components/SessionCard/SessionCard.test.ts 2>&1 | tail -20`

Expected: FAIL — `"terminal"` not in STATUS_DOT_CLASS, sessionType not on makeSession.

- [ ] **Step 3: Update makeSession helper**

```typescript
function makeSession(overrides?: Partial<SessionInfo>): SessionInfo {
  return {
    id: "test-1",
    name: "My Session",
    status: "idle",
    createdAt: Date.now(),
    cwd: "/projects/app",
    sessionType: "claude",
    ...overrides,
  };
}
```

- [ ] **Step 4: Add terminal to STATUS_DOT_CLASS and STATUS_LABEL**

```typescript
const STATUS_DOT_CLASS: Record<SessionStatus, string> = {
  starting: styles.statusStarting,
  working: styles.statusWorking,
  idle: styles.statusIdle,
  needs_attention: styles.statusNeedsAttention,
  finished: styles.statusFinished,
  error: styles.statusError,
  terminal: styles.statusTerminal,
};

const STATUS_LABEL: Record<SessionStatus, string> = {
  starting: "Starting...",
  working: "Working",
  idle: "Idle",
  needs_attention: "Needs Attention",
  finished: "Finished",
  error: "Error",
  terminal: "Terminal",
};
```

- [ ] **Step 5: Update isRunning to handle terminal status**

Terminal sessions are "running" (closeable) while alive:

```typescript
function isRunning(status: SessionStatus): boolean {
  return status !== "finished" && status !== "error";
}
```

This already works — `"terminal"` is neither `"finished"` nor `"error"`, so `isRunning("terminal")` returns `true`. No change needed.

- [ ] **Step 6: Hide timer for terminal sessions**

Update the DurationTimer line (~line 137):

```tsx
{session.sessionType !== "terminal" && (
  <DurationTimer createdAt={session.createdAt} active={isRunning(session.status)} />
)}
```

- [ ] **Step 7: Add CSS for terminal status dot**

Add to `SessionCard.module.css`:

```css
.statusTerminal { background-color: #6b7280; }
```

- [ ] **Step 8: Run tests to verify they pass**

Run: `npx vitest run src/components/SessionCard/SessionCard.test.tsx 2>&1 | tail -20`

Expected: All tests pass.

- [ ] **Step 9: Commit**

```bash
git add src/components/SessionCard/SessionCard.tsx src/components/SessionCard/SessionCard.module.css src/components/SessionCard/SessionCard.test.tsx
git commit -m "feat: render terminal status in SessionCard with grey dot, no timer"
```

### Task 10: Update useInitializeSessions for sessionType

**Files:**
- Modify: `src/hooks/useInitializeSessions.ts`

- [ ] **Step 1: Update session restoration logic**

The hook calls `list_sessions` which now returns `session_type`. The backend response uses snake_case (`session_type`), but our frontend `SessionInfo` uses camelCase (`sessionType`). The existing hook passes `SessionInfo[]` directly from the IPC response.

Update to map the backend response:

```typescript
export function useInitializeSessions() {
  const addSession = useSessionStore((s) => s.addSession);
  const setupEventListeners = useSessionStore((s) => s.setupEventListeners);

  useEffect(() => {
    async function init() {
      try {
        const existing = await invoke<Array<{
          id: string;
          name: string;
          cwd: string;
          created_at_epoch_ms: number;
          session_type: string;
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
          };
          addSession(session);
          setupEventListeners(session.id);
        }
      } catch (err) {
        console.error("Failed to initialize sessions:", err);
      }
    }
    init();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps
}
```

- [ ] **Step 2: Verify frontend compiles and all tests pass**

Run: `npx vitest run 2>&1 | tail -20`

Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/hooks/useInitializeSessions.ts
git commit -m "feat: restore session type when reinitializing from backend"
```

### Task 11: Run full test suite

- [ ] **Step 1: Run frontend tests**

Run: `npx vitest run 2>&1 | tail -20`

Expected: All pass.

- [ ] **Step 2: Run backend tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -20`

Expected: All pass.

- [ ] **Step 3: Build check**

Run: `npm run tauri build 2>&1 | tail -20`

Expected: Builds successfully.
