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

After the `shell_env()` loop (line ~286), add — placement after the loop is critical so it overrides any `TERM` value the user's shell may have set (e.g., `screen` inside tmux):

```rust
cmd.env("TERM", "xterm-256color");
cmd.env("COLORTERM", "truecolor");
```

`TERM` tells programs to emit 256-color ANSI codes. `COLORTERM=truecolor` tells modern CLI tools (including Node.js-based ones) that 24-bit color is supported, which xterm.js fully handles.

### Fix 2: Terminal focus management

**Files:** `src/components/XTermInstance/useTerminal.ts`, `src/components/XTermInstance/XTermInstance.tsx`

**Important:** `XTermInstance.tsx` currently destructures only `{ containerRef, write, fit }` from `useTerminal()`. The `getTerminal` function must also be destructured (i.e., `{ containerRef, write, fit, getTerminal }`) for the focus triggers below.

Three focus triggers:

1. **On mount** (`useTerminal.ts`): Call `term.focus()` after `term.open(container)` and the initial `fitAddon.fit()` inside the `requestAnimationFrame` callback. **Only call focus if the terminal is visible** — if the component mounts in a `display: none` state (non-active tabs), `focus()` silently fails. The on-activation trigger (item 2) covers those cases.

2. **On activation** (`XTermInstance.tsx`): When `isActive` transitions from `false` to `true`, call `getTerminal()?.focus()` alongside the existing `fit()` call.

3. **On click** (`XTermInstance.tsx`): Add an `onClick` handler on the container div that calls `getTerminal()?.focus()`, so clicking anywhere in the terminal area grabs focus.

Focus management is internal to the component — no changes to `XTermInstanceHandle` needed.

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

Add a 2px left border to the base `.container` class using `transparent` color (reserves space to prevent layout shift). Add an `.active` class variant that changes the border color to `var(--accent)`. Apply the `.active` class in `XTermInstance.tsx` when `isActive` is true.

## Files Changed

| File | Change |
|------|--------|
| `src-tauri/src/pty_manager.rs` | Add `TERM=xterm-256color` and `COLORTERM=truecolor` env overrides |
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
