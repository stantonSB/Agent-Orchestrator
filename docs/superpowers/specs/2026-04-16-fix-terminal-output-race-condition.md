# Fix Terminal Output Race Condition

**Date**: 2026-04-16
**Status**: Approved
**Type**: Bug fix

## Problem

The terminal rendering in Agent Orchestrator is intermittently corrupted. The Claude Code welcome screen renders with garbled layout — elements are present but mispositioned. The issue is intermittent: first session after launch tends to work, but subsequent sessions frequently break.

## Root Cause

Two bugs in `src/components/TerminalArea/TerminalArea.tsx`:

### Bug 1: Batch listener teardown on every sessions change

The `useEffect` that registers Tauri output listeners depends on `sessions`. Every time a session is added or removed:

1. The cleanup function runs, tearing down ALL output listeners
2. An async IIFE re-registers listeners for ALL sessions
3. During the gap between steps 1 and 2, output events for every session are lost

Lost escape sequences (cursor positioning, alternate screen buffer, color setup) corrupt xterm.js internal state, causing all subsequent rendering to be garbled.

### Bug 2: No output buffering before terminal mount

When a new session's output listener registers, the `XTermInstance` ref may not be available yet (React hasn't mounted/committed the component). Output that arrives before the ref is set is silently dropped:

```ts
const handle = refsMap.current.get(sid);
if (handle && payload.data) {
  handle.write(new Uint8Array(payload.data));  // drops if handle is null
}
```

## Solution: Incremental Listener Management with Output Buffering

### Changes (single file: `TerminalArea.tsx`)

#### 1. Incremental listener tracking via refs

Replace the batch `useEffect` with ref-based listener maps:

- `outputListeners: Map<string, Promise<() => void>>` — tracks registered output listeners
- `exitListeners: Map<string, Promise<() => void>>` — tracks registered exit listeners

The `useEffect` only registers listeners for sessions that don't have one yet, and only unregisters for sessions that have been removed. No existing listener is ever torn down when a new session is added.

#### 2. Output buffering

- `outputBuffers: Map<string, Uint8Array[]>` — per-session output buffer

In the output listener callback:
- If the terminal handle exists → write directly
- If the terminal handle doesn't exist yet → push to buffer

In the `setRef` callback:
- When a handle is set → flush any buffered chunks for that session

#### 3. Stable exit callback ref

Store `onSessionExitProp` in a ref (`onSessionExitRef`) so exit listeners don't need to be re-registered when the callback reference changes.

#### 4. Unmount cleanup

A separate `useEffect(() => cleanup, [])` that tears down all listeners and clears all buffers when TerminalArea unmounts.

### What stays the same

- `handleSessionData` (keystroke forwarding)
- `handleSessionResize` (resize forwarding)
- JSX rendering and XTermInstance components
- All backend code (Rust PTY manager, commands, event emission)
- `useTerminal` hook and xterm.js configuration

## Why this works

- **No teardown gap**: Adding session B never touches session A's listener
- **No lost output**: Data arriving before terminal mount is buffered and flushed when the ref is set
- **Minimal change surface**: Single file modification, no backend changes, no new dependencies
