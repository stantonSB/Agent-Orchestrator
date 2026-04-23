# Nested Subagent Terminals

## Overview

When a session spawns subagents (e.g. Claude Code dispatching parallel workers), their terminals should appear as nested tabs within the parent session's terminal area on the right-hand side panel.

## Design

- Each parent session can have zero or more child subagent sessions
- The right-hand terminal area shows the active parent session's terminal at full height by default
- When subagents are active, a tab bar appears at the top of the terminal area with tabs for `Parent` plus each subagent (e.g. `Parent | Agent 1 | Agent 2`)
- Clicking a tab switches which terminal is visible (CSS show/hide, not unmount — preserving scrollback)
- Subagent tabs show a status indicator (dot matching SessionStatus colors) so users can monitor progress without switching
- When a subagent finishes, its tab remains accessible (for reviewing output) but is visually dimmed

## Data Model Changes

- Add `parentSessionId: string | null` to `SessionInfo` — null for top-level sessions, set for subagents
- Add a derived selector `getChildSessions(parentId)` to the store that filters sessions by `parentSessionId`
- The backend `create_session` command needs a `parent_id` parameter to establish the relationship

## UI Changes

- New `TerminalTabBar` component: renders tabs for parent + children, highlights active, shows status dots
- Modify the terminal area in `App.tsx` to render `TerminalTabBar` when the active session has children
- All terminals (parent + children) stay mounted with CSS visibility toggling via the existing `isActive` prop pattern

## Backend Changes

- Extend `create_session` Tauri command to accept optional `parent_id`
- Emit a `session-child-created-{parent_id}` event so the frontend can reactively update
- `list_sessions` response should include `parent_id` field
