# Session Rename Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow users to rename sessions via double-click on the session name or right-click context menu.

**Architecture:** Purely frontend — add inline editing to SessionCard, thread `onRename` prop through ProjectGroup from SessionPanel. The backend `renameSession` Zustand action and `rename_session` Tauri IPC command already exist.

**Tech Stack:** React, TypeScript, CSS Modules, Vitest + @testing-library/react

---

## Chunk 1: Session Rename

### Task 1: Add `nameInput` CSS class

**Files:**
- Modify: `src/components/SessionCard/SessionCard.module.css`

- [ ] **Step 1: Add `.nameInput` class to SessionCard.module.css**

Add after the existing `.name` rule (after line 76):

```css
.nameInput {
  font-size: 13px;
  font-weight: 500;
  color: #e5e7eb;
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(99, 102, 241, 0.5);
  border-radius: 4px;
  padding: 0 4px;
  outline: none;
  font-family: inherit;
  width: 100%;
  user-select: text;
}
```

- [ ] **Step 2: Commit**

```bash
git add src/components/SessionCard/SessionCard.module.css
git commit -m "style: add nameInput CSS class for inline session rename"
```

---

### Task 2: Add inline rename to SessionCard

**Files:**
- Modify: `src/components/SessionCard/SessionCard.tsx`

- [ ] **Step 1: Add `onRename` prop and `isEditing` state**

In the `SessionCardProps` interface, add:
```typescript
onRename?: (id: string, name: string) => void;
```

In the component destructuring, add `onRename`:
```typescript
export function SessionCard({ session, isActive, onClick, onClose, onDismiss, onRename }: SessionCardProps) {
```

Add state after the existing `useState` calls:
```typescript
const [isEditing, setIsEditing] = useState(false);
```

- [ ] **Step 2: Add the `handleRename` save function**

Add this function inside the component, after the existing `getContextMenuItems`:

```typescript
const savingRef = useRef(false);

function handleRename(newName: string) {
  if (savingRef.current) return;
  savingRef.current = true;
  const trimmed = newName.trim();
  if (trimmed && trimmed.length <= 50 && trimmed !== session.name) {
    onRename?.(session.id, trimmed);
  }
  setIsEditing(false);
}
```

Note: The `savingRef` guard prevents double-invocation when Enter triggers `handleRename` and the subsequent input unmount fires a blur event. Also add `useRef` to the React import on line 1.

- [ ] **Step 3: Replace name span with conditional input**

Replace line 98:
```tsx
<span className={styles.name}>{session.name}</span>
```

With:
```tsx
{isEditing ? (
  <input
    className={styles.nameInput}
    defaultValue={session.name}
    maxLength={50}
    autoFocus
    onFocus={(e) => e.target.select()}
    onKeyDown={(e) => {
      e.stopPropagation();
      if (e.key === "Enter") {
        handleRename(e.currentTarget.value);
      } else if (e.key === "Escape") {
        setIsEditing(false);
      }
    }}
    onBlur={(e) => handleRename(e.target.value)}
    onClick={(e) => e.stopPropagation()}
  />
) : (
  <span
    className={styles.name}
    onDoubleClick={(e) => {
      e.stopPropagation();
      setIsEditing(true);
    }}
  >
    {session.name}
  </span>
)}
```

- [ ] **Step 4: Add "Rename" to context menu**

In `getContextMenuItems()`, add a "Rename" item at the beginning of both branches. Replace the entire function:

```typescript
function getContextMenuItems() {
  const items = [
    {
      label: "Rename",
      onClick: () => setIsEditing(true),
    },
  ];
  if (!isRunning(session.status)) {
    items.push({
      label: "Dismiss",
      onClick: () => setShowCloseConfirm(true),
    });
  } else {
    items.push({
      label: "Close Session",
      danger: true,
      onClick: () => setShowCloseConfirm(true),
    } as { label: string; onClick: () => void; danger?: boolean });
  }
  return items;
}
```

Note: The `as` cast is needed because the first item in the array doesn't have `danger`, so TypeScript narrows the array element type. Alternatively, add `danger: false` to the Rename item to keep types consistent:

```typescript
function getContextMenuItems() {
  const renameItem = { label: "Rename", onClick: () => setIsEditing(true) };
  if (!isRunning(session.status)) {
    return [
      renameItem,
      { label: "Dismiss", onClick: () => setShowCloseConfirm(true) },
    ];
  }
  return [
    renameItem,
    { label: "Close Session", danger: true, onClick: () => setShowCloseConfirm(true) },
  ];
}
```

Use this second version (cleaner).

- [ ] **Step 5: Commit**

```bash
git add src/components/SessionCard/SessionCard.tsx
git commit -m "feat: add inline rename to SessionCard with double-click and context menu"
```

---

### Task 3: Thread `onRename` through ProjectGroup

**Files:**
- Modify: `src/components/ProjectGroup/ProjectGroup.tsx`

- [ ] **Step 1: Add `onRename` to ProjectGroupProps**

Add to the interface:
```typescript
onRename?: (id: string, name: string) => void;
```

Add to the destructured props:
```typescript
export function ProjectGroup({
  projectName,
  sessions,
  activeSessionId,
  isCollapsed,
  onToggleCollapse,
  onSessionClick,
  onClose,
  onDismiss,
  onRename,
}: ProjectGroupProps) {
```

- [ ] **Step 2: Pass `onRename` to SessionCard**

Add `onRename={onRename}` to the `<SessionCard>` render (after the `onDismiss` prop):

```tsx
<SessionCard
  key={session.id}
  session={session}
  isActive={session.id === activeSessionId}
  onClick={onSessionClick}
  onClose={onClose}
  onDismiss={onDismiss}
  onRename={onRename}
/>
```

- [ ] **Step 3: Commit**

```bash
git add src/components/ProjectGroup/ProjectGroup.tsx
git commit -m "feat: thread onRename prop through ProjectGroup to SessionCard"
```

---

### Task 4: Wire `renameSession` from SessionPanel

**Files:**
- Modify: `src/components/SessionPanel/SessionPanel.tsx`

- [ ] **Step 1: Pull `renameSession` from the store**

Add after line 70 (`const dismissSession = ...`):
```typescript
const renameSession = useSessionStore((s) => s.renameSession);
```

- [ ] **Step 2: Pass `onRename` to ProjectGroup**

Add `onRename={renameSession}` to the `<ProjectGroup>` render (after `onDismiss`):

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
/>
```

- [ ] **Step 3: Commit**

```bash
git add src/components/SessionPanel/SessionPanel.tsx
git commit -m "feat: wire renameSession store action to SessionPanel -> ProjectGroup"
```

---

### Task 5: Add tests for session rename

**Files:**
- Create: `src/components/SessionCard/SessionCard.test.tsx`

- [ ] **Step 1: Write tests for the rename feature**

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SessionCard } from "./SessionCard";
import type { SessionInfo } from "../../types/session";

function makeSession(overrides?: Partial<SessionInfo>): SessionInfo {
  return {
    id: "test-1",
    name: "My Session",
    status: "idle",
    createdAt: Date.now(),
    cwd: "/projects/app",
    ...overrides,
  };
}

describe("SessionCard rename", () => {
  it("enters edit mode on double-click of the name", () => {
    render(
      <SessionCard
        session={makeSession()}
        isActive={false}
        onClick={vi.fn()}
        onRename={vi.fn()}
      />
    );

    const nameEl = screen.getByText("My Session");
    fireEvent.doubleClick(nameEl);

    const input = screen.getByDisplayValue("My Session");
    expect(input).toBeTruthy();
    expect(input.tagName).toBe("INPUT");
  });

  it("saves on Enter and calls onRename", () => {
    const onRename = vi.fn();
    render(
      <SessionCard
        session={makeSession()}
        isActive={false}
        onClick={vi.fn()}
        onRename={onRename}
      />
    );

    fireEvent.doubleClick(screen.getByText("My Session"));
    const input = screen.getByDisplayValue("My Session");
    fireEvent.change(input, { target: { value: "Renamed" } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onRename).toHaveBeenCalledWith("test-1", "Renamed");
  });

  it("cancels on Escape without calling onRename", () => {
    const onRename = vi.fn();
    render(
      <SessionCard
        session={makeSession()}
        isActive={false}
        onClick={vi.fn()}
        onRename={onRename}
      />
    );

    fireEvent.doubleClick(screen.getByText("My Session"));
    const input = screen.getByDisplayValue("My Session");
    fireEvent.change(input, { target: { value: "Renamed" } });
    fireEvent.keyDown(input, { key: "Escape" });

    expect(onRename).not.toHaveBeenCalled();
    expect(screen.getByText("My Session")).toBeTruthy();
  });

  it("reverts to original name if input is empty on save", () => {
    const onRename = vi.fn();
    render(
      <SessionCard
        session={makeSession()}
        isActive={false}
        onClick={vi.fn()}
        onRename={onRename}
      />
    );

    fireEvent.doubleClick(screen.getByText("My Session"));
    const input = screen.getByDisplayValue("My Session");
    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onRename).not.toHaveBeenCalled();
  });

  it("does not call onRename if name is unchanged", () => {
    const onRename = vi.fn();
    render(
      <SessionCard
        session={makeSession()}
        isActive={false}
        onClick={vi.fn()}
        onRename={onRename}
      />
    );

    fireEvent.doubleClick(screen.getByText("My Session"));
    const input = screen.getByDisplayValue("My Session");
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onRename).not.toHaveBeenCalled();
  });

  it("saves on blur and calls onRename", () => {
    const onRename = vi.fn();
    render(
      <SessionCard
        session={makeSession()}
        isActive={false}
        onClick={vi.fn()}
        onRename={onRename}
      />
    );

    fireEvent.doubleClick(screen.getByText("My Session"));
    const input = screen.getByDisplayValue("My Session");
    fireEvent.change(input, { target: { value: "Blurred Name" } });
    fireEvent.blur(input);

    expect(onRename).toHaveBeenCalledWith("test-1", "Blurred Name");
  });

  it("shows Rename option in context menu and enters edit mode on click", () => {
    render(
      <SessionCard
        session={makeSession()}
        isActive={false}
        onClick={vi.fn()}
        onRename={vi.fn()}
      />
    );

    const card = screen.getByRole("button");
    fireEvent.contextMenu(card);

    const renameItem = screen.getByText("Rename");
    expect(renameItem).toBeTruthy();
    fireEvent.click(renameItem);

    expect(screen.getByDisplayValue("My Session")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run the tests**

```bash
npx vitest run src/components/SessionCard/SessionCard.test.tsx
```

Expected: All 7 tests pass.

- [ ] **Step 3: Run all existing tests to check for regressions**

```bash
npx vitest run
```

Expected: All tests pass (SessionPanel tests should still pass since `onRename` is optional).

- [ ] **Step 4: Commit**

```bash
git add src/components/SessionCard/SessionCard.test.tsx
git commit -m "test: add SessionCard rename tests"
```
