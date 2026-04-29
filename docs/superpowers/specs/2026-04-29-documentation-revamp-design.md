# Documentation Revamp — Design Spec

**Date:** 2026-04-29
**Status:** Draft

## Goal

Restructure Agent Orchestrator's documentation so the README serves as a visual sales pitch for developers already using Claude Code, with a glossary table linking to individual doc files in `docs/` for detailed reference.

## Target Audience

Developers already using Claude Code who want to run multiple sessions in parallel. No need to explain what Claude Code is — focus on the orchestration value prop.

## README.md Structure

The README has 6 sections, visual-heavy at the top, reference links at the bottom.

### Section 1 — Hero

- App name, one-line tagline, badge row (macOS, Tauri 2, version number)
- Tagline: "Run multiple Claude Code sessions in parallel. Monitor status. Switch instantly. One window."

### Section 2 — Hero Screenshot

- Full-width screenshot of the app with 5+ sessions across 2+ project groups
- **Must show all 5 active statuses:** Working, Idle, Needs Attention, Finished, Error (exclude Starting — it's transient)
- This is the first visual impression; it should convey the app's value at a glance
- Asset: `assets/hero.png`

### Section 3 — Feature Highlights

2x2 grid, each cell has a heading, 2-3 sentence description, and a screenshot.

| Feature | Description focus | Asset |
|---------|------------------|-------|
| Parallel Sessions | Run 5+ agents simultaneously, each with full terminal emulation, 10k scrollback | `assets/feature-parallel-sessions.png` |
| Real-Time Status | Hook-driven detection: Working, Idle, Needs Attention, Finished, Error — no output parsing | `assets/feature-status.png` |
| Project Grouping | Sessions grouped by working directory in collapsible sidebar groups | `assets/feature-project-groups.png` |
| Worktree Isolation | Each session gets `--worktree` by default for isolated git branches | Text-only (no dedicated screenshot) |

### Section 4 — Quick Install

Three steps maximum:

1. Download `.dmg` from Releases page (link)
2. Run `xattr -dr com.apple.quarantine /Applications/Agent\ Orchestrator.app`
3. Open the app

Link to `docs/installation.md` for details and troubleshooting.

### Section 5 — Quick Start

Short description of the core workflow: open app → Cmd+T → type prompt → go. Accompanied by a 10-15 second video showing this loop.

- Asset: `assets/quick-start.mp4` (mp4 for GitHub inline video rendering; `.mov` won't embed)

### Section 6 — Documentation Glossary

Markdown table linking to each doc in `docs/`:

| Document | Description |
|----------|-------------|
| [Installation](docs/installation.md) | Download, Gatekeeper bypass, prerequisites, first launch |
| [Architecture](docs/architecture.md) | System design, component deep-dives, data flow |
| [Development](docs/development.md) | Setup, build, test, release, IDE configuration |
| [How Status Works](docs/how-status-works.md) | Hook protocol, state machine, notify script |
| [Keyboard Shortcuts](docs/keyboard-shortcuts.md) | All shortcuts and navigation |
| [Troubleshooting](docs/troubleshooting.md) | Common issues and fixes |

## Individual Doc Files

All live in `docs/` at the repo root.

### docs/installation.md

- Prerequisites: macOS, Claude Code CLI installed
- Download DMG from GitHub Releases
- Gatekeeper bypass (`xattr` command) with explanation of why it's needed
- First launch: what happens (hook installation, env capture)
- Verifying the installation works

### docs/architecture.md

Layered structure — high-level overview first, then deep-dives.

**High-level section:**
- System diagram (ASCII or Mermaid): Tauri WebView ↔ Rust backend (PTY Manager, Status Server, Hook Installer, Env Capture)
- One paragraph per component explaining its role

**Deep-dive sections (one per subsystem):**
- **PTY Manager** — dedicated OS thread, mpsc channel interface, why handles aren't Send/Sync, reader threads, startup timers
- **Status Server** — tiny_http on `127.0.0.1:0`, endpoint format, request/response protocol, session ID routing
- **Status Parser** — state machine with 6 states (Starting, Working, Idle, NeedsAttention, Finished, Error), transition rules, event types
- **Hook Installer** — what files get created/modified (`~/.claude/agent-orchestrator-notify.sh`, `~/.claude/settings.json`, `~/.claude.json`), idempotency
- **Frontend** — Zustand store, CSS show/hide terminal strategy, xterm.js configuration, component hierarchy
- **Environment Capture** — `$SHELL -li -c env` on startup, why this is needed for .app bundles

### docs/development.md

Absorbs the current `DEVELOPMENT.md` content:

- Prerequisites (Node.js 18+, Rust stable, Xcode CLT)
- `npm install` setup
- `npm run tauri dev` for development
- Testing: `npx vitest run` (frontend), `cd src-tauri && cargo test` (backend)
- Building: `npm run tauri build` → .app + DMG output paths
- Releasing: `npm run release:patch/minor/major`
- IDE setup (VS Code + Tauri + rust-analyzer extensions)
- Project file layout overview

### docs/how-status-works.md

- Problem: how does the app know what Claude Code is doing?
- Solution: hook-driven status detection (no output parsing)
- What gets installed on first launch (the 3 files/configs)
- The notify script flow: Claude Code fires hook → bash script → curl POST → status server → state transition → frontend update
- State machine diagram showing all 6 states and their transitions
- Hook event types: `idle_prompt`, `permission_prompt`, `elicitation_dialog`, `Stop`
- HTTP endpoint format: `POST /status/{ao_session_id}` with request/response examples

### docs/keyboard-shortcuts.md

- Table of all shortcuts: Cmd+T (new session), Cmd+W (close), Cmd+1-9 (switch)
- Any context-specific behaviors

### docs/troubleshooting.md

Expanded from the current README troubleshooting section:

- Status stuck on "Starting" — hook script not installed, how to verify
- "claude" not found — PATH not captured, how to fix
- Status shows "Needs Attention" but no visible prompt — scroll up to find it
- Sessions not creating — prerequisites missing
- App won't open — Gatekeeper, xattr command

## File Cleanup

Remove these files/directories:

- `docs/superpowers/` — internal design specs and implementation plans, served their purpose during development
- `docs/INDEX.md` — replaced by the README glossary table
- `DEVELOPMENT.md` (root) — content moves to `docs/development.md`
- `docs/future-phases.md` — redundant pointer to the `docs/future-phases/` folder

Keep `docs/future-phases/` directory (tech-debt.md, nested-subagent-terminals.md) — these are internal backlog items, not user-facing, but still useful for contributors. They don't need to appear in the glossary.

## Media Assets

All stored in `assets/` at repo root.

| Asset | Type | Description |
|-------|------|-------------|
| `hero.png` | Screenshot | Full app window, 5+ sessions, 2+ project groups, all 5 active statuses visible (Working, Idle, Needs Attention, Finished, Error) |
| `feature-parallel-sessions.png` | Screenshot | Sidebar showing 5+ sessions to convey scale |
| `feature-status.png` | Screenshot | Close-up of session cards with different status dots and activity pulse |
| `feature-project-groups.png` | Screenshot | Sidebar with 2-3 project groups, mix of collapsed/expanded |
| `quick-start.mp4` | Video | 10-15 sec: open app → Cmd+T → type prompt → session starts working |

The existing `assets/init-2-sessions.mov` can be removed once `quick-start.mp4` replaces it.

## Media Guidelines

- All screenshots at 2x Retina resolution, ~1400px wide for consistent appearance
- Use a consistent window size across all screenshots
- Video format: `.mp4` (GitHub renders inline; `.mov` renders as download link only)

## Out of Scope

- Cross-platform docs (macOS only for now)
- GitHub Wiki or static docs site
- Contributing guide (can be added later)
- API documentation (no public API)
