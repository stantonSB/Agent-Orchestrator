# Future Phases

Improvements and features identified during Wave 3 development, tracked here for future waves.

---

## Store Robustness

These items were flagged during code review of the Zustand session store (Wave 3, Task 3A).

### Error handling in store IPC actions

`createSession` and `closeSession` in `src/stores/sessionStore.ts` call `invoke` but have no error handling. If the Tauri command rejects, callers get an unhandled promise rejection with no user-facing feedback. The store should either catch and surface errors (e.g. via an `error` state field) or document that callers must handle rejections.

### setupEventListeners async rejection handling

`setupEventListeners` is synchronous (`void`) but internally performs fire-and-forget async work via `Promise.all`. If `listen` calls reject (e.g. Tauri unavailable), the rejection is silently swallowed. The cancellation pattern using a `cancelled` flag closed over by the Promise callback is correct but non-obvious — add a comment explaining the pattern and consider adding `.catch` logging.

### setActiveSession validation

`setActiveSession` accepts any string without validating the ID exists in the sessions map. Calling `store.setActiveSession("nonexistent-id")` succeeds silently, causing downstream consumers that call `sessions.get(activeSessionId)` to get `undefined`. Either validate at the store level or ensure all consumers handle missing sessions defensively.

### renameSession test coverage

`renameSession` has no tests. The IPC call, map mutation, and no-op for a missing session ID are all untested. Add tests covering the happy path and the non-existent session case.

---

## Nested Subagent Terminals

### Overview

When a session spawns subagents (e.g. Claude Code dispatching parallel workers), their terminals should appear as nested tabs within the parent session's terminal area on the right-hand side panel.

### Design

- Each parent session can have zero or more child subagent sessions
- The right-hand terminal area shows the active parent session's terminal at full height by default
- When subagents are active, a tab bar appears at the top of the terminal area with tabs for `Parent` plus each subagent (e.g. `Parent | Agent 1 | Agent 2`)
- Clicking a tab switches which terminal is visible (CSS show/hide, not unmount — preserving scrollback)
- Subagent tabs show a status indicator (dot matching SessionStatus colors) so users can monitor progress without switching
- When a subagent finishes, its tab remains accessible (for reviewing output) but is visually dimmed

### Data Model Changes

- Add `parentSessionId: string | null` to `SessionInfo` — null for top-level sessions, set for subagents
- Add a derived selector `getChildSessions(parentId)` to the store that filters sessions by `parentSessionId`
- The backend `create_session` command needs a `parent_id` parameter to establish the relationship

### UI Changes

- New `TerminalTabBar` component: renders tabs for parent + children, highlights active, shows status dots
- Modify the terminal area in `App.tsx` to render `TerminalTabBar` when the active session has children
- All terminals (parent + children) stay mounted with CSS visibility toggling via the existing `isActive` prop pattern

### Backend Changes

- Extend `create_session` Tauri command to accept optional `parent_id`
- Emit a `session-child-created-{parent_id}` event so the frontend can reactively update
- `list_sessions` response should include `parent_id` field
