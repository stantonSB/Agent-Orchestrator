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

The script runs inside Claude Code's process context, so `$(pwd)` resolves to the worktree path after Claude has created and entered it.

**Status server:** Extract the `X-Cwd` header from every incoming request. If the path contains `.claude/worktrees/`, store it on the session's `StatusTracker` as `worktree_cwd: Option<String>`. Non-worktree paths are ignored.

**Tauri event:** When a worktree cwd is first set on a tracker, emit a `session-worktree-cwd` event to the frontend with `{ sessionId, worktreeCwd }`. The session store listens and updates the session's `worktreeCwd` field.

**Hook installer:** Bump the hook script content so `is_already_installed` detects the old version and rewrites it with the `X-Cwd` header.

### 2. Data Model Changes

**Frontend `SessionInfo`** — two new fields:
- `parentSessionId: string | null` — links a child terminal to its parent Claude session. Default `null`.
- `worktreeCwd: string | null` — the worktree path, populated from hook events. Default `null`.

**Backend per-session state** — add `worktree_cwd: Option<String>` to the PTY manager's session data, updated when the status server receives a worktree cwd.

**`create_session` IPC command** — add optional `parent_session_id: Option<String>` parameter, passed through for frontend use.

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

Sessions with a `parentSessionId` are excluded from normal project-group rendering. Instead, they render indented (16-20px) directly below their parent's `SessionCard`.

The parent `SessionCard` checks for children (sessions where `parentSessionId === thisSession.id`) and renders them inline.

### 5. Cascading Close

When a parent Claude session is closed:

1. The session store finds all sessions where `parentSessionId === closingSessionId`.
2. Each child is closed first via `close_session` IPC (kills the PTY process).
3. Then the parent is closed.

This logic lives in the frontend session store's `closeSession` method — no backend cascade needed.

### 6. Backend Changes Summary

| Component | Change |
|---|---|
| Hook script | Add `X-Cwd: $(pwd)` header |
| Hook installer | Bump script content to trigger reinstall |
| Status server | Extract `X-Cwd` header, store on tracker, emit Tauri event |
| StatusTracker | Add `worktree_cwd: Option<String>` field |
| `create_session` command | Add optional `parent_session_id` parameter |
| PTY manager session data | Store `worktree_cwd` and `parent_session_id` |

### 7. Frontend Changes Summary

| Component | Change |
|---|---|
| `SessionInfo` type | Add `parentSessionId`, `worktreeCwd` fields |
| Session store | Listen for `session-worktree-cwd` events, cascade close logic |
| `NewSessionModal` | Conditional worktree dropdown when Terminal selected |
| `SessionPanel` / `ProjectGroup` | Filter out child sessions from normal grouping |
| `SessionCard` | Render child sessions indented below parent |

## Out of Scope

- Creating worktrees from the modal (Claude Code handles this)
- Multiple terminal sessions per worktree (allowed — each gets its own parent-child link)
- Worktree cleanup on session close (Claude Code manages its own worktrees)
- Backend validation of parent-child relationships
