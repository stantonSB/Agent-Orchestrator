# Session Mode Dropdown

**Date:** 2026-04-29
**Status:** Approved

## Summary

Replace the "Initialise with Claude" checkbox and "Skip permissions" checkbox in `NewSessionModal` with a single dropdown showing the different ways to run a session. Persist the user's last selection in `localStorage`.

## Session Modes

| Mode key | Display label | CLI behavior |
|---|---|---|
| `claude` | Claude | `claude` (standard, permission prompts enabled) |
| `claude-skip` | Claude (skip permissions) | `claude --dangerously-skip-permissions` |
| `claude-plan` | Claude (plan mode) | `claude --plan` |
| `terminal` | Terminal | Spawns `$SHELL`, no Claude |

The `--worktree` flag is conditionally appended for all Claude modes when `isGitRepo` is true, same as today.

## Persistence

- **Storage:** `localStorage` key `"ao-last-session-mode"`
- **Read:** On modal open, read stored value. If valid mode key, use it; otherwise default to `"claude"`.
- **Write:** On session create, save the selected mode to localStorage.

## Component Changes

### NewSessionModal.tsx

- Remove `skipPermissions` and `initWithClaude` state variables.
- Add `sessionMode` state typed as `"claude" | "claude-skip" | "claude-plan" | "terminal"`.
- On mount (when `isOpen` becomes true), read `localStorage.getItem("ao-last-session-mode")` and set state if valid.
- Remove the "Initialise with Claude" checkbox (lines 132-142) and the "Skip permissions" checkbox (lines 155-164). Replace them with a styled `<select>` inside a `.field` div with label "Session Mode". Keep the "Pull latest from main" checkbox (lines 144-153) in place.
- On create, save mode to localStorage, then call `onCreate` with the new signature.

### onCreate callback signature

- **Old:** `(name: string, cwd: string, skipPermissions: boolean, pullLatest: boolean, initWithClaude: boolean, isGitRepo: boolean)`
- **New:** `(name: string, cwd: string, sessionMode: SessionMode, pullLatest: boolean, isGitRepo: boolean)`

Where `SessionMode = "claude" | "claude-skip" | "claude-plan" | "terminal"`.

### sessionStore.ts createSession

- Update the `SessionState` interface's `createSession` type signature to match the new implementation signature.
- Change implementation signature: replace `skipPermissions` + `initWithClaude` params with `sessionMode: SessionMode`.
- Map mode to CLI args:
  - `"claude"` → `[]`
  - `"claude-skip"` → `["--dangerously-skip-permissions"]`
  - `"claude-plan"` → `["--plan"]`
  - `"terminal"` → no Claude command, spawn shell
- `--worktree` still appended conditionally based on `isGitRepo` for all Claude modes.

### App.tsx

- Update `handleCreateSession` to pass `sessionMode` instead of `skipPermissions` and `initWithClaude`.

### CSS (NewSessionModal.module.css)

- Add `.select` style matching the existing `.input` style (same background `#12121a`, border, border-radius, colors, font).
- Remove `.checkboxLabelPrimary` if no longer referenced after removing the "Initialise with Claude" checkbox.

## Unchanged

- **Backend `commands.rs`:** No changes. Already accepts arbitrary `args: Option<Vec<String>>`.
- **PTY manager:** No changes.
- **Status detection:** Unaffected.
- **Session types in store:** Still `"claude"` or `"terminal"`. The `SessionMode` → `sessionType` mapping is: `"claude"`, `"claude-skip"`, and `"claude-plan"` all map to `sessionType: "claude"`; `"terminal"` maps to `sessionType: "terminal"`.
- **"Pull latest from main" checkbox:** Stays as-is, still conditional on `isGitRepo`.

## Type Definition

Add `SessionMode` type to `src/types/session.ts` alongside existing `SessionInfo` and `SessionStatus`:

```typescript
export type SessionMode = "claude" | "claude-skip" | "claude-plan" | "terminal";
```

## Test Changes

Update `src/stores/sessionStore.test.ts` to use the new `createSession(name, cwd, sessionMode, pullLatest, isGitRepo)` signature. The existing tests in the `createSession` describe block call the old positional parameter signature and will need updating:

- Default calls → pass `"claude-skip"` (was the old default behavior with `skipPermissions=true`)
- Terminal session tests → pass `"terminal"`
- Update arg assertions (e.g., `["--dangerously-skip-permissions", "--worktree"]` stays the same for `"claude-skip"` mode)

## Plan Mode Hook Behavior

Claude's `--plan` mode fires the same Notification and Stop hooks as normal mode. The status state machine will work correctly without changes — plan mode sessions will transition through Starting → Working → Idle/Finished like any other Claude session.
