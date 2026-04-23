# Session Rename Feature

## Problem

Sessions have a fixed name set at creation time. Users need to rename sessions after creation to better organize their work.

## Design

### Interaction Model

- **Double-click** the session name text in the SessionCard to enter inline edit mode
- **Right-click context menu** includes a "Rename" option that triggers the same edit mode
- Single click continues to select/activate the session as before

### Edit Mode Behavior

- The name `<span>` is replaced with an `<input>` field, pre-filled with the current name, text fully selected
- **Enter** saves the new name (calls `renameSession` store action → Tauri `rename_session` IPC)
- **Escape** cancels editing, reverts to the original name
- **Blur** (clicking away) saves, same as Enter
- **Validation:** non-empty, max 50 characters. If the input is empty or whitespace-only on save, revert to the original name silently

### Components Changed

1. **SessionCard.tsx**
   - Add `isEditing` local state (boolean, default false)
   - Add `onRename` prop: `(id: string, name: string) => void`
   - Double-click handler on the name `<span>` sets `isEditing = true` (with `e.stopPropagation()` to avoid selecting the session)
   - When `isEditing`, render an `<input>` instead of the name span
   - Input: auto-focused, text selected, handles Enter/Escape/blur
   - Save logic: trim, validate non-empty and ≤50 chars, call `onRename`, set `isEditing = false`

2. **SessionCard.module.css**
   - Add `.nameInput` class matching the existing `.name` font/size/weight/color with a subtle border (e.g., `1px solid rgba(99, 102, 241, 0.5)`) and minimal padding to indicate edit mode

3. **SessionPanel.tsx** (or wherever SessionCard is rendered)
   - Pass `onRename` callback wired to `sessionStore.renameSession`

4. **Context menu (in SessionCard.tsx)**
   - Add "Rename" item to `getContextMenuItems()` that sets `isEditing = true`

### Backend (no changes needed)

The following already exist and are fully functional:

- `sessionStore.renameSession(id, name)` — Zustand action
- `rename_session` Tauri IPC command
- `PtyManager::rename_session` — Rust implementation

This is entirely a frontend change.
