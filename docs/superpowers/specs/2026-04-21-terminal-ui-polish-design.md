# Terminal UI Polish — Design Spec

**Date:** 2026-04-21
**Status:** Draft
**Scope:** Bug fixes + terminal chrome polish

## Problem

The embedded xterm.js terminal has two bugs and several cosmetic gaps:

1. **No colors** — Programs (Claude Code, shell prompt) output ANSI escape codes for color, but the PTY environment lacks `TERM=xterm-256color`, so programs fall back to monochrome output.
2. **No visible cursor** — xterm.js is configured with `cursorBlink: true` and `cursorStyle: "bar"`, but `term.focus()` is never called, so the cursor doesn't render.
3. **Padding too tight** — 4px padding makes text feel cramped against the edges.
4. **Background mismatch** — Terminal background (`#1a1b26`) differs slightly from app chrome (`#1a1a2e`), creating a visible seam.
5. **No scrollbar styling** — Default browser scrollbar breaks the dark theme aesthetic.
6. **No focus indicator** — When multiple sessions exist, there's no visual cue which terminal is active.

## Solution

### Fix 1: TERM environment variable

**File:** `src-tauri/src/pty_manager.rs`

After the `shell_env()` loop (line ~286), add:

```rust
cmd.env("TERM", "xterm-256color");
```

This overrides whatever `TERM` value was (or wasn't) captured from the login shell, ensuring programs emit full color output.

### Fix 2: Terminal focus management

**Files:** `src/components/XTermInstance/useTerminal.ts`, `src/components/XTermInstance/XTermInstance.tsx`

Three focus triggers:

1. **On mount** (`useTerminal.ts`): Call `term.focus()` after `term.open(container)` and the initial `fitAddon.fit()` inside the `requestAnimationFrame` callback.

2. **On activation** (`XTermInstance.tsx`): When `isActive` transitions from `false` to `true`, call `getTerminal()?.focus()` alongside the existing `fit()` call.

3. **On click** (`XTermInstance.tsx`): Add an `onClick` handler on the container div that calls `getTerminal()?.focus()`, so clicking anywhere in the terminal area grabs focus.

### Polish 1: Increased padding

**File:** `src/components/XTermInstance/XTermInstance.module.css`

Change container padding from `4px` to `10px`.

### Polish 2: Background alignment

**Files:** `src/components/XTermInstance/useTerminal.ts`, `src/components/XTermInstance/XTermInstance.module.css`

- Update `THEME.background` from `#1a1b26` to `#1a1a2e` (matches `--bg-primary`).
- Update `THEME.cursorAccent` from `#1a1b26` to `#1a1a2e`.
- Update CSS `.container` background from `#1a1b26` to `var(--bg-primary)`.

### Polish 3: Scrollbar styling

**File:** `src/components/XTermInstance/XTermInstance.module.css`

Target `.xterm-viewport` inside the container:

- Width: 8px
- Track: transparent (blends with terminal background)
- Thumb: `var(--border-color)` with border-radius, brightening on hover
- Firefox: `scrollbar-width: thin; scrollbar-color: var(--border-color) transparent`

### Polish 4: Focus indicator

**File:** `src/components/XTermInstance/XTermInstance.module.css`

Add an `.active` class variant with a 2px left border using `var(--accent)` color. Apply this class in `XTermInstance.tsx` when `isActive` is true.

## Files Changed

| File | Change |
|------|--------|
| `src-tauri/src/pty_manager.rs` | Add `TERM=xterm-256color` env override |
| `src/components/XTermInstance/useTerminal.ts` | Add `term.focus()` on mount; align background color |
| `src/components/XTermInstance/XTermInstance.tsx` | Add focus-on-activate, click-to-focus, active class |
| `src/components/XTermInstance/XTermInstance.module.css` | Padding, background, scrollbar, focus indicator |

## Testing

- Launch app, create new session — cursor should blink immediately
- Claude Code startup screen should show colored output (orange robot, green status bar, yellow links)
- Switch between sessions — cursor should appear in the newly active terminal
- Click on terminal area — should grab focus
- Scroll output — scrollbar should be thin, dark, and match the theme
- Active terminal should have a subtle blue left border accent
