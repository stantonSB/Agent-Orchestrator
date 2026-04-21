# Session Project Grouping Design

**Date:** 2026-04-21
**Status:** Approved

## Overview

Group sessions in the right-hand SessionPanel by project (working directory), with collapsible headers. Each unique folder name gets a header row, and sessions opened in that folder appear underneath.

## Approach

Frontend-only grouping (Approach A). The Rust backend already sends `cwd` in `SessionInfo` — the frontend just needs to start using it. No backend changes required.

## Data Model Changes

Add `cwd: string` to the frontend `SessionInfo` type in `src/types/session.ts`:

```typescript
export interface SessionInfo {
  id: string;
  name: string;
  status: SessionStatus;
  createdAt: number;
  cwd: string; // working directory path
}
```

Store `cwd` when creating a session in the Zustand store. The `createSession` action already receives `cwd` as a parameter — include it in the `SessionInfo` object passed to `addSession`.

The `list_sessions` backend call already returns `cwd` (as a Rust `PathBuf`, serialized by Serde as a plain string). Since `cwd` is already lowercase, it maps directly to the frontend field name with no rename needed. `useInitializeSessions` picks it up automatically with no changes needed there.

## Grouping Logic

Grouping happens at render time in `SessionPanel`:

1. **Group sessions by full `cwd` path** — the grouping key is the complete path string, not just the folder name. This prevents sessions from different directories that share the same folder name (e.g., `/work/myapp` and `/personal/myapp`) from being incorrectly merged.
2. **Display name** is the last path segment of the `cwd` (e.g., `/Users/stanton/SProjects/Agent-Orchestrator` → `Agent-Orchestrator`).
3. **Group ordering**: project groups are ordered by the newest session's `createdAt` within each group (most recently active project first).
4. **Within each group**, sessions remain sorted by `createdAt` descending (newest first, same as current behavior).

## New Component: ProjectGroup

A `ProjectGroup` component renders each group within `SessionPanel`:

- **Header row**: clickable, contains chevron + project name + horizontal divider line
- **Body**: the existing `SessionCard` components for that group, hidden when collapsed

### Collapse State

- Stored in `SessionPanel` local React state: `useState<Set<string>>` tracking which `cwd` paths are collapsed.
- Not persisted — all groups start expanded on app launch.
- Clicking the header row toggles the group's collapsed state.

### Accessibility

- Header row has `role="button"`, `tabIndex={0}`, and `aria-expanded` reflecting collapse state.
- Keyboard support: Enter and Space toggle collapse.

## Visual Design

The project group header matches the existing dark theme (`#16161e` background):

| Element | Style |
|---------|-------|
| Chevron | 9px, `#6b7280`, rotates ▶ (collapsed) ↔ ▼ (expanded), CSS transition on transform |
| Project name | 12px, `font-weight: 600`, `#9ca3af` |
| Divider line | 1px solid `rgba(255,255,255,0.08)`, `flex: 1` (extends to right edge) |
| Group spacing | 8px `margin-bottom` between groups |
| Header padding | 4px all around, 6px `padding-bottom` before first session card |
| Click target | Entire header row |

No changes to `SessionCard` styling. Cards sit directly under their header with the existing 2px gap.

## Edge Cases

- **Single project**: Still gets a header — consistent behavior, no special-casing.
- **Empty groups**: When all sessions in a group are dismissed, the group disappears (no empty headers lingering).
- **New session creation**: Session appears under its project group immediately. If it's a new project folder, a new group is created at the top (newest session = top position).
- **Restored sessions** (from `list_sessions` on app restart): Grouped correctly since `cwd` comes from the backend.

## Files Changed

| File | Change |
|------|--------|
| `src/types/session.ts` | Add `cwd: string` field |
| `src/stores/sessionStore.ts` | Include `cwd` in `SessionInfo` during `createSession` |
| `src/components/SessionPanel/SessionPanel.tsx` | Group sessions by project, render `ProjectGroup` components |
| `src/components/SessionPanel/SessionPanel.module.css` | Add styles for project group header |
| `src/components/ProjectGroup/ProjectGroup.tsx` | New component: collapsible project header + session cards |
| `src/components/ProjectGroup/ProjectGroup.module.css` | New styles for the project group |
