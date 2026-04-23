# Agent Orchestrator

A macOS desktop app for running multiple [Claude Code](https://docs.anthropic.com/en/docs/claude-code) terminal sessions in parallel. Monitor session status at a glance, switch between agents without losing scrollback, and manage everything from a single window.

Built with Tauri 2, React 19, and TypeScript. Inspired by [Scape](https://www.scape.work).

## Features

- **Parallel sessions** — Run 5+ Claude Code agents simultaneously, each in its own PTY with full terminal emulation (xterm.js, 256-color, 10k-line scrollback)
- **Real-time status** — Hook-driven detection shows Working / Idle / Needs Attention / Finished for each session (no output parsing heuristics)
- **Project grouping** — Sessions are grouped by working directory in a collapsible sidebar
- **Session management** — Create, rename (double-click or right-click), close, and dismiss sessions
- **Pull latest from main** — Optionally run `git checkout main && git pull` before spawning a new session
- **Keyboard shortcuts** — `Cmd+T` new session, `Cmd+W` close active, `Cmd+1-9` switch sessions
- **Worktree isolation** — Each session runs `claude --worktree` by default, giving it an isolated git branch
- **Skip permissions** — Optionally pass `--dangerously-skip-permissions` (default on) so agents run unattended

## How status detection works

On first launch, the app installs a lightweight hook into your Claude Code configuration:

- **Creates** `~/.claude/agent-orchestrator-notify.sh` — a small bash script that forwards hook events via `curl`
- **Adds entries** to `~/.claude/settings.json` — registers `Notification` and `Stop` hooks pointing to the script
- **Sets** `messageIdleNotifThresholdMs: 500` in `~/.claude.json` — lowers the idle notification delay

The app runs a local HTTP server on a random port. When Claude Code fires a hook (idle, permission prompt, stop), the script POSTs the event to the app, which updates the session status instantly. This is idempotent — re-launching the app won't duplicate entries.

## Installation (macOS)

Download the latest `.dmg` from the [Releases](https://github.com/stantonSB/Agent-Orchestrator/releases) page.

Since the app is not yet code-signed with an Apple Developer certificate, macOS Gatekeeper will block it on first launch. After installing, run this command once to allow it:

```bash
xattr -dr com.apple.quarantine /Applications/Agent\ Orchestrator.app
```

Then open the app normally. This only needs to be done once after downloading.

> **Why?** When macOS downloads a file via a browser, it attaches a quarantine attribute. Unsigned apps with this attribute are blocked by Gatekeeper. The command above removes the quarantine flag.

## Architecture

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

The PTY manager runs on a dedicated OS thread (required by `portable-pty` — handles aren't `Send`/`Sync`). All communication happens via mpsc channels. The app captures the user's full login-shell environment on startup so that sessions spawned from the `.app` bundle have access to `claude`, `node`, custom certs, etc.

## Prerequisites (Development)

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

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+T` | Open new session modal |
| `Cmd+W` | Close active session |
| `Cmd+1` – `Cmd+9` | Switch to session by position |

## Troubleshooting

### Status stuck on "Starting"

The hook script may not be installed. Check that `~/.claude/agent-orchestrator-notify.sh` exists and is executable, and that `~/.claude/settings.json` contains `Notification` and `Stop` hook entries referencing it. Relaunch the app to trigger auto-installation.

### Sessions won't spawn / "claude" not found

The `.app` bundle captures your login shell environment on startup. If `claude` isn't on your shell's `PATH`, the app won't find it. Ensure `claude` is available in a new terminal window, then relaunch the app.

### Status shows "Needs Attention" but there's no prompt

Claude Code may have fired a `permission_prompt` or `elicitation_dialog` hook. Scroll up in the terminal to find the prompt, or type a response to dismiss the status.

## Tech Stack

- **Frontend:** React 19, TypeScript, Vite, xterm.js, Zustand
- **Backend:** Rust, Tauri 2, portable-pty, tiny_http
- **Packaging:** Tauri bundler (DMG + .app)

## IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
