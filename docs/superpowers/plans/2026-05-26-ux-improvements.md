# UX Improvements Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add session cycling keybindings, default session names, quit confirmation dialog, and a settings modal for configurable naming.

**Architecture:** Four independent UI features touching the React frontend only (no Rust backend changes). Features 1 & 3 are fully independent. Feature 4 builds on Feature 2's `getDefaultSessionName` helper. All follow existing patterns: Zustand store, CSS modules, portal-rendered modals.

**Tech Stack:** React, TypeScript, Zustand, CSS Modules, Vitest, Tauri window API

**Spec:** `docs/superpowers/specs/2026-05-26-ux-improvements-design.md`

---

## File Map

| Action | File | Responsibility |
|--------|------|---------------|
| Modify | `src/hooks/useGlobalKeybindings.ts` | Add Cmd+Shift+[/] cycling and Cmd+, settings shortcut |
| Modify | `src/App.tsx` | Wire cycling callbacks, quit confirm, settings modal |
| Modify | `src/components/NewSessionModal/NewSessionModal.tsx` | Placeholder default names, optional name field |
| Modify | `src/stores/sessionStore.ts` | Add `showQuitConfirm` state |
| Modify | `src/hooks/useSaveOnClose.ts` | Simplify to just show quit confirm dialog |
| Modify | `src/components/TitleBar/TitleBar.tsx` | Add gear icon + `onSettingsClick` prop |
| Modify | `src/components/TitleBar/TitleBar.module.css` | Right-side controls layout |
| Create | `src/components/QuitConfirmDialog/QuitConfirmDialog.tsx` | Quit confirmation dialog |
| Create | `src/components/QuitConfirmDialog/QuitConfirmDialog.module.css` | Quit dialog styles (copy CloseConfirmDialog) |
| Create | `src/components/SettingsModal/SettingsModal.tsx` | Settings modal with default name pattern |
| Create | `src/components/SettingsModal/SettingsModal.module.css` | Settings modal styles (follow NewSessionModal) |
| Create | `src/hooks/sessionCycling.test.ts` | Tests for cycling logic (pure function) |
| Create | `src/components/NewSessionModal/defaultSessionName.test.ts` | Tests for name generation |

---

## Chunk 1: Session Cycling Keybindings

### Task 1: Add cycling callbacks to useGlobalKeybindings

**Files:**
- Modify: `src/hooks/useGlobalKeybindings.ts`
- Create: `src/hooks/sessionCycling.test.ts`

- [ ] **Step 1: Write tests for the cycling logic**

Create `src/hooks/sessionCycling.test.ts`:

```ts
import { describe, it, expect } from "vitest";

function getCycledIndex(
  direction: "prev" | "next",
  currentId: string | null,
  orderedIds: string[],
): number | null {
  if (orderedIds.length <= 1 || !currentId) return null;
  const idx = orderedIds.indexOf(currentId);
  if (idx === -1) return null;
  if (direction === "prev") {
    return idx <= 0 ? orderedIds.length - 1 : idx - 1;
  }
  return idx >= orderedIds.length - 1 ? 0 : idx + 1;
}

describe("getCycledIndex", () => {
  const ids = ["a", "b", "c", "d"];

  it("returns null with 0 sessions", () => {
    expect(getCycledIndex("next", "a", [])).toBeNull();
  });

  it("returns null with 1 session", () => {
    expect(getCycledIndex("next", "a", ["a"])).toBeNull();
  });

  it("returns null with no active session", () => {
    expect(getCycledIndex("next", null, ids)).toBeNull();
  });

  it("cycles next from middle", () => {
    expect(getCycledIndex("next", "b", ids)).toBe(2);
  });

  it("wraps next from last to first", () => {
    expect(getCycledIndex("next", "d", ids)).toBe(0);
  });

  it("cycles prev from middle", () => {
    expect(getCycledIndex("prev", "c", ids)).toBe(1);
  });

  it("wraps prev from first to last", () => {
    expect(getCycledIndex("prev", "a", ids)).toBe(3);
  });

  it("returns null for unknown activeId", () => {
    expect(getCycledIndex("next", "unknown", ids)).toBeNull();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/hooks/sessionCycling.test.ts`
Expected: PASS (pure function is defined inline in test for now)

- [ ] **Step 3: Extract getCycledIndex as a utility and import it**

Create the actual utility in `src/hooks/useGlobalKeybindings.ts` — add and export the `getCycledIndex` function:

```ts
export function getCycledIndex(
  direction: "prev" | "next",
  currentId: string | null,
  orderedIds: string[],
): number | null {
  if (orderedIds.length <= 1 || !currentId) return null;
  const idx = orderedIds.indexOf(currentId);
  if (idx === -1) return null;
  if (direction === "prev") {
    return idx <= 0 ? orderedIds.length - 1 : idx - 1;
  }
  return idx >= orderedIds.length - 1 ? 0 : idx + 1;
}
```

Update the test file to import from the hook file instead of defining inline:

```ts
import { getCycledIndex } from "./useGlobalKeybindings";
```

Remove the inline function definition from the test.

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/hooks/sessionCycling.test.ts`
Expected: All 8 tests PASS

- [ ] **Step 5: Add cycling keybindings to the hook**

Modify `src/hooks/useGlobalKeybindings.ts`:

```ts
interface GlobalKeybindingActions {
  onNewSession: () => void;
  onCloseActiveSession: () => void;
  onSwitchToSession: (index: number) => void;
  onCyclePrev: () => void;
  onCycleNext: () => void;
  onOpenSettings: () => void;
}

export function useGlobalKeybindings({
  onNewSession,
  onCloseActiveSession,
  onSwitchToSession,
  onCyclePrev,
  onCycleNext,
  onOpenSettings,
}: GlobalKeybindingActions) {
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (!e.metaKey) return;

      if (e.key === "t") {
        e.preventDefault();
        onNewSession();
      }

      if (e.key === "w") {
        e.preventDefault();
        onCloseActiveSession();
      }

      if (e.key === "," && !e.shiftKey) {
        e.preventDefault();
        onOpenSettings();
      }

      if (e.shiftKey && e.code === "BracketLeft") {
        e.preventDefault();
        onCyclePrev();
      }

      if (e.shiftKey && e.code === "BracketRight") {
        e.preventDefault();
        onCycleNext();
      }

      const digit = parseInt(e.key, 10);
      if (digit >= 1 && digit <= 9) {
        e.preventDefault();
        onSwitchToSession(digit - 1);
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onNewSession, onCloseActiveSession, onSwitchToSession, onCyclePrev, onCycleNext, onOpenSettings]);
}
```

- [ ] **Step 6: Wire cycling callbacks in App.tsx**

In `src/App.tsx`, add the import and cycling callbacks. The callbacks use the tested `getCycledIndex` utility to ensure the same logic is used in production and tests:

```ts
import { getCycledIndex } from "./hooks/useGlobalKeybindings";
```

```ts
const handleCyclePrev = useCallback(() => {
  const idx = getCycledIndex("prev", activeSessionId, orderedSessionIds);
  if (idx !== null) setActiveSession(orderedSessionIds[idx]);
}, [orderedSessionIds, activeSessionId, setActiveSession]);

const handleCycleNext = useCallback(() => {
  const idx = getCycledIndex("next", activeSessionId, orderedSessionIds);
  if (idx !== null) setActiveSession(orderedSessionIds[idx]);
}, [orderedSessionIds, activeSessionId, setActiveSession]);
```

Update the `useGlobalKeybindings` call (settings handler will be a no-op for now, wired in Task 7):

```ts
useGlobalKeybindings({
  onNewSession: handleNewSession,
  onCloseActiveSession: handleCloseActiveSession,
  onSwitchToSession: handleSwitchToSession,
  onCyclePrev: handleCyclePrev,
  onCycleNext: handleCycleNext,
  onOpenSettings: () => {}, // wired in Task 7
});
```

- [ ] **Step 7: Run all tests**

Run: `npx vitest run`
Expected: All tests PASS

- [ ] **Step 8: Commit**

```bash
git add src/hooks/useGlobalKeybindings.ts src/hooks/sessionCycling.test.ts src/App.tsx
git commit -m "feat: add Cmd+Shift+[/] session cycling keybindings"
```

---

## Chunk 2: Default Session Names

### Task 2: Add default session name generation

**Files:**
- Modify: `src/components/NewSessionModal/NewSessionModal.tsx`
- Create: `src/components/NewSessionModal/defaultSessionName.test.ts`

- [ ] **Step 1: Write tests for default name generation**

Create `src/components/NewSessionModal/defaultSessionName.test.ts`:

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { getDefaultSessionName, getNextSessionNumber, _resetCounterForTesting } from "./NewSessionModal";

describe("getDefaultSessionName", () => {
  beforeEach(() => {
    _resetCounterForTesting();
    localStorage.clear();
  });

  it("returns 'Session 1' for first session with no custom pattern", () => {
    expect(getDefaultSessionName(1)).toBe("Session 1");
  });

  it("returns 'Session 5' for n=5", () => {
    expect(getDefaultSessionName(5)).toBe("Session 5");
  });

  it("uses custom pattern from localStorage", () => {
    localStorage.setItem("ao-default-session-name", "Agent {n}");
    expect(getDefaultSessionName(3)).toBe("Agent 3");
  });

  it("uses pattern as-is when no {n} token", () => {
    localStorage.setItem("ao-default-session-name", "My Task");
    expect(getDefaultSessionName(7)).toBe("My Task");
  });

  it("falls back to default when localStorage is empty string", () => {
    localStorage.setItem("ao-default-session-name", "");
    expect(getDefaultSessionName(2)).toBe("Session 2");
  });
});

describe("getNextSessionNumber", () => {
  beforeEach(() => {
    _resetCounterForTesting();
  });

  it("starts at 1", () => {
    expect(getNextSessionNumber()).toBe(1);
  });

  it("increments on each call", () => {
    expect(getNextSessionNumber()).toBe(1);
    expect(getNextSessionNumber()).toBe(2);
    expect(getNextSessionNumber()).toBe(3);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/components/NewSessionModal/defaultSessionName.test.ts`
Expected: FAIL — functions not exported yet

- [ ] **Step 3: Add name generation logic to NewSessionModal**

In `src/components/NewSessionModal/NewSessionModal.tsx`, add at the top (after imports):

```ts
const DEFAULT_NAME_STORAGE_KEY = "ao-default-session-name";
const DEFAULT_PATTERN = "Session {n}";

let sessionCounter = 0;

export function getNextSessionNumber(): number {
  return ++sessionCounter;
}

export function peekNextSessionNumber(): number {
  return sessionCounter + 1;
}

export function getDefaultSessionName(n: number): string {
  const pattern = localStorage.getItem(DEFAULT_NAME_STORAGE_KEY) || DEFAULT_PATTERN;
  return pattern.replaceAll("{n}", String(n));
}

// Only for tests
export function _resetCounterForTesting(): void {
  sessionCounter = 0;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/components/NewSessionModal/defaultSessionName.test.ts`
Expected: All 7 tests PASS

- [ ] **Step 5: Update NewSessionModal to use placeholder names**

In `src/components/NewSessionModal/NewSessionModal.tsx`, modify the component:

1. Add state for the default name placeholder:
```ts
const [defaultName, setDefaultName] = useState("");
```

2. In the `useEffect` that runs when `isOpen` changes, set the placeholder:
```ts
useEffect(() => {
  if (isOpen) {
    setName("");
    setDefaultName(getDefaultSessionName(peekNextSessionNumber()));
    setDirectory(lastUsedDirectory);
    // ... rest unchanged
  }
}, [isOpen, lastUsedDirectory]);
```

3. Update the name input to use `placeholder`:
```tsx
<input
  ref={inputRef}
  id="session-name"
  className={styles.input}
  type="text"
  placeholder={defaultName}
  value={name}
  onChange={(e) => setName(e.target.value)}
  autoComplete="off"
/>
```

4. Update `handleCreate` to use default name when field is empty:
```ts
const handleCreate = () => {
  const trimmedName = name.trim();
  const finalName = trimmedName || getDefaultSessionName(getNextSessionNumber());
  if (!directory) return;
  localStorage.setItem(STORAGE_KEY, sessionMode);
  onCreate(finalName, directory, sessionMode, effectivePullLatest, isGitRepo ?? false);
};
```

5. Update `handleKeyDown` Enter guard:
```ts
if (e.key === "Enter" && directory) {
  handleCreate();
}
```

6. Update `isValid` — only directory required:
```ts
const isValid = directory !== null;
```

- [ ] **Step 6: Run all tests**

Run: `npx vitest run`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add src/components/NewSessionModal/NewSessionModal.tsx src/components/NewSessionModal/defaultSessionName.test.ts
git commit -m "feat: add default placeholder session names with auto-incrementing counter"
```

---

## Chunk 3: Quit Application Confirmation

### Task 3: Add showQuitConfirm state to store

**Files:**
- Modify: `src/stores/sessionStore.ts`

- [ ] **Step 1: Add quit confirm state to SessionState interface**

In `src/stores/sessionStore.ts`, add to the `SessionState` interface:

```ts
// Quit confirmation
showQuitConfirm: boolean;
setShowQuitConfirm: (show: boolean) => void;
```

- [ ] **Step 2: Add initial state and setter in the store**

In the `create<SessionState>` call, add:

```ts
showQuitConfirm: false,
setShowQuitConfirm: (show) => set({ showQuitConfirm: show }),
```

- [ ] **Step 3: Run all tests**

Run: `npx vitest run`
Expected: All tests PASS (no existing tests break)

- [ ] **Step 4: Commit**

```bash
git add src/stores/sessionStore.ts
git commit -m "feat: add showQuitConfirm state to session store"
```

### Task 4: Create QuitConfirmDialog component

**Files:**
- Create: `src/components/QuitConfirmDialog/QuitConfirmDialog.tsx`
- Create: `src/components/QuitConfirmDialog/QuitConfirmDialog.module.css`

- [ ] **Step 1: Create the CSS file**

Create `src/components/QuitConfirmDialog/QuitConfirmDialog.module.css` — copy from `CloseConfirmDialog.module.css` exactly:

```css
.overlay {
  position: fixed;
  inset: 0;
  background-color: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2000;
}
.dialog {
  background-color: #1f2937;
  border: 1px solid #374151;
  border-radius: 8px;
  padding: 24px;
  max-width: 400px;
  width: 90%;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
}
.title {
  margin: 0 0 12px;
  font-size: 16px;
  font-weight: 600;
  color: #f3f4f6;
}
.message {
  margin: 0 0 20px;
  font-size: 14px;
  color: #9ca3af;
  line-height: 1.5;
}
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.cancelBtn {
  padding: 8px 16px;
  border: 1px solid #374151;
  border-radius: 6px;
  background: none;
  color: #e5e7eb;
  font-size: 13px;
  cursor: pointer;
}
.cancelBtn:hover {
  background-color: #374151;
}
.confirmBtn {
  padding: 8px 16px;
  border: none;
  border-radius: 6px;
  background-color: #ef4444;
  color: white;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
}
.confirmBtn:hover {
  background-color: #dc2626;
}
```

- [ ] **Step 2: Create the component**

Create `src/components/QuitConfirmDialog/QuitConfirmDialog.tsx`:

```tsx
import styles from "./QuitConfirmDialog.module.css";

interface QuitConfirmDialogProps {
  onConfirm: () => void;
  onCancel: () => void;
}

export function QuitConfirmDialog({ onConfirm, onCancel }: QuitConfirmDialogProps) {
  return (
    <div className={styles.overlay} onClick={onCancel}>
      <div className={styles.dialog} onClick={(e) => e.stopPropagation()}>
        <h3 className={styles.title}>Quit Agent Orchestrator?</h3>
        <p className={styles.message}>
          All running sessions will be terminated. Session state will be saved.
        </p>
        <div className={styles.actions}>
          <button className={styles.cancelBtn} onClick={onCancel}>Cancel</button>
          <button className={styles.confirmBtn} onClick={onConfirm}>Quit</button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Commit**

```bash
git add src/components/QuitConfirmDialog/
git commit -m "feat: create QuitConfirmDialog component"
```

### Task 5: Wire quit confirmation into useSaveOnClose and App

**Files:**
- Modify: `src/hooks/useSaveOnClose.ts`
- Modify: `src/App.tsx`

- [ ] **Step 1: Simplify useSaveOnClose to show confirm dialog**

Replace the contents of `src/hooks/useSaveOnClose.ts` with both the simplified hook and the extracted save-and-quit helper. The `invoke` import is retained (static, not dynamic) since `saveSessionsAndQuit` needs it:

```ts
import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "../stores/sessionStore";

export function useSaveOnClose() {
  useEffect(() => {
    const appWindow = getCurrentWindow();

    const unlisten = appWindow.onCloseRequested(async (event) => {
      event.preventDefault();
      useSessionStore.getState().setShowQuitConfirm(true);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);
}

export async function saveSessionsAndQuit() {
  const appWindow = getCurrentWindow();

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

    const allScrollbacks = (window as any).__aoGetAllScrollbacks?.() ?? {};

    for (const session of sessions) {
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

  await appWindow.destroy();
}
```

- [ ] **Step 3: Wire QuitConfirmDialog in App.tsx**

In `src/App.tsx`, add imports:
```ts
import { QuitConfirmDialog } from "./components/QuitConfirmDialog/QuitConfirmDialog";
import { saveSessionsAndQuit } from "./hooks/useSaveOnClose";
```

Add store selectors:
```ts
const showQuitConfirm = useSessionStore((s) => s.showQuitConfirm);
const setShowQuitConfirm = useSessionStore((s) => s.setShowQuitConfirm);
```

Add quit handler:
```ts
const handleConfirmQuit = useCallback(async () => {
  setShowQuitConfirm(false);
  await saveSessionsAndQuit();
}, [setShowQuitConfirm]);
```

Add the dialog portal in JSX (after the existing `CloseConfirmDialog` portal):
```tsx
{showQuitConfirm &&
  createPortal(
    <QuitConfirmDialog
      onConfirm={handleConfirmQuit}
      onCancel={() => setShowQuitConfirm(false)}
    />,
    document.body
  )}
```

- [ ] **Step 4: Run all tests**

Run: `npx vitest run`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/hooks/useSaveOnClose.ts src/App.tsx
git commit -m "feat: add quit confirmation dialog on app close"
```

---

## Chunk 4: Settings Modal and Configurable Names

### Task 6: Create SettingsModal component

**Files:**
- Create: `src/components/SettingsModal/SettingsModal.tsx`
- Create: `src/components/SettingsModal/SettingsModal.module.css`

- [ ] **Step 1: Create the CSS file**

Create `src/components/SettingsModal/SettingsModal.module.css` — follow `NewSessionModal.module.css` patterns:

```css
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

.hint {
  font-size: 11px;
  color: #6b7280;
  margin-top: 2px;
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

.saveButton {
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

.saveButton:hover {
  background: #2563eb;
}
```

- [ ] **Step 2: Create the SettingsModal component**

Create `src/components/SettingsModal/SettingsModal.tsx`:

```tsx
import { useState, useEffect, useRef } from "react";
import styles from "./SettingsModal.module.css";

const STORAGE_KEY = "ao-default-session-name";
const DEFAULT_PATTERN = "Session {n}";

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export function SettingsModal({ isOpen, onClose }: SettingsModalProps) {
  const [namePattern, setNamePattern] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      setNamePattern(localStorage.getItem(STORAGE_KEY) ?? "");
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const handleSave = () => {
    const trimmed = namePattern.trim();
    if (trimmed) {
      localStorage.setItem(STORAGE_KEY, trimmed);
    } else {
      localStorage.removeItem(STORAGE_KEY);
    }
    onClose();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose();
    }
    if (e.key === "Enter") {
      handleSave();
    }
  };

  return (
    <div className={styles.overlay} onClick={onClose}>
      <div
        className={styles.modal}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
        tabIndex={-1}
      >
        <h2 className={styles.title}>Settings</h2>

        <div className={styles.field}>
          <label className={styles.label} htmlFor="name-pattern">
            Default Session Name
          </label>
          <input
            ref={inputRef}
            id="name-pattern"
            className={styles.input}
            type="text"
            placeholder={DEFAULT_PATTERN}
            value={namePattern}
            onChange={(e) => setNamePattern(e.target.value)}
            autoComplete="off"
          />
          <span className={styles.hint}>
            Use {"{n}"} for auto-incrementing number
          </span>
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
            className={styles.saveButton}
            onClick={handleSave}
            type="button"
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Commit**

```bash
git add src/components/SettingsModal/
git commit -m "feat: create SettingsModal component for configurable session names"
```

### Task 7: Add gear icon to TitleBar

**Files:**
- Modify: `src/components/TitleBar/TitleBar.tsx`
- Modify: `src/components/TitleBar/TitleBar.module.css`

- [ ] **Step 1: Update TitleBar CSS for balanced layout**

Add to `src/components/TitleBar/TitleBar.module.css`:

```css
.rightControls {
  display: flex;
  align-items: center;
}

.settingsButton {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 0;
  opacity: 0.5;
  transition: opacity 0.15s;
}

.settingsButton:hover {
  opacity: 1;
  background: rgba(255, 255, 255, 0.06);
}
```

- [ ] **Step 2: Update TitleBar component with gear icon and onSettingsClick prop**

Modify `src/components/TitleBar/TitleBar.tsx`:

```tsx
import { getCurrentWindow } from "@tauri-apps/api/window";
import styles from "./TitleBar.module.css";

interface TitleBarProps {
  onSettingsClick: () => void;
}

export function TitleBar({ onSettingsClick }: TitleBarProps) {
  const appWindow = getCurrentWindow();

  return (
    <div className={styles.titleBar} data-tauri-drag-region>
      <div className={styles.windowControls}>
        <button
          className={`${styles.trafficLight} ${styles.close}`}
          aria-label="Close"
          onClick={() => appWindow.close()}
        >
          <svg width="6" height="6" viewBox="0 0 6 6">
            <line x1="0" y1="0" x2="6" y2="6" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
            <line x1="6" y1="0" x2="0" y2="6" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
          </svg>
        </button>
        <button
          className={`${styles.trafficLight} ${styles.minimize}`}
          aria-label="Minimize"
          onClick={() => appWindow.minimize()}
        >
          <svg width="8" height="2" viewBox="0 0 8 2">
            <line x1="0" y1="1" x2="8" y2="1" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
          </svg>
        </button>
        <button
          className={`${styles.trafficLight} ${styles.maximize}`}
          aria-label="Maximize"
          onClick={() => appWindow.toggleMaximize()}
        >
          <svg width="6" height="6" viewBox="0 0 6 6">
            <path d="M0.5 3.5 L0.5 0.5 L3.5 0.5" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
            <path d="M5.5 2.5 L5.5 5.5 L2.5 5.5" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </button>
      </div>
      <div className={styles.title} data-tauri-drag-region>
        Agent Orchestrator
      </div>
      <div className={styles.rightControls}>
        <button
          className={styles.settingsButton}
          aria-label="Settings"
          onClick={onSettingsClick}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="3" />
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
          </svg>
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Commit**

```bash
git add src/components/TitleBar/TitleBar.tsx src/components/TitleBar/TitleBar.module.css
git commit -m "feat: add settings gear icon to title bar"
```

### Task 8: Wire SettingsModal and Cmd+, shortcut in App.tsx

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: Add settings modal state and imports**

In `src/App.tsx`, add imports:
```ts
import { SettingsModal } from "./components/SettingsModal/SettingsModal";
```

Add state:
```ts
const [isSettingsOpen, setIsSettingsOpen] = useState(false);
```

Add callback:
```ts
const handleOpenSettings = useCallback(() => {
  setIsSettingsOpen(true);
}, []);
```

- [ ] **Step 2: Wire settings into keybindings and TitleBar**

Update `useGlobalKeybindings` call — replace the no-op `onOpenSettings`:
```ts
useGlobalKeybindings({
  onNewSession: handleNewSession,
  onCloseActiveSession: handleCloseActiveSession,
  onSwitchToSession: handleSwitchToSession,
  onCyclePrev: handleCyclePrev,
  onCycleNext: handleCycleNext,
  onOpenSettings: handleOpenSettings,
});
```

Update `TitleBar` usage:
```tsx
<TitleBar onSettingsClick={handleOpenSettings} />
```

Add `SettingsModal` to JSX (after `NewSessionModal`):
```tsx
<SettingsModal
  isOpen={isSettingsOpen}
  onClose={() => setIsSettingsOpen(false)}
/>
```

- [ ] **Step 3: Run all tests**

Run: `npx vitest run`
Expected: All tests PASS

- [ ] **Step 4: Commit**

```bash
git add src/App.tsx
git commit -m "feat: wire settings modal with Cmd+, shortcut and title bar gear icon"
```

---

## Chunk 5: Final Verification

### Task 9: Run full test suite and manual verification

- [ ] **Step 1: Run all frontend tests**

Run: `npx vitest run`
Expected: All tests PASS

- [ ] **Step 2: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All tests PASS (no backend changes, just verifying nothing broke)

- [ ] **Step 3: Build check**

Run: `npx tsc --noEmit`
Expected: No TypeScript errors

- [ ] **Step 4: Final commit (if any fixes needed)**

Only if previous steps required fixes.

- [ ] **Step 5: Update keyboard shortcuts docs**

In `docs/keyboard-shortcuts.md`, add the new shortcuts:

| Shortcut | Action |
|---|---|
| `Cmd+Shift+[` | Previous session |
| `Cmd+Shift+]` | Next session |
| `Cmd+,` | Open Settings |

- [ ] **Step 6: Commit docs update**

```bash
git add docs/keyboard-shortcuts.md
git commit -m "docs: add new keyboard shortcuts to documentation"
```
