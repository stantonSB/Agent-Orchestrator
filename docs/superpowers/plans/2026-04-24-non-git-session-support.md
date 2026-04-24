# Non-Git Directory Session Support Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow users to create Claude sessions in directories that are not git repositories by conditionally omitting `--worktree` and displaying worktree status on session cards.

**Architecture:** Frontend detects git repo status via a new Tauri command when a directory is selected. This drives conditional UI (disabled checkboxes) and conditional args to `claude`. A new `isGitRepo` field on `SessionInfo` persists the state for display in `SessionCard`.

**Tech Stack:** Rust (Tauri command), TypeScript/React (frontend), Vitest (tests)

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src-tauri/src/commands.rs` | Modify | Add `check_is_git_repo` command |
| `src-tauri/src/lib.rs:114-122` | Modify | Register new command in invoke handler |
| `src/types/session.ts` | Modify | Add `isGitRepo` field to `SessionInfo` |
| `src/stores/sessionStore.ts` | Modify | Accept `isGitRepo` param, conditional `--worktree` |
| `src/stores/sessionStore.test.ts` | Modify | Add tests for new `isGitRepo` behavior |
| `src/components/NewSessionModal/NewSessionModal.tsx` | Modify | Git detection, disable checkboxes |
| `src/components/SessionCard/SessionCard.tsx` | Modify | Render worktree status icon |
| `src/components/SessionCard/SessionCard.test.tsx` | Modify | Add tests for icon rendering |
| `src/App.tsx:88-91` | Modify | Pass `isGitRepo` through `handleCreateSession` |

---

## Chunk 1: Backend + Data Model

### Task 1: Add `check_is_git_repo` Tauri command

**Files:**
- Modify: `src-tauri/src/commands.rs` (add new function after `git_pull_main` at line 158)
- Modify: `src-tauri/src/lib.rs:114-122` (register in invoke handler)

- [ ] **Step 1: Add the command to `commands.rs`**

Add after the `git_pull_main` function (line 158):

```rust
#[tauri::command]
pub fn check_is_git_repo(cwd: String) -> Result<bool, String> {
    let path = PathBuf::from(&cwd);
    if !path.exists() {
        return Err(format!("Directory does not exist: {cwd}"));
    }

    let output = std::process::Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(&path)
        .output()
        .map_err(|e| format!("Failed to run git: {e}"))?;

    Ok(output.status.success())
}
```

- [ ] **Step 2: Register the command in `lib.rs`**

In `src-tauri/src/lib.rs`, add `commands::check_is_git_repo` to the `invoke_handler` macro (line 114-122):

```rust
.invoke_handler(tauri::generate_handler![
    commands::create_session,
    commands::close_session,
    commands::write_to_session,
    commands::resize_session,
    commands::rename_session,
    commands::list_sessions,
    commands::git_pull_main,
    commands::check_is_git_repo,
])
```

- [ ] **Step 3: Verify backend compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add check_is_git_repo Tauri command"
```

### Task 2: Add `isGitRepo` to `SessionInfo` type

**Files:**
- Modify: `src/types/session.ts:10-17`

- [ ] **Step 1: Add the field to the `SessionInfo` interface**

In `src/types/session.ts`, add `isGitRepo` to the `SessionInfo` interface:

```typescript
export interface SessionInfo {
  id: string;
  name: string;
  status: SessionStatus;
  createdAt: number; // unix timestamp ms
  cwd: string; // working directory path
  sessionType: "claude" | "terminal";
  isGitRepo: boolean;
}
```

- [ ] **Step 2: Fix all TypeScript errors from the new required field**

Every place that constructs a `SessionInfo` object will now need `isGitRepo`. There are two locations in `sessionStore.ts` (lines 203-210 and 217-224) and several in test files. Add `isGitRepo: true` as the default for now — the store changes in the next task will make it dynamic.

In `src/stores/sessionStore.ts`, the Claude session object (line 203-210):
```typescript
session = {
  id,
  name,
  status: "starting",
  createdAt: Date.now(),
  cwd,
  sessionType: "claude",
  isGitRepo: true,
};
```

In `src/stores/sessionStore.ts`, the terminal session object (line 217-224):
```typescript
session = {
  id,
  name,
  status: "terminal",
  createdAt: Date.now(),
  cwd,
  sessionType: "terminal",
  isGitRepo: false,
};
```

In `src/stores/sessionStore.test.ts`, update the `makeSession`-style session objects. Every `addSession` call that constructs an inline `SessionInfo` needs `isGitRepo: true` (or `false` for terminal sessions). There are approximately 12 inline session objects — add `isGitRepo: true` to each Claude session and `isGitRepo: false` to each terminal session.

In `src/components/SessionCard/SessionCard.test.tsx`, update the `makeSession` helper (line 6-16):
```typescript
function makeSession(overrides?: Partial<SessionInfo>): SessionInfo {
  return {
    id: "test-1",
    name: "My Session",
    status: "idle",
    createdAt: Date.now(),
    cwd: "/projects/app",
    sessionType: "claude",
    isGitRepo: true,
    ...overrides,
  };
}
```

- [ ] **Step 3: Run frontend tests to verify nothing is broken**

Run: `npx vitest run`
Expected: All existing tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/types/session.ts src/stores/sessionStore.ts src/stores/sessionStore.test.ts src/components/SessionCard/SessionCard.test.tsx
git commit -m "feat: add isGitRepo field to SessionInfo type"
```

---

## Chunk 2: Store Logic + Tests

### Task 3: Update `createSession` to conditionally pass `--worktree`

**Files:**
- Modify: `src/stores/sessionStore.ts:25,183-231`
- Modify: `src/stores/sessionStore.test.ts`

- [ ] **Step 1: Write failing tests for the new behavior**

Add these tests to the `createSession` describe block in `src/stores/sessionStore.test.ts`:

```typescript
it("omits --worktree when isGitRepo is false", async () => {
  const { invoke } = await import("@tauri-apps/api/core");
  vi.mocked(invoke).mockResolvedValueOnce("non-git-id");

  const store = useSessionStore.getState();
  await store.createSession("Non-Git Session", "/path/to/non-git", true, false, true, false);

  expect(invoke).toHaveBeenCalledWith("create_session", {
    name: "Non-Git Session",
    cwd: "/path/to/non-git",
    command: "claude",
    args: ["--dangerously-skip-permissions"],
    sessionType: "claude",
  });

  const session = useSessionStore.getState().sessions.get("non-git-id");
  expect(session?.isGitRepo).toBe(false);
});

it("includes --worktree when isGitRepo is true", async () => {
  const { invoke } = await import("@tauri-apps/api/core");
  vi.mocked(invoke).mockResolvedValueOnce("git-id");

  const store = useSessionStore.getState();
  await store.createSession("Git Session", "/path/to/git-repo", true, false, true, true);

  expect(invoke).toHaveBeenCalledWith("create_session", {
    name: "Git Session",
    cwd: "/path/to/git-repo",
    command: "claude",
    args: ["--dangerously-skip-permissions", "--worktree"],
    sessionType: "claude",
  });

  const session = useSessionStore.getState().sessions.get("git-id");
  expect(session?.isGitRepo).toBe(true);
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npx vitest run src/stores/sessionStore.test.ts`
Expected: The two new tests fail (createSession doesn't accept `isGitRepo` param yet).

- [ ] **Step 3: Update the store's `createSession` signature and logic**

In `src/stores/sessionStore.ts`:

Update the `createSession` type in the `SessionState` interface (line 25):
```typescript
createSession: (name: string, cwd: string, skipPermissions?: boolean, pullLatest?: boolean, initWithClaude?: boolean, isGitRepo?: boolean) => Promise<void>;
```

Update the `createSession` implementation (line 183):
```typescript
createSession: async (name, cwd, skipPermissions = true, pullLatest = false, initWithClaude = true, isGitRepo = true) => {
```

Update the args construction for Claude sessions (lines 192-195):
```typescript
    if (initWithClaude) {
      const args: string[] = [];
      if (skipPermissions) {
        args.push("--dangerously-skip-permissions");
      }
      if (isGitRepo) {
        args.push("--worktree");
      }
```

Update the Claude session `SessionInfo` object (lines 203-210) to use the parameter:
```typescript
      session = {
        id,
        name,
        status: "starting",
        createdAt: Date.now(),
        cwd,
        sessionType: "claude",
        isGitRepo,
      };
```

- [ ] **Step 4: Update existing tests that call `createSession` with positional args**

The existing test `"calls Tauri invoke and adds the session"` calls `createSession("My Session", "/path/to/project")` which relies on defaults. Since the default for `isGitRepo` is `true`, this test should still pass as-is. Verify.

The test `"calls git_pull_main before create_session when pullLatest is true"` calls `createSession("Pull Session", "/path/to/project", true, true)` — also fine with defaults.

- [ ] **Step 5: Run all tests**

Run: `npx vitest run`
Expected: All tests pass including the two new ones.

- [ ] **Step 6: Commit**

```bash
git add src/stores/sessionStore.ts src/stores/sessionStore.test.ts
git commit -m "feat: conditionally pass --worktree based on isGitRepo"
```

---

## Chunk 3: Modal + App Wiring

### Task 4: Update NewSessionModal with git detection

**Files:**
- Modify: `src/components/NewSessionModal/NewSessionModal.tsx`

- [ ] **Step 1: Add `isGitRepo` state and detection effect**

Add `invoke` import at top of file:
```typescript
import { invoke } from "@tauri-apps/api/core";
```

Add state after existing state declarations (line 22):
```typescript
const [isGitRepo, setIsGitRepo] = useState<boolean | null>(null);
```

Add an effect that runs when `directory` changes. Place after the existing `useEffect` (after line 34):
```typescript
useEffect(() => {
  if (!directory) {
    setIsGitRepo(null);
    return;
  }
  invoke<boolean>("check_is_git_repo", { cwd: directory })
    .then(setIsGitRepo)
    .catch(() => setIsGitRepo(false));
}, [directory]);
```

Also reset `isGitRepo` in the existing `useEffect` that runs on `isOpen` (add `setIsGitRepo(null)` inside the `if (isOpen)` block, after line 31).

- [ ] **Step 2: Disable "Pull latest from main" when not a git repo**

Compute effective pull latest (add after `effectiveSkipPermissions` on line 50):
```typescript
const effectivePullLatest = isGitRepo === false ? false : pullLatest;
```

Update the "Pull latest from main" checkbox (lines 125-133) to be disabled when not a git repo:
```tsx
<label className={`${styles.checkboxRow} ${isGitRepo === false ? styles.checkboxDisabled : ""}`}>
  <input
    type="checkbox"
    checked={effectivePullLatest}
    onChange={(e) => setPullLatest(e.target.checked)}
    disabled={isGitRepo === false}
    className={styles.checkbox}
  />
  <span className={styles.checkboxLabel}>Pull latest from main</span>
</label>
```

- [ ] **Step 3: Update `onCreate` to pass `isGitRepo`**

Update the `NewSessionModalProps` interface `onCreate` signature (line 8):
```typescript
onCreate: (name: string, cwd: string, skipPermissions: boolean, pullLatest: boolean, initWithClaude: boolean, isGitRepo: boolean) => void;
```

Update `handleCreate` (line 52-56):
```typescript
const handleCreate = () => {
  const trimmedName = name.trim();
  if (!trimmedName || !directory) return;
  onCreate(trimmedName, directory, effectiveSkipPermissions, effectivePullLatest, initWithClaude, isGitRepo ?? true);
};
```

- [ ] **Step 4: Verify the app compiles (TypeScript check)**

Run: `npx tsc --noEmit`
Expected: Type error in `App.tsx` because `handleCreateSession` doesn't accept `isGitRepo` yet. This is expected and will be fixed in the next task.

- [ ] **Step 5: Commit**

```bash
git add src/components/NewSessionModal/NewSessionModal.tsx
git commit -m "feat: detect git repo status in NewSessionModal"
```

### Task 5: Wire `isGitRepo` through App.tsx

**Files:**
- Modify: `src/App.tsx:88-91`

- [ ] **Step 1: Update `handleCreateSession` to accept and pass `isGitRepo`**

Update `handleCreateSession` (line 88-91):
```typescript
const handleCreateSession = async (name: string, cwd: string, skipPermissions: boolean, pullLatest: boolean, initWithClaude: boolean, isGitRepo: boolean) => {
  setIsModalOpen(false);
  await createSession(name, cwd, skipPermissions, pullLatest, initWithClaude, isGitRepo);
};
```

- [ ] **Step 2: Verify TypeScript compiles cleanly**

Run: `npx tsc --noEmit`
Expected: No errors.

- [ ] **Step 3: Run all frontend tests**

Run: `npx vitest run`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/App.tsx
git commit -m "feat: wire isGitRepo through App to session store"
```

---

## Chunk 4: SessionCard Icons + Tests

### Task 6: Add worktree status icon to SessionCard

**Files:**
- Modify: `src/components/SessionCard/SessionCard.tsx:107-142`
- Modify: `src/components/SessionCard/SessionCard.module.css`

- [ ] **Step 1: Write failing tests for the icon**

Add to `src/components/SessionCard/SessionCard.test.tsx`:

```typescript
describe("SessionCard worktree icon", () => {
  it("shows tree icon for Claude sessions with isGitRepo true", () => {
    const session = makeSession({ isGitRepo: true });
    render(
      <SessionCard session={session} isActive={false} onClick={vi.fn()} />
    );
    const icon = screen.getByTitle("Running in a git worktree");
    expect(icon).toBeTruthy();
    expect(icon.textContent).toContain("🌳");
  });

  it("shows folder icon for Claude sessions with isGitRepo false", () => {
    const session = makeSession({ isGitRepo: false });
    render(
      <SessionCard session={session} isActive={false} onClick={vi.fn()} />
    );
    const icon = screen.getByTitle("No worktree — not a git repository");
    expect(icon).toBeTruthy();
    expect(icon.textContent).toContain("📁");
  });

  it("does not show worktree icon for terminal sessions", () => {
    const session = makeSession({ sessionType: "terminal", status: "terminal", isGitRepo: false });
    render(
      <SessionCard session={session} isActive={false} onClick={vi.fn()} />
    );
    expect(screen.queryByTitle("Running in a git worktree")).toBeNull();
    expect(screen.queryByTitle("No worktree — not a git repository")).toBeNull();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npx vitest run src/components/SessionCard/SessionCard.test.tsx`
Expected: The 3 new tests fail.

- [ ] **Step 3: Add the CSS class for the worktree icon**

Add to `src/components/SessionCard/SessionCard.module.css`:

```css
.worktreeIcon {
  font-size: 12px;
  flex-shrink: 0;
  line-height: 1;
  cursor: help;
}
```

- [ ] **Step 4: Add the icon to SessionCard component**

In `src/components/SessionCard/SessionCard.tsx`, inside the `nameRow` div (line 108-142), add the worktree icon between the name/input and the DurationTimer. Replace the block from line 139-141:

```tsx
{session.sessionType !== "terminal" && (
  <>
    <span
      className={styles.worktreeIcon}
      title={session.isGitRepo ? "Running in a git worktree" : "No worktree — not a git repository"}
    >
      {session.isGitRepo ? "🌳" : "📁"}
    </span>
    <DurationTimer createdAt={session.createdAt} active={isRunning(session.status)} />
  </>
)}
```

- [ ] **Step 5: Run all tests**

Run: `npx vitest run`
Expected: All tests pass including the 3 new ones.

- [ ] **Step 6: Commit**

```bash
git add src/components/SessionCard/SessionCard.tsx src/components/SessionCard/SessionCard.module.css src/components/SessionCard/SessionCard.test.tsx
git commit -m "feat: display worktree status icon on session cards"
```

### Task 7: Final verification

- [ ] **Step 1: Run full test suite**

Run: `npx vitest run`
Expected: All tests pass.

- [ ] **Step 2: Run backend compilation check**

Run: `cd src-tauri && cargo check`
Expected: Compiles with no errors.

- [ ] **Step 3: Run TypeScript check**

Run: `npx tsc --noEmit`
Expected: No errors.

- [ ] **Step 4: Manual smoke test (if dev server available)**

Run: `npm run tauri dev`
- Create a session pointing to a git repo — should work as before with 🌳 icon
- Create a session pointing to a non-git directory (e.g. `/tmp/test-non-git`) — should work without `--worktree`, show 📁 icon, and have "Pull latest from main" disabled
