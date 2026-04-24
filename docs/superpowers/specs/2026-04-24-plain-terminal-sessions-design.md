# Plain Terminal Sessions

**Date:** 2026-04-24
**Status:** Draft

## Overview

Allow users to open plain terminal sessions (user's default shell) instead of always initialising Claude Code. A new "Initialise with Claude" checkbox in the New Session modal controls this, defaulting to checked.

## Motivation

Users sometimes need a regular terminal alongside their Claude sessions — for manual git operations, running scripts, monitoring processes, etc. Currently the only way to get a terminal is to open a separate app.

## Design

### Modal UI

- New `initWithClaude` boolean state, defaulting to `true`.
- "Initialise with Claude" checkbox placed above existing checkboxes, with a slightly bolder label to distinguish it as a primary option.
- When unchecked: "Skip permissions" becomes disabled and unchecked (visually greyed out at reduced opacity). "Pull latest from main" remains fully interactive.
- The `onCreate` callback gains a new `initWithClaude: boolean` parameter. This flows through the full call chain: `NewSessionModal.onCreate` → `App.tsx` handler → `sessionStore.createSession`.
- On modal open, `initWithClaude` resets to `true`.

### Session Type

- Add `sessionType: "claude" | "terminal"` field to `SessionInfo`. Set at creation time, immutable.
- Add `"terminal"` to `SessionStatus` type — a permanent status for plain terminal sessions that never transitions.
- Session card renders a static `#6b7280` grey dot for terminal status. No pulse animation. This is the same grey as idle — intentionally simple. The `sessionType` field on `SessionInfo` is the authoritative way to distinguish them, not the dot colour.
- No elapsed timer shown for terminal sessions.
- The `isRunning` helper in `SessionCard` must be updated: terminal sessions are considered "running" while the process is alive (they are closeable, not dismissable), but the elapsed timer is suppressed via a separate `sessionType` check.

### Frontend Store (`sessionStore.ts`)

- `createSession` gains the `initWithClaude` parameter.
- When `initWithClaude` is `false`:
  - Pass `command: null` to backend IPC (backend defaults to `$SHELL`).
  - Pass `sessionType: "terminal"` to the backend IPC command.
  - Pass empty args array.
  - Set `sessionType: "terminal"` and `status: "terminal"` on the local `SessionInfo`.
- When `initWithClaude` is `true`:
  - Existing behaviour unchanged.
  - Pass `sessionType: "claude"` to the backend IPC command.
  - Set `sessionType: "claude"`.
- Skip setting up the status event listener (`session-status-{id}`) for terminal sessions — no status events will arrive.
- Exit event listener still set up for all sessions. On exit, terminal sessions transition to `"error"` only on non-zero exit codes; otherwise they remain `"terminal"` (no `"finished"` state for terminals).

### Backend: IPC Command (`commands.rs`)

- `create_session` gains a new `session_type: Option<String>` parameter, defaulting to `"claude"`.
- This is forwarded to the PTY manager via `PtyRequest::Create` as a new `session_type: SessionType` field (enum: `Claude | Terminal`).
- The command resolution logic is unchanged — `command` defaults to `$SHELL` when `None`.

### Backend: PTY Manager (`pty_manager.rs`)

The `Session` struct gains a `session_type: SessionType` field, stored at creation time.

When `session_type` is `Terminal`, the PTY manager skips:

- Creating a `StatusTracker` for the session.
- Setting `AO_SESSION_ID` and `AO_STATUS_PORT` environment variables.
- The 5-second startup timer (no Starting → Idle transition needed).

What still works for plain terminals:

- PTY pair creation and process spawn (using `$SHELL` from captured env).
- Reader thread for output events (`session-output-{id}`).
- Exit handler for exit events (`session-exit-{id}`). The reader thread's exit path gracefully handles the absence of a `StatusTracker` — when `trackers.get_mut(&id)` returns `None`, no status event is emitted, and only the `session-exit-{id}` event fires.
- The `Write` handler's `notify_user_input` path also handles tracker absence gracefully (`None` return, no-op).
- Resize handling.

### Backend: Session Listing (`commands.rs`)

- `list_sessions` response includes a `session_type` field (`"claude"` or `"terminal"`).
- Read directly from the `Session` struct's `session_type` field (not derived from tracker presence, which would be fragile since trackers are removed on session kill).

## Status Dot Rendering

| Status | Colour | Animation | Icon |
|--------|--------|-----------|------|
| `terminal` | `#6b7280` (grey) | None | Dot |
| `starting` | `#3b82f6` (blue) | None | Dot |
| `working` | `#22c55e` (green) | Pulse | Dot |
| `idle` | `#6b7280` (grey) | None | Dot |
| `needs_attention` | `#f97316` (orange) | None | Dot |
| `finished` | — | — | ✓ checkmark |
| `error` | `#ef4444` (red) | None | Dot |

## Out of Scope

- Converting an existing Claude session to a terminal session or vice versa.
- Custom shell selection (always uses `$SHELL`).
- Launching Claude Code manually inside a plain terminal session — hooks won't have `AO_SESSION_ID`/`AO_STATUS_PORT`, so status tracking won't work. The session remains in permanent `"terminal"` status. This is intentional; the session type is immutable.
