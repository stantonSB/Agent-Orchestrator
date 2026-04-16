# Fix Terminal Output Race Condition — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix intermittent terminal rendering corruption caused by output listener teardown gaps and missing output buffering.

**Architecture:** Replace the batch useEffect listener pattern in TerminalArea.tsx with incremental ref-based listener tracking. Add per-session output buffering that captures data before the xterm.js handle is ready and flushes on ref attachment.

**Tech Stack:** React, TypeScript, Tauri IPC events, xterm.js

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/components/TerminalArea/TerminalArea.tsx` | Modify | Replace batch listener effect with incremental management + buffering |

No new files. No backend changes.

---

## Chunk 1: Implementation

### Task 1: Replace batch listener useEffect with incremental listener management and output buffering

**Files:**
- Modify: `src/components/TerminalArea/TerminalArea.tsx:1-151`

- [ ] **Step 1: Add ref declarations for listener tracking and output buffering**

At the top of the component function body (after `refsMap`), add three new refs:

```ts
const outputListeners = useRef(new Map<string, Promise<() => void>>());
const exitListeners = useRef(new Map<string, Promise<() => void>>());
const outputBuffers = useRef(new Map<string, Uint8Array[]>());
const onSessionExitRef = useRef(onSessionExitProp);
onSessionExitRef.current = onSessionExitProp;
```

- [ ] **Step 2: Update setRef to flush buffered output**

Replace the existing `setRef` callback with a version that flushes buffered output when a terminal handle is set:

```ts
const setRef = useCallback(
  (id: string) => (handle: XTermInstanceHandle | null) => {
    if (handle) {
      refsMap.current.set(id, handle);
      // Flush any output that arrived before the terminal mounted
      const buffer = outputBuffers.current.get(id);
      if (buffer) {
        for (const chunk of buffer) handle.write(chunk);
        outputBuffers.current.delete(id);
      }
    } else {
      refsMap.current.delete(id);
    }
  },
  [],
);
```

- [ ] **Step 3: Replace the batch useEffect with incremental listener management**

Remove the entire existing `useEffect` block (lines 55–91) that wires Tauri event listeners. Replace it with an incremental effect that only registers new listeners and unregisters removed ones:

```ts
// Incrementally manage per-session output and exit listeners.
// Listeners persist in refs so adding session B never tears down session A's listener.
useEffect(() => {
  if (mockMode) return;

  const currentIds = new Set(sessions.map((s) => s.id));

  // Register output listeners for new sessions
  for (const session of sessions) {
    const sid = session.id;
    if (outputListeners.current.has(sid)) continue;

    const promise = onSessionOutput(sid, (payload) => {
      if (!payload.data) return;
      const handle = refsMap.current.get(sid);
      if (handle) {
        handle.write(new Uint8Array(payload.data));
      } else {
        let buf = outputBuffers.current.get(sid);
        if (!buf) {
          buf = [];
          outputBuffers.current.set(sid, buf);
        }
        buf.push(new Uint8Array(payload.data));
      }
    });
    outputListeners.current.set(sid, promise);
  }

  // Register exit listeners for new sessions
  for (const session of sessions) {
    const sid = session.id;
    if (exitListeners.current.has(sid)) continue;

    const promise = onSessionExit(sid, (payload) => {
      onSessionExitRef.current?.(sid, payload);
    });
    exitListeners.current.set(sid, promise);
  }

  // Clean up listeners for removed sessions
  for (const [sid, promise] of outputListeners.current) {
    if (!currentIds.has(sid)) {
      promise.then((unlisten) => unlisten());
      outputListeners.current.delete(sid);
      outputBuffers.current.delete(sid);
    }
  }
  for (const [sid, promise] of exitListeners.current) {
    if (!currentIds.has(sid)) {
      promise.then((unlisten) => unlisten());
      exitListeners.current.delete(sid);
    }
  }
}, [sessions, mockMode]);
```

- [ ] **Step 4: Add unmount cleanup effect**

Add a separate effect with empty deps that cleans up all listeners when TerminalArea unmounts:

```ts
// Clean up all listeners on unmount.
useEffect(() => {
  return () => {
    for (const [, promise] of outputListeners.current) {
      promise.then((unlisten) => unlisten());
    }
    for (const [, promise] of exitListeners.current) {
      promise.then((unlisten) => unlisten());
    }
    outputListeners.current.clear();
    exitListeners.current.clear();
    outputBuffers.current.clear();
  };
}, []);
```

- [ ] **Step 5: Verify the full file compiles**

Run: `cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/.claude/worktrees/buzzing-doodling-codd && npx tsc --noEmit`
Expected: No type errors in TerminalArea.tsx

- [ ] **Step 6: Run existing tests**

Run: `cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/.claude/worktrees/buzzing-doodling-codd && npx vitest run`
Expected: All existing tests pass (SessionPanel, sessionStore)

- [ ] **Step 7: Commit**

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/.claude/worktrees/buzzing-doodling-codd
git add src/components/TerminalArea/TerminalArea.tsx
git commit -m "fix: replace batch listener teardown with incremental management and output buffering

Fixes intermittent terminal rendering corruption caused by:
1. All output listeners being torn down and re-registered whenever the
   sessions array changes, creating a gap where output events are lost
2. No buffering of output that arrives before the xterm.js handle is ready

The new approach tracks listeners in refs (never tears down existing
listeners when a new session is added) and buffers output until the
terminal component mounts and flushes the buffer via the ref callback."
```

### Task 2: Manual smoke test

- [ ] **Step 1: Start the dev server**

Run: `cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/.claude/worktrees/buzzing-doodling-codd && npx tauri dev`

- [ ] **Step 2: Verify first session renders correctly**

Create a new session. The Claude Code welcome screen should display properly with the pig mascot, two-column layout, and status bar.

- [ ] **Step 3: Verify subsequent sessions render correctly**

Create a second session while the first is running. Both should render correctly when switching between them. This is the scenario that was previously broken.
