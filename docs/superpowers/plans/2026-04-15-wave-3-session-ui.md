# Wave 3: Session Management UI

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Zustand state store and session panel UI, then wire them together so clicking "New Session" spawns a PTY and updates the sidebar end-to-end.

**Architecture:** A Zustand store (`useSessionStore`) owns the `Map<string, SessionInfo>` and `activeSessionId`, exposes CRUD methods, and subscribes to Tauri events (`session-status-{id}`, `session-exit-{id}`) to keep state in sync with the backend. Presentational components (SessionPanel, SessionCard, NewSessionButton, NewSessionModal) consume the store and render session state. Task 3C wires the store to the components and connects the "New Session" flow end-to-end through the Tauri IPC bridge established in Wave 2.

**Tech Stack:** Zustand 5, React 19, TypeScript, CSS Modules, Tauri 2 IPC (invoke/listen), `@tauri-apps/plugin-dialog` (native folder picker), xterm.js

---

## Prerequisites (from Waves 1-2)

These files/modules are assumed to exist and work:

- **Tauri commands**: `create_session`, `close_session`, `rename_session`, `write_to_session`, `resize_session`, `list_sessions` (in `src-tauri/src/`)
- **Tauri events**: `session-output-{id}`, `session-status-{id}`, `session-exit-{id}`
- **React scaffold**: `src/main.tsx`, `src/App.tsx`, Vite config, CSS Modules working
- **App shell**: `src/components/TitleBar/`, two-pane layout in `App.tsx`
- **XTermInstance**: `src/components/XTermInstance/XTermInstance.tsx` — renders an xterm.js terminal, accepts `sessionId` and `isVisible` props, listens for `session-output-{id}` events. The `isVisible` prop controls CSS `display: block`/`display: none` so that inactive terminals remain mounted in the DOM (preserving scrollback and xterm state) rather than being unmounted and remounted on session switch.

---

## Task 3A: Zustand Store + Session State

> **Parallel with:** 3B  
> **Files to create:** `src/stores/sessionStore.ts`, `src/stores/sessionStore.test.ts`, `src/types/session.ts`  
> **Files to modify:** none

### Step 3A.1: Create session types

- [ ] Create `src/types/session.ts` with the `SessionInfo` interface and `SessionStatus` type:

```typescript
// src/types/session.ts

export type SessionStatus =
  | "starting"
  | "working"
  | "idle"
  | "needs_attention"
  | "finished"
  | "error";

export interface SessionInfo {
  id: string;
  name: string;
  status: SessionStatus;
  createdAt: number; // unix timestamp ms
}
```

### Step 3A.2: Write tests for the Zustand store

- [ ] Create `src/stores/sessionStore.test.ts`:

```typescript
// src/stores/sessionStore.test.ts

import { describe, it, expect, beforeEach, vi } from "vitest";
import { useSessionStore } from "./sessionStore";

// Mock Tauri APIs
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn())), // returns unlisten fn
}));

describe("sessionStore", () => {
  beforeEach(() => {
    // Reset store between tests
    useSessionStore.setState({
      sessions: new Map(),
      activeSessionId: null,
    });
  });

  describe("addSession", () => {
    it("adds a session to the map", () => {
      const { addSession } = useSessionStore.getState();
      addSession({
        id: "abc-123",
        name: "Test Session",
        status: "starting",
        createdAt: Date.now(),
      });

      const { sessions } = useSessionStore.getState();
      expect(sessions.size).toBe(1);
      expect(sessions.get("abc-123")?.name).toBe("Test Session");
    });
  });

  describe("removeSession", () => {
    it("removes a session from the map", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "abc-123",
        name: "Test",
        status: "idle",
        createdAt: Date.now(),
      });
      store.removeSession("abc-123");

      const { sessions } = useSessionStore.getState();
      expect(sessions.size).toBe(0);
    });

    it("clears activeSessionId if the removed session was active", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "abc-123",
        name: "Test",
        status: "idle",
        createdAt: Date.now(),
      });
      store.setActiveSession("abc-123");
      store.removeSession("abc-123");

      const { activeSessionId } = useSessionStore.getState();
      expect(activeSessionId).toBeNull();
    });
  });

  describe("updateSessionStatus", () => {
    it("updates the status of an existing session", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "abc-123",
        name: "Test",
        status: "starting",
        createdAt: Date.now(),
      });
      store.updateSessionStatus("abc-123", "working");

      const session = useSessionStore.getState().sessions.get("abc-123");
      expect(session?.status).toBe("working");
    });

    it("no-ops for a non-existent session", () => {
      const store = useSessionStore.getState();
      store.updateSessionStatus("nonexistent", "working");
      expect(useSessionStore.getState().sessions.size).toBe(0);
    });
  });

  describe("setActiveSession", () => {
    it("sets the active session id", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "abc-123",
        name: "Test",
        status: "idle",
        createdAt: Date.now(),
      });
      store.setActiveSession("abc-123");

      expect(useSessionStore.getState().activeSessionId).toBe("abc-123");
    });
  });

  describe("createSession", () => {
    it("calls Tauri invoke and adds the session", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockResolvedValueOnce("new-id-456");

      const store = useSessionStore.getState();
      await store.createSession("My Session", "/path/to/project");

      expect(invoke).toHaveBeenCalledWith("create_session", {
        name: "My Session",
        cwd: "/path/to/project",
      });

      const { sessions, activeSessionId } = useSessionStore.getState();
      expect(sessions.has("new-id-456")).toBe(true);
      expect(sessions.get("new-id-456")?.name).toBe("My Session");
      expect(sessions.get("new-id-456")?.status).toBe("starting");
      expect(activeSessionId).toBe("new-id-456");
    });
  });

  describe("closeSession", () => {
    it("calls Tauri invoke and removes the session", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockResolvedValueOnce(undefined);

      const store = useSessionStore.getState();
      store.addSession({
        id: "abc-123",
        name: "Test",
        status: "idle",
        createdAt: Date.now(),
      });

      await store.closeSession("abc-123");

      expect(invoke).toHaveBeenCalledWith("close_session", { id: "abc-123" });
      expect(useSessionStore.getState().sessions.has("abc-123")).toBe(false);
    });
  });
});
```

### Step 3A.3: Run tests (expect failures)

- [ ] Run the test suite and confirm the tests fail because the store doesn't exist yet:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
npx vitest run src/stores/sessionStore.test.ts
```

Expected: all tests fail with "Cannot find module './sessionStore'"

### Step 3A.4: Implement the Zustand store

- [ ] Create `src/stores/sessionStore.ts`:

```typescript
// src/stores/sessionStore.ts

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { SessionInfo, SessionStatus } from "../types/session";

interface SessionState {
  sessions: Map<string, SessionInfo>;
  activeSessionId: string | null;

  // Mutations
  addSession: (session: SessionInfo) => void;
  removeSession: (id: string) => void;
  updateSessionStatus: (id: string, status: SessionStatus) => void;
  setActiveSession: (id: string) => void;

  // Tauri IPC actions
  createSession: (name: string, cwd: string) => Promise<void>;
  closeSession: (id: string) => Promise<void>;
  renameSession: (id: string, name: string) => Promise<void>;

  // Event listener management
  setupEventListeners: (sessionId: string) => void;
}

// Track unlisten functions outside the store to avoid serialization issues
const eventCleanups = new Map<string, UnlistenFn[]>();

export const useSessionStore = create<SessionState>((set, get) => ({
  sessions: new Map(),
  activeSessionId: null,

  addSession: (session) =>
    set((state) => {
      const next = new Map(state.sessions);
      next.set(session.id, session);
      return { sessions: next };
    }),

  removeSession: (id) =>
    set((state) => {
      const next = new Map(state.sessions);
      next.delete(id);

      // Clean up event listeners
      const cleanups = eventCleanups.get(id);
      if (cleanups) {
        cleanups.forEach((unlisten) => unlisten());
        eventCleanups.delete(id);
      }

      return {
        sessions: next,
        activeSessionId: state.activeSessionId === id ? null : state.activeSessionId,
      };
    }),

  updateSessionStatus: (id, status) =>
    set((state) => {
      const session = state.sessions.get(id);
      if (!session) return state;

      const next = new Map(state.sessions);
      next.set(id, { ...session, status });
      return { sessions: next };
    }),

  setActiveSession: (id) => set({ activeSessionId: id }),

  createSession: async (name, cwd) => {
    const id = await invoke<string>("create_session", { name, cwd });
    const session: SessionInfo = {
      id,
      name,
      status: "starting",
      createdAt: Date.now(),
    };
    get().addSession(session);
    get().setActiveSession(id);
    get().setupEventListeners(id);
  },

  closeSession: async (id) => {
    await invoke("close_session", { id });
    get().removeSession(id);
  },

  renameSession: async (id, name) => {
    await invoke("rename_session", { id, name });
    set((state) => {
      const session = state.sessions.get(id);
      if (!session) return state;
      const next = new Map(state.sessions);
      next.set(id, { ...session, name });
      return { sessions: next };
    });
  },

  setupEventListeners: (sessionId) => {
    // Guard against race condition: if removeSession runs before the
    // listen() promises resolve, the unlisten fns would be stored after
    // cleanup already ran. We use a cancelled flag to detect this case
    // and immediately unlisten if the session was already removed.
    let cancelled = false;

    const cleanups: Promise<UnlistenFn>[] = [];

    // Listen for status changes
    cleanups.push(
      listen<{ status: SessionStatus }>(`session-status-${sessionId}`, (event) => {
        get().updateSessionStatus(sessionId, event.payload.status);
      })
    );

    // Listen for session exit
    cleanups.push(
      listen<{ exitCode: number }>(`session-exit-${sessionId}`, (event) => {
        const status: SessionStatus = event.payload.exitCode === 0 ? "finished" : "error";
        get().updateSessionStatus(sessionId, status);
      })
    );

    // Store a cancel function so removeSession can signal late-resolving listeners
    eventCleanups.set(sessionId, [() => { cancelled = true; }]);

    Promise.all(cleanups).then((unlistenFns) => {
      if (cancelled) {
        // Session was already removed before listeners registered — clean up immediately
        unlistenFns.forEach((unlisten) => unlisten());
        return;
      }
      eventCleanups.set(sessionId, unlistenFns);
    });
  },
}));
```

### Step 3A.5: Run tests (expect pass)

- [ ] Re-run the test suite:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
npx vitest run src/stores/sessionStore.test.ts
```

Expected: all 8 tests pass.

### Step 3A.6: Add event listener integration tests

- [ ] Add these tests to the bottom of `src/stores/sessionStore.test.ts` to verify event listener wiring:

```typescript
describe("setupEventListeners", () => {
  it("registers listeners for status and exit events", async () => {
    const { listen } = await import("@tauri-apps/api/event");

    const store = useSessionStore.getState();
    store.setupEventListeners("test-session");

    // listen should have been called twice: once for status, once for exit
    expect(listen).toHaveBeenCalledWith(
      "session-status-test-session",
      expect.any(Function)
    );
    expect(listen).toHaveBeenCalledWith(
      "session-exit-test-session",
      expect.any(Function)
    );
  });
});
```

### Step 3A.7: Run full test suite

- [ ] Confirm all tests pass:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
npx vitest run src/stores/sessionStore.test.ts
```

Expected: all 9 tests pass.

---

## Task 3B: Session Panel Components

> **Parallel with:** 3A  
> **Files to create:** `src/components/SessionPanel/SessionPanel.tsx`, `src/components/SessionPanel/SessionPanel.module.css`, `src/components/SessionCard/SessionCard.tsx`, `src/components/SessionCard/SessionCard.module.css`, `src/components/NewSessionButton/NewSessionButton.tsx`, `src/components/NewSessionButton/NewSessionButton.module.css`, `src/components/NewSessionModal/NewSessionModal.tsx`, `src/components/NewSessionModal/NewSessionModal.module.css`  
> **Files to modify:** none (these are standalone presentational components)

### Step 3B.0: Install the dialog plugin

- [ ] Install `@tauri-apps/plugin-dialog` (required by `NewSessionModal` for the native folder picker):

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
npm install @tauri-apps/plugin-dialog
```

- [ ] Register the dialog plugin in the Tauri config. Add `"dialog"` to the `plugins` array in `src-tauri/tauri.conf.json`:

```json
{
  "plugins": {
    "dialog": {}
  }
}
```

- [ ] Add the plugin to the Rust side. In `src-tauri/Cargo.toml`, add:

```toml
tauri-plugin-dialog = "2"
```

And in `src-tauri/src/lib.rs` (or `main.rs`), register the plugin:

```rust
.plugin(tauri_plugin_dialog::init())
```

### Step 3B.1: Create the SessionCard component

- [ ] Create `src/components/SessionCard/SessionCard.module.css`:

```css
/* src/components/SessionCard/SessionCard.module.css */

.card {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: 6px;
  cursor: pointer;
  border: 1px solid transparent;
  transition: background-color 0.15s ease, border-color 0.15s ease;
  user-select: none;
}

.card:hover {
  background-color: rgba(255, 255, 255, 0.05);
}

.active {
  background-color: rgba(255, 255, 255, 0.08);
  border-color: rgba(255, 255, 255, 0.12);
}

.dismissed {
  opacity: 0.5;
}

.statusDot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.statusStarting {
  background-color: #3b82f6; /* blue */
}

.statusWorking {
  background-color: #22c55e; /* green */
  animation: pulse 1.5s ease-in-out infinite;
}

.statusIdle {
  background-color: #6b7280; /* gray */
}

.statusNeedsAttention {
  background-color: #f97316; /* orange */
}

.statusFinished {
  background-color: #6b7280; /* muted gray */
}

.statusError {
  background-color: #ef4444; /* red */
}

@keyframes pulse {
  0%, 100% {
    opacity: 1;
  }
  50% {
    opacity: 0.4;
  }
}

.info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  flex: 1;
}

.name {
  font-size: 13px;
  font-weight: 500;
  color: #e5e7eb;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.status {
  font-size: 11px;
  color: #9ca3af;
  font-family: "SF Mono", "Menlo", "Monaco", monospace;
  text-transform: capitalize;
}

.finishedIcon {
  color: #6b7280;
  font-size: 12px;
  flex-shrink: 0;
}
```

- [ ] Create `src/components/SessionCard/SessionCard.tsx`:

```typescript
// src/components/SessionCard/SessionCard.tsx

import type { SessionInfo, SessionStatus } from "../../types/session";
import styles from "./SessionCard.module.css";

interface SessionCardProps {
  session: SessionInfo;
  isActive: boolean;
  onClick: (id: string) => void;
}

const STATUS_DOT_CLASS: Record<SessionStatus, string> = {
  starting: styles.statusStarting,
  working: styles.statusWorking,
  idle: styles.statusIdle,
  needs_attention: styles.statusNeedsAttention,
  finished: styles.statusFinished,
  error: styles.statusError,
};

const STATUS_LABEL: Record<SessionStatus, string> = {
  starting: "Starting",
  working: "Working",
  idle: "Idle",
  needs_attention: "Needs Attention",
  finished: "Finished",
  error: "Error",
};

function isDismissed(status: SessionStatus): boolean {
  return status === "finished" || status === "error";
}

export function SessionCard({ session, isActive, onClick }: SessionCardProps) {
  const cardClass = [
    styles.card,
    isActive ? styles.active : "",
    isDismissed(session.status) ? styles.dismissed : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      className={cardClass}
      onClick={() => onClick(session.id)}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          onClick(session.id);
        }
      }}
    >
      {session.status === "finished" ? (
        <span className={styles.finishedIcon}>&#10003;</span>
      ) : (
        <span
          className={`${styles.statusDot} ${STATUS_DOT_CLASS[session.status]}`}
        />
      )}
      <div className={styles.info}>
        <span className={styles.name}>{session.name}</span>
        <span className={styles.status}>{STATUS_LABEL[session.status]}</span>
      </div>
    </div>
  );
}
```

### Step 3B.2: Create the NewSessionButton component

- [ ] Create `src/components/NewSessionButton/NewSessionButton.module.css`:

```css
/* src/components/NewSessionButton/NewSessionButton.module.css */

.button {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  width: 100%;
  padding: 10px 0;
  border: 1px dashed rgba(255, 255, 255, 0.15);
  border-radius: 6px;
  background: transparent;
  color: #9ca3af;
  font-size: 13px;
  font-family: "SF Mono", "Menlo", "Monaco", monospace;
  cursor: pointer;
  transition: background-color 0.15s ease, color 0.15s ease,
    border-color 0.15s ease;
}

.button:hover {
  background-color: rgba(255, 255, 255, 0.05);
  color: #e5e7eb;
  border-color: rgba(255, 255, 255, 0.25);
}

.plus {
  font-size: 16px;
  font-weight: 300;
  line-height: 1;
}
```

- [ ] Create `src/components/NewSessionButton/NewSessionButton.tsx`:

```typescript
// src/components/NewSessionButton/NewSessionButton.tsx

import styles from "./NewSessionButton.module.css";

interface NewSessionButtonProps {
  onClick: () => void;
}

export function NewSessionButton({ onClick }: NewSessionButtonProps) {
  return (
    <button className={styles.button} onClick={onClick} type="button">
      <span className={styles.plus}>+</span>
      New Session
    </button>
  );
}
```

### Step 3B.3: Create the NewSessionModal component

- [ ] Create `src/components/NewSessionModal/NewSessionModal.module.css`:

```css
/* src/components/NewSessionModal/NewSessionModal.module.css */

.overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}

.modal {
  background: #1e1e2e;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  padding: 24px;
  width: 400px;
  max-width: 90vw;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
}

.title {
  font-size: 16px;
  font-weight: 600;
  color: #e5e7eb;
  margin: 0 0 20px 0;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 16px;
}

.label {
  font-size: 12px;
  font-weight: 500;
  color: #9ca3af;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.input {
  padding: 8px 12px;
  background: #12121a;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  color: #e5e7eb;
  font-size: 14px;
  font-family: "SF Mono", "Menlo", "Monaco", monospace;
  outline: none;
  transition: border-color 0.15s ease;
}

.input:focus {
  border-color: rgba(59, 130, 246, 0.5);
}

.input::placeholder {
  color: #4b5563;
}

.folderRow {
  display: flex;
  align-items: center;
  gap: 8px;
}

.folderPath {
  flex: 1;
  padding: 8px 12px;
  background: #12121a;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  color: #9ca3af;
  font-size: 13px;
  font-family: "SF Mono", "Menlo", "Monaco", monospace;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-height: 36px;
  display: flex;
  align-items: center;
}

.folderPath.hasValue {
  color: #e5e7eb;
}

.browseButton {
  padding: 8px 14px;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  color: #e5e7eb;
  font-size: 13px;
  cursor: pointer;
  white-space: nowrap;
  transition: background-color 0.15s ease;
  flex-shrink: 0;
}

.browseButton:hover {
  background: rgba(255, 255, 255, 0.1);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 24px;
}

.cancelButton {
  padding: 8px 16px;
  background: transparent;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  color: #9ca3af;
  font-size: 13px;
  cursor: pointer;
  transition: background-color 0.15s ease, color 0.15s ease;
}

.cancelButton:hover {
  background: rgba(255, 255, 255, 0.05);
  color: #e5e7eb;
}

.createButton {
  padding: 8px 16px;
  background: #3b82f6;
  border: none;
  border-radius: 6px;
  color: #ffffff;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: background-color 0.15s ease;
}

.createButton:hover {
  background: #2563eb;
}

.createButton:disabled {
  background: #1e3a5f;
  color: #6b7280;
  cursor: not-allowed;
}
```

- [ ] Create `src/components/NewSessionModal/NewSessionModal.tsx`:

```typescript
// src/components/NewSessionModal/NewSessionModal.tsx

import { useState, useEffect, useRef } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import styles from "./NewSessionModal.module.css";

interface NewSessionModalProps {
  isOpen: boolean;
  onClose: () => void;
  onCreate: (name: string, cwd: string) => void;
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
  const inputRef = useRef<HTMLInputElement>(null);

  // Reset form when modal opens; pre-fill directory from last used
  useEffect(() => {
    if (isOpen) {
      setName("");
      setDirectory(lastUsedDirectory);
      // Focus the name input on next tick after render
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen, lastUsedDirectory]);

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

  const handleCreate = () => {
    const trimmedName = name.trim();
    if (!trimmedName || !directory) return;
    onCreate(trimmedName, directory);
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
        ref={(el) => el?.focus()}
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

### Step 3B.4: Create the SessionPanel component

- [ ] Create `src/components/SessionPanel/SessionPanel.module.css`:

```css
/* src/components/SessionPanel/SessionPanel.module.css */

.panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: #16161e;
  border-left: 1px solid rgba(255, 255, 255, 0.06);
  padding: 12px;
  gap: 8px;
}

.header {
  font-size: 11px;
  font-weight: 600;
  color: #6b7280;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  padding: 4px 4px 8px 4px;
}

.sessionList {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
  overflow-y: auto;
}

.sessionList::-webkit-scrollbar {
  width: 4px;
}

.sessionList::-webkit-scrollbar-track {
  background: transparent;
}

.sessionList::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.1);
  border-radius: 2px;
}

.empty {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  color: #4b5563;
  font-size: 13px;
  font-style: italic;
}
```

- [ ] Create `src/components/SessionPanel/SessionPanel.tsx`:

```typescript
// src/components/SessionPanel/SessionPanel.tsx

import type { SessionInfo } from "../../types/session";
import { SessionCard } from "../SessionCard/SessionCard";
import { NewSessionButton } from "../NewSessionButton/NewSessionButton";
import styles from "./SessionPanel.module.css";

interface SessionPanelProps {
  sessions: SessionInfo[];
  activeSessionId: string | null;
  onSessionClick: (id: string) => void;
  onNewSession: () => void;
}

export function SessionPanel({
  sessions,
  activeSessionId,
  onSessionClick,
  onNewSession,
}: SessionPanelProps) {
  return (
    <div className={styles.panel}>
      <div className={styles.header}>Sessions</div>
      <NewSessionButton onClick={onNewSession} />
      {sessions.length === 0 ? (
        <div className={styles.empty}>No active sessions</div>
      ) : (
        <div className={styles.sessionList}>
          {sessions.map((session) => (
            <SessionCard
              key={session.id}
              session={session}
              isActive={session.id === activeSessionId}
              onClick={onSessionClick}
            />
          ))}
        </div>
      )}
    </div>
  );
}
```

### Step 3B.5: Verify components render in isolation

- [ ] Create a quick smoke test at `src/components/SessionPanel/SessionPanel.test.tsx`:

```typescript
// src/components/SessionPanel/SessionPanel.test.tsx

import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SessionPanel } from "./SessionPanel";
import type { SessionInfo } from "../../types/session";

describe("SessionPanel", () => {
  it("shows empty state when no sessions exist", () => {
    render(
      <SessionPanel
        sessions={[]}
        activeSessionId={null}
        onSessionClick={vi.fn()}
        onNewSession={vi.fn()}
      />
    );
    expect(screen.getByText("No active sessions")).toBeTruthy();
  });

  it("renders session cards for each session", () => {
    const sessions: SessionInfo[] = [
      { id: "1", name: "Session A", status: "working", createdAt: Date.now() },
      { id: "2", name: "Session B", status: "idle", createdAt: Date.now() },
    ];
    render(
      <SessionPanel
        sessions={sessions}
        activeSessionId="1"
        onSessionClick={vi.fn()}
        onNewSession={vi.fn()}
      />
    );
    expect(screen.getByText("Session A")).toBeTruthy();
    expect(screen.getByText("Session B")).toBeTruthy();
  });

  it("renders the New Session button", () => {
    render(
      <SessionPanel
        sessions={[]}
        activeSessionId={null}
        onSessionClick={vi.fn()}
        onNewSession={vi.fn()}
      />
    );
    expect(screen.getByText("New Session")).toBeTruthy();
  });
});
```

### Step 3B.6: Run component tests

- [ ] Run the test:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
npx vitest run src/components/SessionPanel/SessionPanel.test.tsx
```

Expected: all 3 tests pass.

---

## Task 3C: Wire It Together

> **Depends on:** 3A + 3B complete  
> **Files to create:** none  
> **Files to modify:** `src/App.tsx`, `src/App.module.css`

### Step 3C.1: Add lastUsedDirectory to the store

- [ ] Add `lastUsedDirectory` state and setter to `src/stores/sessionStore.ts`. Add these to the interface and implementation:

In the `SessionState` interface, add:

```typescript
  lastUsedDirectory: string | null;
  setLastUsedDirectory: (dir: string) => void;
```

In the `create<SessionState>(...)` body, add:

```typescript
  lastUsedDirectory: null,
  setLastUsedDirectory: (dir) => set({ lastUsedDirectory: dir }),
```

And update the `createSession` method to save the directory:

```typescript
  createSession: async (name, cwd) => {
    const id = await invoke<string>("create_session", { name, cwd });
    const session: SessionInfo = {
      id,
      name,
      status: "starting",
      createdAt: Date.now(),
    };
    get().addSession(session);
    get().setActiveSession(id);
    get().setupEventListeners(id);
    set({ lastUsedDirectory: cwd });
  },
```

### Step 3C.1b: Add a test for lastUsedDirectory

- [ ] Add this test to `src/stores/sessionStore.test.ts` inside the `sessionStore` describe block, after the existing `closeSession` tests:

```typescript
  describe("createSession — lastUsedDirectory", () => {
    it("sets lastUsedDirectory after creating a session", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockResolvedValueOnce("dir-test-id");

      const store = useSessionStore.getState();
      await store.createSession("Dir Test", "/projects/my-app");

      const { lastUsedDirectory } = useSessionStore.getState();
      expect(lastUsedDirectory).toBe("/projects/my-app");
    });
  });
```

Also update the `beforeEach` reset to include the new state:

```typescript
  beforeEach(() => {
    useSessionStore.setState({
      sessions: new Map(),
      activeSessionId: null,
      lastUsedDirectory: null,
    });
  });
```

### Step 3C.2: Update App.tsx to integrate store and components

- [ ] Replace `src/App.tsx` with the wired-up version:

```typescript
// src/App.tsx

import { useState, useMemo } from "react";
import { useSessionStore } from "./stores/sessionStore";
import { TitleBar } from "./components/TitleBar/TitleBar";
import { SessionPanel } from "./components/SessionPanel/SessionPanel";
import { NewSessionModal } from "./components/NewSessionModal/NewSessionModal";
import { XTermInstance } from "./components/XTermInstance/XTermInstance";
import styles from "./App.module.css";

export function App() {
  const [isModalOpen, setIsModalOpen] = useState(false);

  const sessions = useSessionStore((s) => s.sessions);
  const activeSessionId = useSessionStore((s) => s.activeSessionId);
  const lastUsedDirectory = useSessionStore((s) => s.lastUsedDirectory);
  const setActiveSession = useSessionStore((s) => s.setActiveSession);
  const createSession = useSessionStore((s) => s.createSession);

  // Convert Map to sorted array (newest first) for the panel
  const sessionList = useMemo(() => {
    return Array.from(sessions.values()).sort(
      (a, b) => b.createdAt - a.createdAt
    );
  }, [sessions]);

  const handleNewSession = () => {
    setIsModalOpen(true);
  };

  const handleCreateSession = async (name: string, cwd: string) => {
    setIsModalOpen(false);
    await createSession(name, cwd);
  };

  return (
    <div className={styles.app}>
      <TitleBar />
      <div className={styles.content}>
        <div className={styles.terminalArea}>
          {sessionList.length === 0 && (
            <div className={styles.emptyTerminal}>
              <p className={styles.emptyText}>
                Create a session to get started
              </p>
              <button
                className={styles.emptyButton}
                onClick={handleNewSession}
                type="button"
              >
                + New Session
              </button>
            </div>
          )}
          {sessionList.map((session) => (
            <XTermInstance
              key={session.id}
              sessionId={session.id}
              isVisible={session.id === activeSessionId}
            />
          ))}
        </div>
        <SessionPanel
          sessions={sessionList}
          activeSessionId={activeSessionId}
          onSessionClick={setActiveSession}
          onNewSession={handleNewSession}
        />
      </div>
      <NewSessionModal
        isOpen={isModalOpen}
        onClose={() => setIsModalOpen(false)}
        onCreate={handleCreateSession}
        lastUsedDirectory={lastUsedDirectory}
      />
    </div>
  );
}
```

### Step 3C.3: Update App.module.css

- [ ] Replace `src/App.module.css` with layout styles:

```css
/* src/App.module.css */

.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #12121a;
  color: #e5e7eb;
  overflow: hidden;
}

.content {
  display: flex;
  flex: 1;
  min-height: 0;
}

.terminalArea {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.emptyTerminal {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
}

.emptyText {
  color: #4b5563;
  font-size: 15px;
  font-family: "SF Mono", "Menlo", "Monaco", monospace;
}

.emptyButton {
  padding: 10px 20px;
  background: rgba(59, 130, 246, 0.15);
  border: 1px solid rgba(59, 130, 246, 0.3);
  border-radius: 6px;
  color: #3b82f6;
  font-size: 14px;
  font-family: "SF Mono", "Menlo", "Monaco", monospace;
  cursor: pointer;
  transition: background-color 0.15s ease;
}

.emptyButton:hover {
  background: rgba(59, 130, 246, 0.25);
}
```

### Step 3C.4: Initialize store from backend on app load

- [ ] Create `src/hooks/useInitializeSessions.ts` to hydrate the store from existing backend sessions on app startup:

```typescript
// src/hooks/useInitializeSessions.ts

import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "../stores/sessionStore";
import type { SessionInfo } from "../types/session";

export function useInitializeSessions() {
  const addSession = useSessionStore((s) => s.addSession);
  const setupEventListeners = useSessionStore((s) => s.setupEventListeners);

  useEffect(() => {
    async function init() {
      try {
        const existing = await invoke<SessionInfo[]>("list_sessions");
        for (const session of existing) {
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

- [ ] Add the hook to `src/App.tsx` — add this import and call inside `App()`:

```typescript
import { useInitializeSessions } from "./hooks/useInitializeSessions";

// Inside App(), at the top of the function body:
useInitializeSessions();
```

### Step 3C.5: End-to-end flow verification

- [ ] Verify no TypeScript errors (use `tsc --noEmit` since `npm run build` only builds the frontend, not the full Tauri app):

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
npx tsc --noEmit
```

Expected: type-checking succeeds with no errors.

- [ ] Run the full test suite:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
npx vitest run
```

Expected: all store and component tests pass.

### Step 3C.6: Manual smoke test

- [ ] Start the Tauri dev server and verify the end-to-end flow:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
npm run tauri dev
```

Verify:
1. App opens with empty state ("Create a session to get started" message)
2. Click "+ New Session" in the sidebar -- modal appears
3. Type a session name (e.g. "test-session")
4. Click "Browse" -- native macOS folder picker opens
5. Select a git repo directory and confirm
6. Click "Create" -- modal closes, session card appears in sidebar with "Starting" status, terminal area shows xterm.js
7. Terminal streams Claude Code output
8. Session card status updates (Starting -> Working as output streams)

---

## File Summary

### New files created in this wave:

| File | Task |
|------|------|
| `src/types/session.ts` | 3A |
| `src/stores/sessionStore.ts` | 3A |
| `src/stores/sessionStore.test.ts` | 3A |
| `src/components/SessionCard/SessionCard.tsx` | 3B |
| `src/components/SessionCard/SessionCard.module.css` | 3B |
| `src/components/NewSessionButton/NewSessionButton.tsx` | 3B |
| `src/components/NewSessionButton/NewSessionButton.module.css` | 3B |
| `src/components/NewSessionModal/NewSessionModal.tsx` | 3B |
| `src/components/NewSessionModal/NewSessionModal.module.css` | 3B |
| `src/components/SessionPanel/SessionPanel.tsx` | 3B |
| `src/components/SessionPanel/SessionPanel.module.css` | 3B |
| `src/components/SessionPanel/SessionPanel.test.tsx` | 3B |
| `src/hooks/useInitializeSessions.ts` | 3C |

### Files modified in this wave:

| File | Task | Change |
|------|------|--------|
| `src-tauri/Cargo.toml` | 3B | Add `tauri-plugin-dialog` dependency |
| `src-tauri/tauri.conf.json` | 3B | Register dialog plugin |
| `src-tauri/src/lib.rs` | 3B | Register dialog plugin in Tauri builder |
| `src/stores/sessionStore.ts` | 3A, 3C | Add `renameSession` action; add `lastUsedDirectory` state + setter, update `createSession` to persist dir |
| `src/stores/sessionStore.test.ts` | 3C | Add `lastUsedDirectory` test case, update `beforeEach` reset |
| `src/App.tsx` | 3C | Integrate store, session panel, modal, terminal area (render ALL terminals with CSS show/hide via `isVisible` prop), initialization hook |
| `src/App.module.css` | 3C | Two-pane layout styles, empty terminal state |
