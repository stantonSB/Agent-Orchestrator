# Auto Mode Session Option — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a "Claude (auto)" dropdown option that launches Claude Code with `--auto`.

**Architecture:** Frontend-only change. Add `"claude-auto"` to the `SessionMode` type, the modal dropdown, the validation array, and the arg-mapping logic. Backend untouched.

**Tech Stack:** TypeScript, React

**Spec:** `docs/superpowers/specs/2026-04-29-auto-mode-session-option-design.md`

---

## Chunk 1: Implementation

### Task 1: Add `"claude-auto"` to `SessionMode` type

**Files:**
- Modify: `src/types/session.ts:10`

- [ ] **Step 1: Update the type union**

```typescript
export type SessionMode = "claude-auto" | "claude" | "claude-skip" | "claude-plan" | "terminal";
```

- [ ] **Step 2: Commit**

```bash
git add src/types/session.ts
git commit -m "feat: add claude-auto to SessionMode type"
```

### Task 2: Add dropdown option and update VALID_MODES

**Files:**
- Modify: `src/components/NewSessionModal/NewSessionModal.tsx:8` (VALID_MODES)
- Modify: `src/components/NewSessionModal/NewSessionModal.tsx:152-155` (dropdown options)

- [ ] **Step 1: Add `"claude-auto"` to `VALID_MODES` (line 8)**

```typescript
const VALID_MODES: SessionMode[] = ["claude-auto", "claude", "claude-skip", "claude-plan", "terminal"];
```

- [ ] **Step 2: Add the dropdown `<option>` as the first entry (before line 152)**

```tsx
<option value="claude-auto">Claude (auto)</option>
<option value="claude">Claude</option>
<option value="claude-skip">Claude (skip permissions)</option>
<option value="claude-plan">Claude (plan mode)</option>
<option value="terminal">Terminal</option>
```

- [ ] **Step 3: Commit**

```bash
git add src/components/NewSessionModal/NewSessionModal.tsx
git commit -m "feat: add Claude (auto) option to session mode dropdown"
```

### Task 3: Map `"claude-auto"` to `--auto` CLI flag

**Files:**
- Modify: `src/stores/sessionStore.ts:208-212`

- [ ] **Step 1: Add the arg mapping before the existing `claude-skip` branch**

Change the if/else chain at lines 208-212 (the `if (isGitRepo)` block that follows remains unchanged):

```typescript
if (sessionMode === "claude-skip") {
  args.push("--dangerously-skip-permissions");
} else if (sessionMode === "claude-plan") {
  args.push("--plan");
}
```

To:

```typescript
if (sessionMode === "claude-auto") {
  args.push("--auto");
} else if (sessionMode === "claude-skip") {
  args.push("--dangerously-skip-permissions");
} else if (sessionMode === "claude-plan") {
  args.push("--plan");
}
```

- [ ] **Step 2: Commit**

```bash
git add src/stores/sessionStore.ts
git commit -m "feat: map claude-auto mode to --auto CLI flag"
```

### Task 4: Manual verification

- [ ] **Step 1: Run frontend tests**

```bash
npx vitest run
```

Expected: All tests pass (no tests reference SessionMode values directly).

- [ ] **Step 2: Run `npm run tauri dev` and verify**

1. Open the New Session modal
2. Confirm "Claude (auto)" is the first dropdown option
3. Select it, create a session, confirm Claude launches with `--auto`
4. Close and reopen modal — confirm the mode persisted
