# UX Improvements: Session Cycling, Default Names, Quit Confirmation, Settings

**Date:** 2026-05-26
**Status:** Approved

## Overview

Four UX improvements to reduce friction in daily Agent Orchestrator usage:

1. **Session cycling** — Navigate between sessions with Cmd+Shift+[ / ]
2. **Default session names** — Pre-filled placeholder names so sessions can be created without typing a name
3. **Quit confirmation** — "Quit Agent Orchestrator?" dialog on app close
4. **Configurable naming** — Settings modal to customize the default session name pattern

---

## Feature 1: Session Cycling Keybindings

### Behavior

| Shortcut | Action |
|---|---|
| `Cmd+Shift+[` | Switch to previous session in sidebar order |
| `Cmd+Shift+]` | Switch to next session in sidebar order |

- Uses the existing `orderedSessionIds` array (derived from `groupSessionsByProject` in `App.tsx`)
- Wraps around: last → first, first → last
- No-op if 0 or 1 sessions exist

### Implementation

**`useGlobalKeybindings.ts`:**
- Add `onCyclePrev` and `onCycleNext` to the `GlobalKeybindingActions` interface
- Use `e.code` (layout-independent) for detection: `e.metaKey && e.shiftKey && e.code === "BracketLeft"` → `onCyclePrev()`
- `e.metaKey && e.shiftKey && e.code === "BracketRight"` → `onCycleNext()`
- Note: `e.key` would produce `{`/`}` with Shift held, so `e.code` is required here

**`App.tsx`:**
- Add `handleCyclePrev` and `handleCycleNext` callbacks:
  ```ts
  const handleCyclePrev = useCallback(() => {
    if (orderedSessionIds.length <= 1 || !activeSessionId) return;
    const idx = orderedSessionIds.indexOf(activeSessionId);
    const prevIdx = idx <= 0 ? orderedSessionIds.length - 1 : idx - 1;
    setActiveSession(orderedSessionIds[prevIdx]);
  }, [orderedSessionIds, activeSessionId, setActiveSession]);
  ```
- Pass both callbacks to `useGlobalKeybindings`

### Files Changed

- `src/hooks/useGlobalKeybindings.ts`
- `src/App.tsx`

---

## Feature 2: Default Session Names

### Behavior

- The session name input field in `NewSessionModal` starts **empty** with a grey placeholder like "Session 3"
- The placeholder uses native HTML `placeholder` attribute — grey text that disappears when the user starts typing
- If the user submits the form with an empty name field, the current placeholder value is used as the session name
- The Create button is enabled when a directory is selected (name is no longer required)
- A module-level counter increments with each session created during the app lifecycle

### Counter Logic

```ts
let sessionCounter = 0;

function getNextSessionNumber(): number {
  return ++sessionCounter;
}
```

- Counter starts at 0, increments before use (first session = 1)
- Never resets during app lifecycle
- On app restart, resets to 0 (intentional — keeps it simple, avoids stale numbers)
- The placeholder is computed when the modal opens: `getDefaultName(getNextSessionNumber())`
  - Actually, the number should be computed when the modal opens but only consumed (counter incremented) on create. Use a `useRef` to hold the next number, computed on modal open.

**Revised approach:** On modal open, peek at `sessionCounter + 1` for the placeholder. On create, if using default name, call `getNextSessionNumber()` to consume it. If user provides a custom name, don't increment.

**Note:** If the user opens the modal, cancels, and reopens, they'll see the same suggested number. If they create with a custom name, the next modal will still suggest the same number. This is intentional — the counter only advances when a default name is actually used.

### Implementation

**`NewSessionModal.tsx`:**
- Add `getDefaultSessionName(n: number): string` that reads the localStorage pattern (Feature 4) or falls back to `Session {n}`
- Replace `{n}` with the number
- Set as `placeholder` on the name input
- Remove name from the `isValid` check (only `directory !== null` required)
- Update `handleCreate`'s early return guard (`if (!trimmedName || !directory) return`) to allow empty names — substitute the default name before the guard
- Update `handleKeyDown`'s Enter-key guard (`name.trim() && directory`) to only require directory
- In `handleCreate`: if `name.trim()` is empty, use the default name and increment the counter

### Files Changed

- `src/components/NewSessionModal/NewSessionModal.tsx`

---

## Feature 3: Quit Application Confirmation

### Behavior

- When the user clicks the close button (traffic light or Cmd+Q), show a confirmation dialog
- Always shown regardless of session state
- Dialog:
  - Title: "Quit Agent Orchestrator?"
  - Message: "All running sessions will be terminated. Session state will be saved."
  - Buttons: "Cancel" (secondary) | "Quit" (primary/destructive)
- "Quit" → save sessions (existing logic) → `appWindow.destroy()`
- "Cancel" → dismiss dialog, app stays open

### Implementation

**`sessionStore.ts`:**
- Add `showQuitConfirm: boolean` state
- Add `setShowQuitConfirm: (show: boolean) => void` action

**`useSaveOnClose.ts`:**
- The existing `event.preventDefault()` call stays in place
- Replace the save-and-destroy logic with: `useSessionStore.getState().setShowQuitConfirm(true)`
- The save-and-destroy logic moves to the quit confirm handler in `App.tsx`
- Access the store via `useSessionStore.getState()` (not a hook selector) since this runs inside a `useEffect` callback

**`App.tsx`:**
- Read `showQuitConfirm` from store
- Render a `QuitConfirmDialog` (or reuse `CloseConfirmDialog` with adapted props) as a portal
- On confirm: run the save-sessions logic (extracted from `useSaveOnClose`), then `appWindow.destroy()`
- On cancel: set `showQuitConfirm = false`

**New component `QuitConfirmDialog`:**
- Reuse the same visual pattern as `CloseConfirmDialog` (overlay + dialog box)
- Could be a generic `ConfirmDialog` component, or a separate small component
- Recommended: create a minimal `QuitConfirmDialog` to avoid over-abstracting

### Files Changed

- `src/stores/sessionStore.ts`
- `src/hooks/useSaveOnClose.ts`
- `src/App.tsx`
- New: `src/components/QuitConfirmDialog/QuitConfirmDialog.tsx`
- New: `src/components/QuitConfirmDialog/QuitConfirmDialog.module.css` (can copy from CloseConfirmDialog)

---

## Feature 4: Configurable Default Name Pattern

### Behavior

- A **Settings modal** accessible via a gear icon in the title bar
- Contains a "Default session name" text input
- Placeholder: `Session {n}` (showing the default pattern)
- `{n}` token is replaced with the auto-incrementing number at session creation time
- If the pattern doesn't contain `{n}`, the name is used as-is (all sessions get the same default name)
- Stored in localStorage key `ao-default-session-name`
- If not set or empty, falls back to `Session {n}`

### Settings Modal

- Simple modal with a single setting for now (expandable later)
- Title: "Settings"
- Field: "Default session name" with text input
- Placeholder on the input: `Session {n}`
- "Save" and "Cancel" buttons
- Escape key closes
- `Cmd+,` keyboard shortcut opens Settings (macOS convention), added to `useGlobalKeybindings`

### Title Bar Integration

- Add a small gear icon (SVG) to the right side of the title bar, before the drag region ends
- On click, opens the Settings modal
- Styled subtly to match the existing traffic light aesthetic

### Implementation

**New `SettingsModal` component:**
- Reads current value from `localStorage.getItem("ao-default-session-name")`
- On save, writes to localStorage
- Renders as an overlay/modal (same pattern as `NewSessionModal`)

**`TitleBar.tsx`:**
- Add `onSettingsClick` prop to `TitleBar` (settings open state lives in `App.tsx`, consistent with `NewSessionModal` pattern)
- Add gear icon button in a new right-aligned container
- CSS: Add a `rightControls` container to balance the layout — the title bar currently uses flex with `windowControls` on the left and `title` with `flex: 1`. Add a right-side element so the title stays centered
- On click, calls `onSettingsClick`

**`NewSessionModal.tsx`:**
- `getDefaultSessionName(n)` reads `localStorage.getItem("ao-default-session-name")` or defaults to `Session {n}`
- Replaces `{n}` with the number

### Files Changed

- New: `src/components/SettingsModal/SettingsModal.tsx`
- New: `src/components/SettingsModal/SettingsModal.module.css`
- `src/components/TitleBar/TitleBar.tsx`
- `src/components/TitleBar/TitleBar.module.css`
- `src/App.tsx` (settings modal state + render)
- `src/components/NewSessionModal/NewSessionModal.tsx`

---

## Testing Strategy

### Unit Tests

- **Session cycling:** Test wrap-around logic, no-op with 0/1 sessions
- **Default names:** Test counter incrementing, pattern replacement, fallback behavior
- **Quit confirm:** Test that dialog appears on close request, cancel dismisses, confirm triggers save+destroy

### Manual Tests

- Cmd+Shift+[ and ] cycle correctly through sessions in sidebar order
- New session modal shows grey placeholder, submitting empty uses default name
- Closing app always shows quit dialog
- Settings modal saves and the pattern is reflected in new session placeholder

---

## Implementation Order

1. Feature 1 (session cycling) — standalone, no dependencies
2. Feature 2 (default names) — standalone
3. Feature 4 (configurable naming) — builds on Feature 2's `getDefaultSessionName`
4. Feature 3 (quit confirmation) — standalone, touches `useSaveOnClose`

Features 1 and 2 can be implemented in parallel. Feature 4 depends on Feature 2. Feature 3 is independent.
