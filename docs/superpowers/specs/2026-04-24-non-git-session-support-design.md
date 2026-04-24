# Non-Git Directory Session Support

## Problem

When a user selects a directory that is not a git repository, session creation fails because the app always passes `--worktree` to Claude Code. Claude Code cannot create a worktree in a non-git directory. Users should be able to run Claude sessions in any directory.

## Solution

Detect whether the selected directory is a git repo before session creation. When it isn't, omit `--worktree` from the claude command args, disable git-dependent UI options, and display a visual indicator on the session card showing whether it's running in a worktree or not.

## Design

### 1. New Tauri Command: `check_is_git_repo`

**File:** `src-tauri/src/commands.rs`

- Signature: `fn check_is_git_repo(cwd: String) -> Result<bool, String>`
- Runs `git rev-parse --git-dir` with `current_dir(cwd)`
- Returns `true` if exit code is 0, `false` otherwise
- Stateless — no access to `AppState` needed
- Must be registered in the Tauri command handler

### 2. Data Model: `SessionInfo`

**File:** `src/types/session.ts`

Add field:

```typescript
isGitRepo: boolean
```

Set at session creation time based on the git check result. Immutable after creation.

### 3. NewSessionModal Changes

**File:** `src/components/NewSessionModal/NewSessionModal.tsx`

- Add `isGitRepo: boolean | null` state (null = not yet checked)
- When `directory` changes (via Browse or `lastUsedDirectory`), call `check_is_git_repo` and update state
- When `isGitRepo === false`:
  - "Pull latest from main" checkbox: disabled and unchecked (same disabled pattern as "Skip permissions" when Claude is unticked)
- Pass `isGitRepo` (defaulting to `true` if null) through `onCreate` callback

**`onCreate` signature change:**

```typescript
onCreate: (
  name: string,
  cwd: string,
  skipPermissions: boolean,
  pullLatest: boolean,
  initWithClaude: boolean,
  isGitRepo: boolean
) => void
```

### 4. Session Store Changes

**File:** `src/stores/sessionStore.ts`

In `createSession`:

- Accept `isGitRepo` parameter
- When `isGitRepo === true`: args include `--worktree` (current behavior)
- When `isGitRepo === false`: args omit `--worktree`
- Set `isGitRepo` on the `SessionInfo` object stored in the sessions map

### 5. SessionCard Display

**File:** `src/components/SessionCard/SessionCard.tsx`

For Claude sessions only (not terminal sessions), render an icon between the session name and the duration timer:

- `isGitRepo === true`: 🌳 emoji, ~12px, tooltip "Running in a git worktree"
- `isGitRepo === false`: 📁 emoji, ~12px, tooltip "No worktree — not a git repository"

The icon sits in the existing `nameRow` flex container with `flex-shrink: 0`.

## Files Changed

| File | Change |
|------|--------|
| `src-tauri/src/commands.rs` | Add `check_is_git_repo` command |
| `src-tauri/src/lib.rs` | Register new command in handler |
| `src/types/session.ts` | Add `isGitRepo` to `SessionInfo` |
| `src/components/NewSessionModal/NewSessionModal.tsx` | Git detection on directory select, disable git-dependent checkboxes |
| `src/stores/sessionStore.ts` | Conditional `--worktree` arg, pass `isGitRepo` to `SessionInfo` |
| `src/components/SessionCard/SessionCard.tsx` | Render worktree status icon |

## Out of Scope

- Git repo detection for directories selected in previous sessions (only applies to new session creation)
- Worktree cleanup or management
- Supporting git repos that exist but have issues (e.g., corrupt `.git`)
