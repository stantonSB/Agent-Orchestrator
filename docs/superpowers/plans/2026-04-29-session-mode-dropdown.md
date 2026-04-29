# Session Mode Dropdown Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the "Initialise with Claude" and "Skip permissions" checkboxes with a single "Session Mode" dropdown that persists the user's last selection.

**Architecture:** Add a `SessionMode` type, refactor `createSession` to accept it instead of two booleans, update the modal UI to render a `<select>`, and persist selection via `localStorage`.

**Tech Stack:** React, TypeScript, Zustand, CSS Modules, Vitest

**Spec:** `docs/superpowers/specs/2026-04-29-session-mode-dropdown-design.md`

---

## Chunk 1: Type + Store + Tests

### Task 1: Add SessionMode type

**Files:**
- Modify: `src/types/session.ts`

- [ ] **Step 1: Add the SessionMode type**

Add after the `SessionStatus` type at the top of the file:

```typescript
export type SessionMode = "claude" | "claude-skip" | "claude-plan" | "terminal";
```

- [ ] **Step 2: Commit**

```bash
git add src/types/session.ts
git commit -m "feat: add SessionMode type"
```

---

### Task 2: Update sessionStore interface and implementation

**Files:**
- Modify: `src/stores/sessionStore.ts`

- [ ] **Step 1: Add import for SessionMode**

In `src/stores/sessionStore.ts:4`, change:

```typescript
import type { SessionInfo, SessionStatus, SubagentStatus } from "../types/session";
```

to:

```typescript
import type { SessionInfo, SessionMode, SessionStatus, SubagentStatus } from "../types/session";
```

- [ ] **Step 2: Update the SessionState interface**

In `src/stores/sessionStore.ts:25`, change:

```typescript
createSession: (name: string, cwd: string, skipPermissions?: boolean, pullLatest?: boolean, initWithClaude?: boolean, isGitRepo?: boolean) => Promise<void>;
```

to:

```typescript
createSession: (name: string, cwd: string, sessionMode?: SessionMode, pullLatest?: boolean, isGitRepo?: boolean) => Promise<void>;
```

- [ ] **Step 3: Update the createSession implementation**

Replace the entire `createSession` implementation (lines 183-248) with:

```typescript
createSession: async (name, cwd, sessionMode = "claude", pullLatest = false, isGitRepo = true) => {
  if (pullLatest) {
    await invoke("git_pull_main", { cwd });
  }

  let id: string;
  let session: SessionInfo;

  if (sessionMode === "terminal") {
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
      isGitRepo: false,
    };
  } else {
    const args: string[] = [];
    if (sessionMode === "claude-skip") {
      args.push("--dangerously-skip-permissions");
    } else if (sessionMode === "claude-plan") {
      args.push("--plan");
    }
    if (isGitRepo) {
      args.push("--worktree");
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
      isGitRepo,
    };
  }

  get().addSession(session);
  get().setActiveSession(id);
  get().setupEventListeners(id);
  set({ lastUsedDirectory: cwd });

  if (sessionMode !== "terminal") {
    try {
      const currentStatus = await invoke<string | null>("get_session_status", { id });
      if (currentStatus && currentStatus !== "starting") {
        get().updateSessionStatus(id, currentStatus as SessionStatus);
      }
    } catch {
      // Session may have already been removed
    }
  }
},
```

- [ ] **Step 4: Run tests to confirm they fail (expected — signatures changed)**

Run: `npx vitest run src/stores/sessionStore.test.ts`
Expected: Multiple failures in the `createSession` describe block due to old signatures.

- [ ] **Step 5: Commit**

```bash
git add src/stores/sessionStore.ts
git commit -m "feat: refactor createSession to accept SessionMode"
```

---

### Task 3: Update sessionStore tests

**Files:**
- Modify: `src/stores/sessionStore.test.ts`

- [ ] **Step 1: Update test — "calls Tauri invoke and adds the session" (line 150)**

Change `src/stores/sessionStore.test.ts` line 155:

```typescript
await store.createSession("My Session", "/path/to/project");
```

to:

```typescript
await store.createSession("My Session", "/path/to/project", "claude-skip");
```

The assertion on line 161 stays the same (`args: ["--dangerously-skip-permissions", "--worktree"]`).

- [ ] **Step 2: Update test — "calls git_pull_main before create_session" (line 173)**

Change line 183:

```typescript
await store.createSession("Pull Session", "/path/to/project", true, true);
```

to:

```typescript
await store.createSession("Pull Session", "/path/to/project", "claude-skip", true);
```

- [ ] **Step 3: Update test — "does NOT call git_pull_main when pullLatest is false" (line 191)**

Change line 196:

```typescript
await store.createSession("No Pull", "/path/to/project", true, false);
```

to:

```typescript
await store.createSession("No Pull", "/path/to/project", "claude-skip", false);
```

- [ ] **Step 4: Update test — "creates a terminal session" (line 202)**

Update the test description from `"creates a terminal session when initWithClaude is false"` to `"creates a terminal session when mode is 'terminal'"`.

Change line 207:

```typescript
await store.createSession("My Terminal", "/path/to/project", true, false, false);
```

to:

```typescript
await store.createSession("My Terminal", "/path/to/project", "terminal");
```

- [ ] **Step 5: Update test — "does NOT create session when git_pull_main fails" (line 220)**

Change line 226:

```typescript
store.createSession("Fail Pull", "/path/to/project", true, true)
```

to:

```typescript
store.createSession("Fail Pull", "/path/to/project", "claude-skip", true)
```

- [ ] **Step 6: Update test — "omits --worktree when isGitRepo is false" (line 233)**

Change line 238:

```typescript
await store.createSession("Non-Git Session", "/path/to/non-git", true, false, true, false);
```

to:

```typescript
await store.createSession("Non-Git Session", "/path/to/non-git", "claude-skip", false, false);
```

- [ ] **Step 7: Update test — "includes --worktree when isGitRepo is true" (line 252)**

Change line 257:

```typescript
await store.createSession("Git Session", "/path/to/git-repo", true, false, true, true);
```

to:

```typescript
await store.createSession("Git Session", "/path/to/git-repo", "claude-skip", false, true);
```

- [ ] **Step 8: Update test — "sets lastUsedDirectory" (line 295)**

Change line 301:

```typescript
await store.createSession("Dir Test", "/projects/my-app");
```

This one uses defaults so it will pass `sessionMode="claude"` by default. No change needed — but verify it doesn't assert `--dangerously-skip-permissions`. It doesn't (it only checks `lastUsedDirectory`), so no change needed.

- [ ] **Step 9: Add new tests for claude and claude-plan modes**

Add these tests inside the `createSession` describe block (after the existing tests, before the closing `});`):

```typescript
it("creates a claude session with no extra args when mode is 'claude'", async () => {
  const { invoke } = await import("@tauri-apps/api/core");
  vi.mocked(invoke).mockResolvedValueOnce("claude-default-id");

  const store = useSessionStore.getState();
  await store.createSession("Default Claude", "/path/to/project", "claude");

  expect(invoke).toHaveBeenCalledWith("create_session", {
    name: "Default Claude",
    cwd: "/path/to/project",
    command: "claude",
    args: ["--worktree"],
    sessionType: "claude",
  });
});

it("creates a claude session with --plan when mode is 'claude-plan'", async () => {
  const { invoke } = await import("@tauri-apps/api/core");
  vi.mocked(invoke).mockResolvedValueOnce("plan-id");

  const store = useSessionStore.getState();
  await store.createSession("Plan Session", "/path/to/project", "claude-plan");

  expect(invoke).toHaveBeenCalledWith("create_session", {
    name: "Plan Session",
    cwd: "/path/to/project",
    command: "claude",
    args: ["--plan", "--worktree"],
    sessionType: "claude",
  });
});
```

- [ ] **Step 10: Run tests to verify all pass**

Run: `npx vitest run src/stores/sessionStore.test.ts`
Expected: All tests pass.

- [ ] **Step 11: Commit**

```bash
git add src/stores/sessionStore.test.ts
git commit -m "test: update createSession tests for SessionMode"
```

---

## Chunk 2: UI + Wiring

### Task 4: Update App.tsx handler

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: Add SessionMode import**

Add to `src/App.tsx` imports:

```typescript
import type { SessionMode } from "./types/session";
```

- [ ] **Step 2: Update handleCreateSession**

Change `src/App.tsx:88-91`:

```typescript
const handleCreateSession = async (name: string, cwd: string, skipPermissions: boolean, pullLatest: boolean, initWithClaude: boolean, isGitRepo: boolean) => {
  setIsModalOpen(false);
  await createSession(name, cwd, skipPermissions, pullLatest, initWithClaude, isGitRepo);
};
```

to:

```typescript
const handleCreateSession = async (name: string, cwd: string, sessionMode: SessionMode, pullLatest: boolean, isGitRepo: boolean) => {
  setIsModalOpen(false);
  await createSession(name, cwd, sessionMode, pullLatest, isGitRepo);
};
```

- [ ] **Step 3: Commit**

```bash
git add src/App.tsx
git commit -m "feat: update App handleCreateSession for SessionMode"
```

---

### Task 5: Add select CSS style

**Files:**
- Modify: `src/components/NewSessionModal/NewSessionModal.module.css`

- [ ] **Step 1: Add .select style and remove .checkboxLabelPrimary**

Add after the `.input::placeholder` block (after line 61):

```css
.select {
  padding: 8px 12px;
  background: #12121a;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  color: #e5e7eb;
  font-size: 14px;
  font-family: "SF Mono", "Menlo", "Monaco", monospace;
  outline: none;
  transition: border-color 0.15s ease;
  cursor: pointer;
  width: 100%;
}

.select:focus {
  border-color: rgba(59, 130, 246, 0.5);
}
```

Remove the `.checkboxLabelPrimary` block (lines 127-130):

```css
.checkboxLabelPrimary {
  color: #e5e7eb;
  font-weight: 500;
}
```

- [ ] **Step 2: Commit**

```bash
git add src/components/NewSessionModal/NewSessionModal.module.css
git commit -m "feat: add select style, remove unused checkboxLabelPrimary"
```

---

### Task 6: Rewrite NewSessionModal

**Files:**
- Modify: `src/components/NewSessionModal/NewSessionModal.tsx`

- [ ] **Step 1: Replace the entire component**

Replace the full contents of `src/components/NewSessionModal/NewSessionModal.tsx` with:

```typescript
import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { SessionMode } from "../../types/session";
import styles from "./NewSessionModal.module.css";

const STORAGE_KEY = "ao-last-session-mode";
const VALID_MODES: SessionMode[] = ["claude", "claude-skip", "claude-plan", "terminal"];

function getStoredMode(): SessionMode {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored && VALID_MODES.includes(stored as SessionMode)) {
    return stored as SessionMode;
  }
  return "claude";
}

interface NewSessionModalProps {
  isOpen: boolean;
  onClose: () => void;
  onCreate: (name: string, cwd: string, sessionMode: SessionMode, pullLatest: boolean, isGitRepo: boolean) => void;
  lastUsedDirectory: string | null;
}

export function NewSessionModal({
  isOpen,
  onClose,
  onCreate,
  lastUsedDirectory,
}: NewSessionModalProps) {
  const [name, setName] = useState("");
  const [directory, setDirectory] = useState<string | null>(null);
  const [sessionMode, setSessionMode] = useState<SessionMode>(getStoredMode);
  const [pullLatest, setPullLatest] = useState(false);
  const [isGitRepo, setIsGitRepo] = useState<boolean | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      setName("");
      setDirectory(lastUsedDirectory);
      setSessionMode(getStoredMode());
      setPullLatest(false);
      setIsGitRepo(null);
      if (lastUsedDirectory) {
        invoke<boolean>("check_is_git_repo", { cwd: lastUsedDirectory })
          .then(setIsGitRepo)
          .catch(() => setIsGitRepo(false));
      }
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen, lastUsedDirectory]);

  useEffect(() => {
    if (!directory) {
      setIsGitRepo(null);
      return;
    }
    invoke<boolean>("check_is_git_repo", { cwd: directory })
      .then(setIsGitRepo)
      .catch(() => setIsGitRepo(false));
  }, [directory]);

  if (!isOpen) return null;

  const handleBrowse = async () => {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: "Select project directory",
      defaultPath: directory ?? undefined,
    });
    if (typeof selected === "string") {
      setDirectory(selected);
    }
  };

  const effectivePullLatest = isGitRepo === false ? false : pullLatest;

  const handleCreate = () => {
    const trimmedName = name.trim();
    if (!trimmedName || !directory) return;
    localStorage.setItem(STORAGE_KEY, sessionMode);
    onCreate(trimmedName, directory, sessionMode, effectivePullLatest, isGitRepo ?? false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose();
    }
    if (e.key === "Enter" && name.trim() && directory) {
      handleCreate();
    }
  };

  const isValid = name.trim().length > 0 && directory !== null;

  return (
    <div className={styles.overlay} onClick={onClose}>
      <div
        className={styles.modal}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
        tabIndex={-1}
      >
        <h2 className={styles.title}>New Session</h2>

        <div className={styles.field}>
          <label className={styles.label} htmlFor="session-name">
            Session Name
          </label>
          <input
            ref={inputRef}
            id="session-name"
            className={styles.input}
            type="text"
            placeholder="e.g. fix-auth-bug"
            value={name}
            onChange={(e) => setName(e.target.value)}
            autoComplete="off"
          />
        </div>

        <div className={styles.field}>
          <label className={styles.label}>Project Directory</label>
          <div className={styles.folderRow}>
            <div
              className={`${styles.folderPath} ${directory ? styles.hasValue : ""}`}
            >
              {directory ?? "No directory selected"}
            </div>
            <button
              className={styles.browseButton}
              onClick={handleBrowse}
              type="button"
            >
              Browse
            </button>
          </div>
        </div>

        <div className={styles.field}>
          <label className={styles.label} htmlFor="session-mode">
            Session Mode
          </label>
          <select
            id="session-mode"
            className={styles.select}
            value={sessionMode}
            onChange={(e) => setSessionMode(e.target.value as SessionMode)}
          >
            <option value="claude">Claude</option>
            <option value="claude-skip">Claude (skip permissions)</option>
            <option value="claude-plan">Claude (plan mode)</option>
            <option value="terminal">Terminal</option>
          </select>
        </div>

        <label className={`${styles.checkboxRow} ${isGitRepo === false ? styles.checkboxDisabled : ""}`}>
          <input
            type="checkbox"
            checked={effectivePullLatest}
            onChange={(e) => setPullLatest(e.target.checked)}
            disabled={isGitRepo === false}
            className={styles.checkbox}
          />
          <span className={styles.checkboxLabel}>Pull latest from main</span>
        </label>

        <div className={styles.actions}>
          <button
            className={styles.cancelButton}
            onClick={onClose}
            type="button"
          >
            Cancel
          </button>
          <button
            className={styles.createButton}
            onClick={handleCreate}
            disabled={!isValid}
            type="button"
          >
            Create
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Run all frontend tests**

Run: `npx vitest run`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/components/NewSessionModal/NewSessionModal.tsx
git commit -m "feat: replace checkboxes with session mode dropdown"
```

---

### Task 7: Manual smoke test

- [ ] **Step 1: Run dev mode**

Run: `npm run tauri dev`

- [ ] **Step 2: Verify dropdown**

Open New Session modal (Cmd+T). Confirm:
- Dropdown shows 4 options: Claude, Claude (skip permissions), Claude (plan mode), Terminal
- "Pull latest from main" checkbox still appears and is disabled for non-git directories
- Default is "Claude" on first use

- [ ] **Step 3: Verify persistence**

Create a session with "Claude (skip permissions)". Open modal again. Confirm it defaults to "Claude (skip permissions)".

- [ ] **Step 4: Verify each mode works**

Create one session per mode and confirm:
- "Claude" → Claude starts with permission prompts
- "Claude (skip permissions)" → Claude starts without permission prompts
- "Claude (plan mode)" → Claude starts in plan mode
- "Terminal" → plain shell opens

- [ ] **Step 5: Final commit (if any fixes needed)**

```bash
git add -A
git commit -m "fix: address smoke test issues"
```
