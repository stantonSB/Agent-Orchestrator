# Tech Debt

Issues flagged during code review, tracked for future work.

## Store Robustness

### Error handling in store IPC actions

`createSession` and `closeSession` in `src/stores/sessionStore.ts` call `invoke` but have no error handling. If the Tauri command rejects, callers get an unhandled promise rejection with no user-facing feedback. The store should either catch and surface errors (e.g. via an `error` state field) or document that callers must handle rejections.

### setupEventListeners async rejection handling

`setupEventListeners` is synchronous (`void`) but internally performs fire-and-forget async work via `Promise.all`. If `listen` calls reject (e.g. Tauri unavailable), the rejection is silently swallowed. The cancellation pattern using a `cancelled` flag closed over by the Promise callback is correct but non-obvious — add a comment explaining the pattern and consider adding `.catch` logging.

### setActiveSession validation

`setActiveSession` accepts any string without validating the ID exists in the sessions map. Calling `store.setActiveSession("nonexistent-id")` succeeds silently, causing downstream consumers that call `sessions.get(activeSessionId)` to get `undefined`. Either validate at the store level or ensure all consumers handle missing sessions defensively.

### renameSession test coverage

`renameSession` has no tests. The IPC call, map mutation, and no-op for a missing session ID are all untested. Add tests covering the happy path and the non-existent session case.
