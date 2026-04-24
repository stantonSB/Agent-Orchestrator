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
- The `onCreate` callback gains a new `initWithClaude: boolean` parameter.
- On modal open, `initWithClaude` resets to `true`.

### Session Type

- Add `sessionType: "claude" | "terminal"` field to `SessionInfo`. Set at creation time, immutable.
- Add `"terminal"` to `SessionStatus` type — a permanent status for plain terminal sessions that never transitions.
- Session card renders a static grey dot (same colour as idle: `#6b7280`) for terminal status. No pulse animation.
- No elapsed timer shown for terminal sessions.

### Frontend Store (`sessionStore.ts`)

- `createSession` gains the `initWithClaude` parameter.
- When `initWithClaude` is `false`:
  - Pass `command: null` to backend IPC (backend defaults to `$SHELL`).
  - Pass empty args array.
  - Set `sessionType: "terminal"` and `status: "terminal"` on the local `SessionInfo`.
- When `initWithClaude` is `true`:
  - Existing behaviour unchanged.
  - Set `sessionType: "claude"`.
- Skip setting up the status event listener (`session-status-{id}`) for terminal sessions — no status events will arrive.
- Exit event listener still set up for all sessions.

### Backend: PTY Manager (`pty_manager.rs`)

When `command` is `None` (or not `"claude"`), the PTY manager skips:

- Creating a `StatusTracker` for the session.
- Setting `AO_SESSION_ID` and `AO_STATUS_PORT` environment variables.
- The 5-second startup timer (no Starting → Idle transition needed).

What still works for plain terminals:

- PTY pair creation and process spawn (using `$SHELL` from captured env).
- Reader thread for output events (`session-output-{id}`).
- Exit handler for exit events (`session-exit-{id}`).
- Resize handling.

### Backend: Session Listing (`commands.rs`)

- `list_sessions` response includes a `session_type` field (`"claude"` or `"terminal"`) so the frontend can restore session type on app restart.
- Determined by checking whether a `StatusTracker` exists for the session.

### Backend: Command Resolution

- `create_session` already accepts an optional `command` parameter defaulting to `$SHELL`.
- No changes needed to the command dispatch logic itself.
- The `args` parameter is passed through as-is (empty for terminal sessions).

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
- Different visual treatment for terminal sessions in the sidebar beyond the status dot.
