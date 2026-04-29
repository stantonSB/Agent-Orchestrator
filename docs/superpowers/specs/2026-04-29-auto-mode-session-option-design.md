# Auto Mode Session Option

## Summary

Add a "Claude (auto)" option to the session creation dropdown that launches Claude Code with the `--auto` flag, which auto-approves all tool calls without permission prompts.

## Changes

### 1. Type Definition (`src/types/session.ts`)

Add `"claude-auto"` to the `SessionMode` union:

```typescript
export type SessionMode = "claude-auto" | "claude" | "claude-skip" | "claude-plan" | "terminal";
```

### 2. Dropdown UI (`src/components/NewSessionModal/NewSessionModal.tsx`)

Add "Claude (auto)" as the first option in the session mode `<select>`:

```
Claude (auto)              → "claude-auto"
Claude                     → "claude"
Claude (skip permissions)  → "claude-skip"
Claude (plan mode)         → "claude-plan"
Terminal                   → "terminal"
```

### 3. Arg Mapping (`src/stores/sessionStore.ts`)

In `createSession`, map the new mode to CLI args:

```typescript
if (sessionMode === "claude-auto") {
  args.push("--auto");
} else if (sessionMode === "claude-skip") {
  args.push("--dangerously-skip-permissions");
} else if (sessionMode === "claude-plan") {
  args.push("--plan");
}
```

## Backend Impact

None. The Rust backend forwards arbitrary args to the PTY command. No changes needed.

## Persistence

The existing `localStorage` key `"ao-last-session-mode"` handles the new value automatically.

## Files Touched

- `src/types/session.ts`
- `src/components/NewSessionModal/NewSessionModal.tsx`
- `src/stores/sessionStore.ts`
