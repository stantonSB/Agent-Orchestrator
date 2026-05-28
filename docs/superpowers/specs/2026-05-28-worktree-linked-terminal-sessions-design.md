# Worktree-Linked Terminal Sessions

## Problem

When running Claude Code sessions in Agent Orchestrator, each Claude session creates a git worktree (via `--worktree`). Users often need to open a plain terminal in that same worktree to run tests, inspect files, or debug — but currently there's no way to link a terminal session to a specific worktree.

## Solution

Add the ability to create terminal sessions linked to an active Claude session's worktree. The terminal opens in the worktree directory and appears nested under the parent Claude session in the sidebar. Closing the parent cascades to close child terminals.

## Design

### 1. Worktree Path Discovery

**Hook script change:** Modify `~/.claude/agent-orchestrator-notify.sh` to include the current working directory as an HTTP header on every hook request:

```bash
#!/bin/bash
if [ -n "$AO_STATUS_PORT" ] && [ -n "$AO_SESSION_ID" ]; then
    curl -s -X POST "http://127.0.0.1:${AO_STATUS_PORT}/status/${AO_SESSION_ID}" \
        -H "Content-Type: application/json" \
        -H "X-Cwd: $(pwd)" \
        -d @- 2>/dev/null || true
fi
```

The script runs inside Claude Code's process context, so `$(pwd)` resolves to the worktree path after Claude has created and entered it. Claude Code hooks are invoked as child processes that inherit the parent's cwd. This assumption is validated by the fact that the existing Stop hook already sends `cwd` from the same context. For Notification hooks (which don't include `cwd` in their JSON body), we rely on the same inherited-cwd behavior via the `X-Cwd` header.

**Status server:** Extract the `X-Cwd` header from every incoming request. If the path contains `.claude/worktrees/`, store it on the session's `StatusTracker` as `worktree_cwd: Option<String>`. Non-worktree paths are ignored.

**Tauri event emission:** Add a new callback `on_worktree_cwd: Arc<WorktreeCwdCallback>` to `StatusServer::start`, following the same pattern as the existing `on_status` and `on_subagents` callbacks. When a worktree cwd is first set on a tracker, invoke this callback. The Tauri side wires it to emit a `session-worktree-cwd` event with `{ sessionId, worktreeCwd }`.

**Hook installer:** The current `is_already_installed` function checks for script existence and permissions but not content. Add a content check: compare the existing script against `HOOK_SCRIPT` and return `false` if they differ. This ensures updating `HOOK_SCRIPT` with the `X-Cwd` header triggers a rewrite for existing installations.

### 2. Data Model Changes

**Frontend `SessionInfo`** — two new fields:
- `parentSessionId: string | null` — links a child terminal to its parent Claude session. Default `null`.
- `worktreeCwd: string | null` — the worktree path, populated from hook events. Default `null`.

Both fields are **frontend-only**. `parentSessionId` is set at creation time in the session store's `createSession` method. `worktreeCwd` is set when the `session-worktree-cwd` event arrives.

**Backend:** `worktree_cwd` is stored **only** on `StatusTracker` (since the status server receives hook events). No changes needed to the PTY manager's `Session` struct or the `create_session` IPC command signature.

**Persistence:** Neither field is persisted. On app restart, sessions restore with `parentSessionId: null` and `worktreeCwd: null`. Worktree paths may not exist after restart, and child terminal sessions are ephemeral by design.

**`createSession` store method** gains an optional parameter:
```typescript
createSession: async (
  name: string,
  cwd: string,
  sessionMode?: SessionMode,
  pullLatest?: boolean,
  isGitRepo?: boolean,
  parentSessionId?: string  // new
) => ...
```

When `parentSessionId` is provided, the constructed `SessionInfo` includes it.

### 3. New Session Modal

When "Terminal" is selected from the session mode dropdown:

1. A second dropdown appears labeled "Worktree" with default value "None".
2. Options are populated from active sessions where:
   - `sessionType === "claude"`
   - `worktreeCwd !== null`
   - Status is not `finished`, `exited`, or `error`
3. Each option is labeled with the parent Claude session's **name**.
4. Selecting a worktree:
   - Sets the terminal's cwd to the selected session's `worktreeCwd`
   - Sets `parentSessionId` to the selected Claude session's ID
   - Disables/greys out the project directory picker, showing the worktree path

When any Claude mode is selected, the worktree dropdown disappears.

### 4. Sidebar Nesting

Child sessions (those with `parentSessionId`) are filtered out **before** passing sessions to `groupSessionsByProject` in `SessionPanel`. This prevents child sessions from creating their own project groups based on their worktree cwd.

```typescript
const topLevelSessions = sessions.filter(s => !s.parentSessionId);
const groups = groupSessionsByProject(topLevelSessions);
```

Within each `ProjectGroup`, the parent `SessionCard` checks for children (sessions where `parentSessionId === thisSession.id`) and renders them indented (16-20px) directly below.

### 5. Cascading Close

When a parent Claude session is closed:

1. The session store finds all sessions where `parentSessionId === closingSessionId`.
2. All children are closed in parallel via `Promise.all(children.map(c => invoke("close_session", { id: c.id })))`.
3. All child sessions and the parent are removed from the store atomically in a single `set()` call to avoid intermediate states (e.g. briefly selecting a child as active session).
4. Then the parent's `close_session` IPC is called.

This logic lives in the frontend session store's `closeSession` method.

### 6. Backend Changes Summary

| Component | Change |
|---|---|
| Hook script | Add `X-Cwd: $(pwd)` header |
| Hook installer | Add script content check to `is_already_installed` |
| Status server | Extract `X-Cwd` header, store on tracker, invoke new `on_worktree_cwd` callback |
| StatusTracker | Add `worktree_cwd: Option<String>` field |

### 7. Frontend Changes Summary

| Component | Change |
|---|---|
| `SessionInfo` type | Add `parentSessionId`, `worktreeCwd` fields |
| Session store | Add `parentSessionId` param to `createSession`, listen for `session-worktree-cwd` events, cascade close logic |
| `NewSessionModal` | Conditional worktree dropdown when Terminal selected |
| `SessionPanel` | Filter out child sessions before grouping |
| `SessionCard` / `ProjectGroup` | Render child sessions indented below parent |

## Out of Scope

- Creating worktrees from the modal (Claude Code handles this)
- Multiple terminal sessions per worktree (allowed — each gets its own parent-child link)
- Worktree cleanup on session close (Claude Code manages its own worktrees)
- Backend validation of parent-child relationships
- Persisting `parentSessionId` or `worktreeCwd` across app restarts
