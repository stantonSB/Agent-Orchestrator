# Documentation Revamp Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure all documentation so the README is a visual sales pitch with a glossary linking to 6 individual doc files in `docs/`.

**Architecture:** Replace the current monolithic README with a 6-section visual layout (hero → screenshot → features → install → quick start → glossary). Extract detailed content into standalone docs. Remove internal design specs and consolidate dev docs.

**Tech Stack:** Markdown, GitHub-flavored markdown tables, embedded images/video

---

## Chunk 1: File Cleanup

Remove internal docs that are being replaced. Do this first so we start from a clean state.

> **Important:** This chunk deletes `docs/superpowers/` which contains this plan file and the design spec. Ensure you have fully loaded/copied the plan before executing this chunk.

### Task 1: Remove internal design specs and redundant files

**Files:**
- Delete: `docs/superpowers/` (entire directory)
- Delete: `docs/INDEX.md`
- Delete: `docs/future-phases.md`
- Delete: `DEVELOPMENT.md`
- Delete: `assets/init-2-sessions.mov`

- [ ] **Step 1: Delete the docs/superpowers/ directory**

```bash
git rm -r docs/superpowers/
```

- [ ] **Step 2: Delete docs/INDEX.md**

```bash
git rm docs/INDEX.md
```

- [ ] **Step 3: Delete docs/future-phases.md**

```bash
git rm docs/future-phases.md
```

- [ ] **Step 4: Delete root DEVELOPMENT.md**

```bash
git rm DEVELOPMENT.md
```

- [ ] **Step 5: Delete old video asset**

```bash
git rm assets/init-2-sessions.mov
```

- [ ] **Step 6: Commit the cleanup**

```bash
git commit -m "docs: remove internal design specs and redundant files

Remove docs/superpowers/ (design specs and plans), docs/INDEX.md,
docs/future-phases.md, root DEVELOPMENT.md, and old init video.
Content is being restructured into user-facing docs."
```

---

## Chunk 2: Create Individual Doc Files

Create all 6 doc files in `docs/`. Each task is one doc file, committed independently.

### Task 2: Create docs/installation.md

**Files:**
- Create: `docs/installation.md`

Source material: current README lines 30-43 (Installation section).

- [ ] **Step 1: Write docs/installation.md**

```markdown
# Installation

## Prerequisites

- **macOS** (Apple Silicon or Intel)
- **[Claude Code](https://docs.anthropic.com/en/docs/claude-code)** CLI installed and available on your PATH

## Download

Download the latest `.dmg` from the [Releases](https://github.com/stantonSB/Agent-Orchestrator/releases) page.

## Install

1. Open the `.dmg` and drag **Agent Orchestrator** to your Applications folder.

2. Since the app is not yet code-signed with an Apple Developer certificate, macOS Gatekeeper will block it on first launch. Run this command once to allow it:

   ```bash
   xattr -dr com.apple.quarantine /Applications/Agent\ Orchestrator.app
   ```

3. Open the app normally. This only needs to be done once after downloading.

> **Why is the xattr command needed?** When macOS downloads a file via a browser, it attaches a quarantine attribute. Unsigned apps with this attribute are blocked by Gatekeeper. The command above removes the quarantine flag.

## First Launch

On first launch, the app automatically:

1. **Captures your shell environment** — runs `$SHELL -li -c env` to pick up your PATH, NODE_EXTRA_CA_CERTS, and other profile variables. This ensures sessions spawned from the `.app` bundle can find `claude`, `node`, etc.

2. **Installs status hooks** — creates `~/.claude/agent-orchestrator-notify.sh` and adds Notification/Stop hook entries to `~/.claude/settings.json`. This is how the app detects session status. See [How Status Works](how-status-works.md) for details.

3. **Sets idle threshold** — sets `messageIdleNotifThresholdMs: 500` in `~/.claude.json` to lower the idle notification delay.

All of this is idempotent — re-launching the app won't duplicate entries.

## Verifying It Works

1. Open Agent Orchestrator
2. Press `Cmd+T` to create a new session
3. Type a prompt and press Enter
4. The session card in the sidebar should show a green "Working" status dot
```

- [ ] **Step 2: Commit**

```bash
git add docs/installation.md
git commit -m "docs: add installation guide"
```

### Task 3: Create docs/architecture.md

**Files:**
- Create: `docs/architecture.md`

Source material: current README lines 46-69 (Architecture section), plus source code in `src-tauri/src/` for deep-dive details.

- [ ] **Step 1: Write docs/architecture.md**

```markdown
# Architecture

## Overview

Agent Orchestrator is a Tauri 2 desktop app with a Rust backend and a React frontend rendered in a WebView.

```
┌─────────────────────────────────────────────────────┐
│  Tauri WebView (React 19 + xterm.js + Zustand)      │
│  ┌──────────────────────┐ ┌───────────────────────┐ │
│  │  Terminal Area        │ │  Session Panel        │ │
│  │  (XTermInstance ×N)   │ │  (ProjectGroup ×N)    │ │
│  │  CSS show/hide        │ │  (SessionCard ×N)     │ │
│  └──────────┬───────────┘ └───────────────────────┘ │
│             │ IPC (invoke)                           │
├─────────────┼───────────────────────────────────────┤
│  Rust Backend                                        │
│  ┌──────────▼───────────┐ ┌───────────────────────┐ │
│  │  PTY Manager Thread   │ │  Status HTTP Server   │ │
│  │  (mpsc channels)      │ │  (tiny_http, :0)      │ │
│  │  portable-pty         │ │  POST /status/{id}    │ │
│  └──────────────────────┘ └───────────────────────┘ │
│  ┌──────────────────────┐ ┌───────────────────────┐ │
│  │  Hook Installer       │ │  Env Capture          │ │
│  │  (~/.claude/ files)   │ │  ($SHELL -li -c env)  │ │
│  └──────────────────────┘ └───────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

**Rust backend** (`src-tauri/src/`) handles PTY management, status tracking, hook installation, and environment capture. **React frontend** (`src/`) renders terminals, session sidebar, and manages UI state via Zustand. Communication between them happens through Tauri's IPC invoke mechanism.

## PTY Manager

**File:** `src-tauri/src/pty_manager.rs`

The PTY manager runs on a **dedicated OS thread**. This is required because `portable-pty` handles are not `Send` or `Sync` — they cannot be shared across threads.

All external code communicates with the PTY manager through an **mpsc channel**. Callers send `PtyRequest` messages and receive responses via oneshot channels. This pattern keeps all PTY state on a single thread while allowing the rest of the app to interact with it safely.

For each session, the manager:
- Spawns a PTY with the user's shell environment (see Environment Capture below)
- Creates a **reader thread** that forwards PTY output to the frontend via Tauri events
- Starts a **startup timer** (5 seconds) — if no hook event arrives in time, the session transitions from Starting → Idle

Session types: `Claude` (runs `claude` CLI) or `Terminal` (plain shell).

## Status Server

**File:** `src-tauri/src/status_server.rs`

A `tiny_http` server bound to `127.0.0.1:0` (OS-assigned port). It receives hook events from Claude Code via HTTP POST and routes them to the correct session's `StatusTracker`.

**Endpoint:** `POST /status/{ao_session_id}`

The server runs on its own thread and processes requests in an accept loop. Each request is matched to a session by the `ao_session_id` path parameter, which corresponds to the `AO_SESSION_ID` environment variable set when the PTY was created.

Response codes:
- `200` — status transition occurred
- `204` — no transition (event didn't change state)
- `400` — bad request (invalid JSON, missing fields)
- `404` — unknown session ID
- `405` — not a POST request

See [How Status Works](how-status-works.md) for the full event flow.

## Status Parser (State Machine)

**File:** `src-tauri/src/status_parser.rs`

Each session has a `StatusTracker` that implements a state machine with 6 states:

```
Starting ──────────────────────────────────────────────┐
  │ idle_prompt / stop hook / 5s timeout → Idle        │
  │ permission_prompt / elicitation_dialog → NeedsAttn  │
  │                                                     │
Idle ◄──────────────────────────────────────────────────┤
  │ user presses Enter → Working                       │
  │                                                     │
Working                                                 │
  │ idle_prompt / stop hook → Finished                 │
  │ user presses Escape → Finished                     │
  │ permission_prompt / elicitation_dialog → NeedsAttn  │
  │                                                     │
NeedsAttention                                          │
  │ idle_prompt → Finished                             │
  │ user presses Enter → Working                       │
  │                                                     │
Finished                                                │
  │ user presses Enter → Working                       │
  │                                                     │
Any state ── process exits ──→ Finished or Error       │
└──────────────────────────────────────────────────────┘
```

The state machine is **purely hook-driven** — it never parses terminal output to determine status.

## Hook Installer

**File:** `src-tauri/src/hook_installer.rs`

On startup, the app ensures Claude Code hooks are installed. Three things are set up:

1. **`~/.claude/agent-orchestrator-notify.sh`** — a bash script that forwards hook events via `curl` to the status server. It no-ops silently when the app isn't running.

2. **`~/.claude/settings.json`** — `Notification` and `Stop` hook entries are merged in, pointing to the script above.

3. **`~/.claude.json`** — `messageIdleNotifThresholdMs` is set to 500ms.

Installation is idempotent — if hooks are already installed, no changes are made.

## Frontend

**Source:** `src/`

| File/Directory | Responsibility |
|----------------|----------------|
| `stores/sessionStore.ts` | Zustand store: sessions map, active session, toast state. Manages all Tauri IPC calls and event listeners. |
| `components/TerminalArea/` | Renders all `XTermInstance` components simultaneously using CSS show/hide (not mount/unmount) to preserve scrollback. |
| `components/XTermInstance/` | xterm.js wrapper. Tokyo Night theme, 10k-line scrollback, WebLinksAddon, file path click support. |
| `components/SessionPanel/` | Sidebar grouping sessions by project (working directory). Contains `ProjectGroup` and `SessionCard`. |
| `components/SessionCard/` | Status dot, session name, duration timer, activity pulse, context menu. |
| `hooks/useGlobalKeybindings.ts` | Keyboard shortcuts: Cmd+T, Cmd+W, Cmd+1-9. |
| `lib/tauri-ipc.ts` | Typed wrappers around Tauri invoke calls. |

**Key design decision:** Terminals are never unmounted when switching sessions — they are hidden via CSS. This preserves scrollback history and terminal state. The `isActive` prop controls visibility.

## Environment Capture

**File:** `src-tauri/src/pty_manager.rs` (`shell_env()` function)

macOS `.app` bundles launched from Finder inherit a minimal environment (`PATH=/usr/bin:/bin:/usr/sbin:/sbin`). The user's shell profile variables (custom PATH entries, NODE_EXTRA_CA_CERTS, etc.) are not present.

On startup, the app runs `$SHELL -li -c env` once, parses the output into a key-value map, and caches it for the process lifetime via `OnceLock`. All PTY sessions are spawned with this captured environment. If the capture fails, the app falls back to its own (minimal) environment.
```

- [ ] **Step 2: Commit**

```bash
git add docs/architecture.md
git commit -m "docs: add architecture guide with component deep-dives"
```

### Task 4: Create docs/development.md

**Files:**
- Create: `docs/development.md`

Source material: current `DEVELOPMENT.md` (root), CLAUDE.md file layout section.

- [ ] **Step 1: Write docs/development.md**

```markdown
# Development Guide

## Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Xcode Command Line Tools](https://developer.apple.com/xcode/) (macOS)

## Setup

```bash
npm install
```

## Development

```bash
# Start the app in development mode (hot-reload enabled)
npm run tauri dev
```

## Testing

```bash
# Frontend tests (Vitest + React Testing Library)
npx vitest run

# Backend tests (Rust)
cd src-tauri && cargo test
```

## Building

### Build the app + DMG installer (macOS)

```bash
npm run tauri build
```

Output files:

- **App bundle:** `src-tauri/target/release/bundle/macos/Agent Orchestrator.app`
- **DMG installer:** `src-tauri/target/release/bundle/dmg/Agent Orchestrator_<version>_aarch64.dmg`

> The architecture suffix will be `aarch64` on Apple Silicon or `x86_64` on Intel Macs.

### Frontend only

```bash
# Type-check and build the frontend
npm run build

# Preview the frontend build
npm run preview
```

## Releasing

```bash
# Bump version, tag, and push
npm run release:patch   # 0.1.1 → 0.1.2
npm run release:minor   # 0.1.1 → 0.2.0
npm run release:major   # 0.1.1 → 1.0.0
```

## IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Project Structure

```
src/                          # React frontend
  App.tsx                     # Root component
  main.tsx                    # Entry point
  stores/
    sessionStore.ts           # Zustand store (sessions, IPC, events)
  components/
    TerminalArea/             # Terminal rendering (CSS show/hide)
    XTermInstance/             # xterm.js wrapper + file path links
    SessionPanel/             # Sidebar with project groups
    SessionCard/              # Session status, name, timer
    ProjectGroup/             # Collapsible project section
    NewSessionModal/          # Create session dialog
    SubagentList/             # Subagent display within sessions
    ...                       # Toast, TitleBar, ContextMenu, etc.
  hooks/
    useGlobalKeybindings.ts   # Cmd+T, Cmd+W, Cmd+1-9
    useInitializeSessions.ts  # Restore sessions on app start
  lib/
    tauri-ipc.ts              # Typed Tauri invoke wrappers
  types/
    session.ts                # Session TypeScript types
    tauri-events.ts           # Tauri event payload types

src-tauri/src/                # Rust backend
  main.rs                     # Tauri app entry point
  lib.rs                      # Plugin registration
  commands.rs                 # Tauri IPC command handlers
  pty_manager.rs              # PTY lifecycle (dedicated thread)
  status_parser.rs            # Session status state machine
  status_server.rs            # HTTP server for hook events
  hook_installer.rs           # Auto-install Claude Code hooks
  state.rs                    # AppState (PtyManager + StatusServer)
  subagent_tracker.rs         # Track nested subagent sessions
```
```

- [ ] **Step 2: Commit**

```bash
git add docs/development.md
git commit -m "docs: add development guide"
```

### Task 5: Create docs/how-status-works.md

**Files:**
- Create: `docs/how-status-works.md`

Source material: current README "How status detection works" section, `status_parser.rs`, `status_server.rs`, `hook_installer.rs`.

- [ ] **Step 1: Write docs/how-status-works.md**

```markdown
# How Status Detection Works

## The Problem

When running multiple Claude Code sessions, you need to know what each one is doing — is it working, waiting for input, or finished? Parsing terminal output for status clues is fragile and unreliable.

## The Solution: Hook-Driven Detection

Agent Orchestrator uses Claude Code's built-in hook system. When Claude Code changes state (goes idle, needs permission, finishes a task), it fires a hook. A small bash script forwards that event to the app via HTTP. The app updates the session status instantly.

**No terminal output is ever parsed to determine status.**

## What Gets Installed

On first launch, the app installs three things (idempotent — re-launching won't duplicate anything):

### 1. Notify Script

**`~/.claude/agent-orchestrator-notify.sh`**

```bash
#!/bin/bash
# Forward Claude Code notifications to Agent Orchestrator.
# No-ops silently when Agent Orchestrator is not running.
if [ -n "$AO_STATUS_PORT" ] && [ -n "$AO_SESSION_ID" ]; then
    curl -s -X POST "http://127.0.0.1:${AO_STATUS_PORT}/status/${AO_SESSION_ID}" \
        -H "Content-Type: application/json" -d @- 2>/dev/null || true
fi
```

The script checks for two environment variables that Agent Orchestrator sets when spawning a session:
- `AO_STATUS_PORT` — the port of the app's local HTTP server
- `AO_SESSION_ID` — the UUID identifying the session

If either is missing (e.g., running Claude Code outside of Agent Orchestrator), the script does nothing.

### 2. Hook Configuration

**`~/.claude/settings.json`** — two hook entries are added:

- **Notification hook** — fires on `idle_prompt`, `permission_prompt`, and `elicitation_dialog` events
- **Stop hook** — fires immediately when Claude Code finishes a task

Both pipe their JSON payload to the notify script via stdin.

### 3. Idle Threshold

**`~/.claude.json`** — `messageIdleNotifThresholdMs` is set to `500` (milliseconds). This controls how quickly Claude Code fires the idle notification after finishing output. The default is higher; lowering it makes status updates near-instant.

## The Event Flow

```
Claude Code fires hook
        │
        ▼
agent-orchestrator-notify.sh
        │ curl POST
        ▼
Status HTTP Server (127.0.0.1:{port})
  POST /status/{ao_session_id}
        │
        ▼
StatusTracker (state machine)
        │ state transition
        ▼
Tauri event → Frontend
        │
        ▼
SessionCard status dot updates
```

## State Machine

Each session has a `StatusTracker` with 6 possible states:

| State | Meaning | Dot Color |
|-------|---------|-----------|
| Starting | Session just created, waiting for first hook event | Gray |
| Working | Claude Code is actively processing | Green (pulsing) |
| Idle | Claude Code is waiting for user input | Blue |
| Needs Attention | Permission prompt or elicitation dialog | Orange |
| Finished | Task completed | Checkmark |
| Error | Process exited abnormally | Red |

### Transitions

| From | Event | To |
|------|-------|----|
| Starting | `idle_prompt` / stop hook / 5s timeout | Idle |
| Starting | `permission_prompt` / `elicitation_dialog` | Needs Attention |
| Starting / Idle / Finished / Needs Attention | User presses Enter | Working |
| Working | `idle_prompt` / stop hook | Finished |
| Working | User presses Escape | Finished |
| Working | `permission_prompt` / `elicitation_dialog` | Needs Attention |
| Needs Attention | `idle_prompt` | Finished |
| Any state | Process exits normally | Finished |
| Any state | Process exits abnormally | Error |

## HTTP Endpoint

**`POST /status/{ao_session_id}`**

### Notification Hook Payload

```json
{
  "session_id": "claude-session-id",
  "notification_type": "idle_prompt"
}
```

`notification_type` values: `idle_prompt`, `permission_prompt`, `elicitation_dialog`

### Stop Hook Payload

```json
{
  "session_id": "claude-session-id",
  "hook_event_name": "Stop",
  "cwd": "/path/to/working/directory"
}
```

### Response Codes

| Code | Meaning |
|------|---------|
| 200 | Status transition occurred |
| 204 | No transition (event didn't change state) |
| 400 | Bad request (invalid JSON, missing fields) |
| 404 | Unknown session ID |
| 405 | Not a POST request |
```

- [ ] **Step 2: Commit**

```bash
git add docs/how-status-works.md
git commit -m "docs: add how-status-works guide"
```

### Task 6: Create docs/keyboard-shortcuts.md

**Files:**
- Create: `docs/keyboard-shortcuts.md`

Source material: current README keyboard shortcuts section, `useGlobalKeybindings.ts`.

- [ ] **Step 1: Write docs/keyboard-shortcuts.md**

```markdown
# Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+T` | Open new session modal |
| `Cmd+W` | Close the active session |
| `Cmd+1` – `Cmd+9` | Switch to session by position in the sidebar |
```

- [ ] **Step 2: Commit**

```bash
git add docs/keyboard-shortcuts.md
git commit -m "docs: add keyboard shortcuts reference"
```

### Task 7: Create docs/troubleshooting.md

**Files:**
- Create: `docs/troubleshooting.md`

Source material: current README troubleshooting section.

- [ ] **Step 1: Write docs/troubleshooting.md**

```markdown
# Troubleshooting

## Status Stuck on "Starting"

The hook script may not be installed. Verify:

1. Check that `~/.claude/agent-orchestrator-notify.sh` exists and is executable:

   ```bash
   ls -la ~/.claude/agent-orchestrator-notify.sh
   ```

2. Check that `~/.claude/settings.json` contains `Notification` and `Stop` hook entries referencing the script:

   ```bash
   cat ~/.claude/settings.json | grep agent-orchestrator
   ```

3. Relaunch Agent Orchestrator — it re-runs hook installation on every startup.

## Sessions Won't Spawn / "claude" Not Found

The `.app` bundle captures your login shell environment on startup. If `claude` isn't on your shell's `PATH`, the app won't find it.

1. Open a new terminal window and run `which claude` — if it's not found, [install Claude Code](https://docs.anthropic.com/en/docs/claude-code) first.
2. If `claude` is available in your terminal but the app can't find it, your shell profile may not be loading correctly. Check that `$SHELL -li -c 'which claude'` outputs the path.
3. After fixing your PATH, **quit and relaunch** Agent Orchestrator (it captures the environment once on startup).

## Status Shows "Needs Attention" but There's No Prompt

Claude Code may have fired a `permission_prompt` or `elicitation_dialog` hook. The prompt might be scrolled out of view:

1. Click the session to switch to it
2. Scroll up in the terminal to find the permission prompt
3. Respond to the prompt, or type to dismiss the status

## App Won't Open (macOS Gatekeeper)

The app is not code-signed. macOS blocks unsigned apps downloaded from the internet.

```bash
xattr -dr com.apple.quarantine /Applications/Agent\ Orchestrator.app
```

This removes the quarantine flag. You only need to run this once after downloading. See [Installation](installation.md) for details.

## Sessions Not Creating

Check prerequisites:

1. **Claude Code CLI** — run `claude --version` in your terminal
2. **Node.js** — run `node --version` (v18+ required for development, not for running the app)
3. **Working directory** — the directory you selected in the new session modal must exist and be accessible
```

- [ ] **Step 2: Commit**

```bash
git add docs/troubleshooting.md
git commit -m "docs: add troubleshooting guide"
```

---

## Chunk 3: Rewrite README.md

### Task 8: Rewrite README.md

**Files:**
- Modify: `README.md`

This is a complete rewrite. The new README follows the 6-section structure from the spec.

- [ ] **Step 1: Write the new README.md**

Replace the entire contents of `README.md` with:

```markdown
# Agent Orchestrator

Run multiple [Claude Code](https://docs.anthropic.com/en/docs/claude-code) sessions in parallel. Monitor status. Switch instantly. One window.

![macOS](https://img.shields.io/badge/macOS-000000?style=flat&logo=apple&logoColor=white)
![Tauri 2](https://img.shields.io/badge/Tauri_2-FFC131?style=flat&logo=tauri&logoColor=black)
![v1.0.0](https://img.shields.io/github/v/release/stantonSB/Agent-Orchestrator?style=flat&label=version)

---

![Agent Orchestrator showing multiple sessions with different statuses](assets/hero.png)

---

## Features

<table>
<tr>
<td width="50%" valign="top">

### Parallel Sessions

Run 5+ Claude Code agents simultaneously, each in its own PTY with full terminal emulation. 256-color support, 10k-line scrollback, and instant switching without losing context.

![Parallel sessions sidebar](assets/feature-parallel-sessions.png)

</td>
<td width="50%" valign="top">

### Real-Time Status

Hook-driven detection shows Working, Idle, Needs Attention, Finished, and Error for each session. No output parsing — status updates come directly from Claude Code's hook system.

![Session status indicators](assets/feature-status.png)

</td>
</tr>
<tr>
<td width="50%" valign="top">

### Project Grouping

Sessions are automatically grouped by working directory in a collapsible sidebar. See all your active projects at a glance.

![Project groups in sidebar](assets/feature-project-groups.png)

</td>
<td width="50%" valign="top">

### Worktree Isolation

Each session runs `claude --worktree` by default, giving it an isolated git branch. Multiple agents can work on the same repo without conflicts.

</td>
</tr>
</table>

---

## Install

1. Download the latest `.dmg` from [**Releases**](https://github.com/stantonSB/Agent-Orchestrator/releases)
2. Drag to Applications, then run:
   ```bash
   xattr -dr com.apple.quarantine /Applications/Agent\ Orchestrator.app
   ```
3. Open Agent Orchestrator

> See [Installation Guide](docs/installation.md) for details on Gatekeeper, first launch, and prerequisites.

---

## Quick Start

Open the app → `Cmd+T` → type your prompt → go.

<video src="https://github.com/stantonSB/Agent-Orchestrator/raw/main/assets/quick-start.mp4" controls autoplay muted loop></video>

---

## Documentation

| | Document | Description |
|-|----------|-------------|
| 📦 | [Installation](docs/installation.md) | Download, Gatekeeper bypass, prerequisites, first launch |
| 🏗️ | [Architecture](docs/architecture.md) | System design, component deep-dives, data flow |
| 🛠️ | [Development](docs/development.md) | Setup, build, test, release, IDE configuration |
| 🔔 | [How Status Works](docs/how-status-works.md) | Hook protocol, state machine, event flow |
| ⌨️ | [Keyboard Shortcuts](docs/keyboard-shortcuts.md) | All shortcuts and navigation |
| 🔧 | [Troubleshooting](docs/troubleshooting.md) | Common issues and fixes |
```

- [ ] **Step 2: Verify all doc links resolve**

```bash
# Check that every linked file exists
for f in docs/installation.md docs/architecture.md docs/development.md docs/how-status-works.md docs/keyboard-shortcuts.md docs/troubleshooting.md; do
  if [ -f "$f" ]; then echo "OK: $f"; else echo "MISSING: $f"; fi
done
```

Expected: all 6 show OK.

- [ ] **Step 3: Verify asset references are consistent**

Check that the README references these exact asset paths:
- `assets/hero.png`
- `assets/feature-parallel-sessions.png`
- `assets/feature-status.png`
- `assets/feature-project-groups.png`
- `assets/quick-start.mp4`

These files won't exist yet (user needs to capture them), but the paths should be correct.

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: rewrite README as visual sales pitch with doc glossary"
```

---

## Chunk 4: Update CLAUDE.md and Final Cleanup

### Task 9: Update CLAUDE.md file layout section

**Files:**
- Modify: `CLAUDE.md`

The File Layout section in CLAUDE.md references `docs/future-phases/` and `docs/superpowers/`. Update it to reflect the new structure.

- [ ] **Step 1: Update the File Layout section in CLAUDE.md**

Replace the `docs/` section of the File Layout block:

```markdown
docs/
  installation.md       # Download, Gatekeeper, first launch
  architecture.md       # System design and component deep-dives
  development.md        # Setup, build, test, release
  how-status-works.md   # Hook protocol and state machine
  keyboard-shortcuts.md # All keyboard shortcuts
  troubleshooting.md    # Common issues and fixes
  future-phases/        # Backlog: tech-debt.md, nested-subagent-terminals.md
```

- [ ] **Step 2: Remove references to deleted files**

Check CLAUDE.md for any references to `docs/INDEX.md`, `DEVELOPMENT.md`, or `docs/superpowers/` and remove them.

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md file layout for new doc structure"
```

### Task 10: Add placeholder notice for missing assets

**Files:**
- Create: `assets/README.md`

Since the screenshots and video don't exist yet, add a brief note in the assets directory explaining what needs to be captured.

- [ ] **Step 1: Write assets/README.md**

```markdown
# Assets

Screenshots and videos for the README. Capture guidelines:

- All screenshots at 2x Retina resolution, ~1400px wide
- Use a consistent window size across all screenshots
- Video format: `.mp4` (GitHub renders inline; `.mov` won't embed)

## Required Assets

| File | Description |
|------|-------------|
| `hero.png` | Full app window, 5+ sessions, 2+ project groups. Must show all 5 active statuses: Working, Idle, Needs Attention, Finished, Error |
| `feature-parallel-sessions.png` | Sidebar showing 5+ sessions to convey scale |
| `feature-status.png` | Close-up of session cards with different status dots and activity pulse |
| `feature-project-groups.png` | Sidebar with 2-3 project groups, mix of collapsed/expanded |
| `quick-start.mp4` | 10-15 sec: open app → Cmd+T → type prompt → session starts working |
```

- [ ] **Step 2: Commit**

```bash
git add assets/README.md
git commit -m "docs: add asset capture guidelines"
```

### Task 11: Final verification

- [ ] **Step 1: Verify no broken internal references**

```bash
# Check all markdown links in README point to existing files
grep -oP '\[.*?\]\((docs/[^)]+)\)' README.md | grep -oP 'docs/[^)]+' | while read f; do
  if [ -f "$f" ]; then echo "OK: $f"; else echo "BROKEN: $f"; fi
done
```

Expected: all links show OK.

- [ ] **Step 2: Verify deleted files are gone**

```bash
for f in DEVELOPMENT.md docs/INDEX.md docs/future-phases.md assets/init-2-sessions.mov; do
  if [ -f "$f" ]; then echo "STILL EXISTS: $f"; else echo "DELETED: $f"; fi
done
# Verify docs/superpowers/ directory is gone
if [ -d "docs/superpowers" ]; then echo "STILL EXISTS: docs/superpowers/"; else echo "DELETED: docs/superpowers/"; fi
```

Expected: all show DELETED.

- [ ] **Step 3: Verify docs/future-phases/ is preserved**

```bash
ls docs/future-phases/
```

Expected: `tech-debt.md` and `nested-subagent-terminals.md` still present.

- [ ] **Step 4: Review git log for the full change set**

```bash
git log --oneline -12
```

Verify all commits are present and in order.
