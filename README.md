# Agent Orchestrator

A macOS desktop app for running multiple [Claude Code](https://docs.anthropic.com/en/docs/claude-code) terminal sessions in parallel. Monitor session status at a glance, switch between agents without losing scrollback, and manage everything from a single window.

Built with Tauri 2, React 19, and TypeScript.

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

<video src="https://github.com/stantonSB/Agent-Orchestrator/raw/main/assets/init-2-sessions.mov" controls autoplay muted loop></video>

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

## Glossary

| Term | Definition |
|------|------------|
| **PTY** | Pseudo-terminal — an OS-level construct that emulates a hardware terminal, used to run each Claude Code session. |
| **Hook** | A Claude Code extension point that fires a shell script on specific events (e.g., idle, permission prompt, task stop). |
| **Session** | A single Claude Code agent running in its own PTY, tracked by a UUID (`AO_SESSION_ID`). |
| **Status** | One of six states a session can be in: Starting, Working, Idle, Needs Attention, Finished, or Error. |
| **Worktree** | A git feature that checks out a branch into a separate directory, giving each session an isolated copy of the repo. |
| **IPC** | Inter-Process Communication — the Tauri mechanism the React frontend uses to call Rust backend functions. |
| **Env capture** | On startup, the app runs `$SHELL -li -c env` to capture the user's full login-shell environment so `.app` bundles have access to tools like `claude` and `node`. |
| **Gatekeeper** | macOS security feature that blocks unsigned apps downloaded from the internet. |

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for setup, building, testing, releasing, and IDE configuration.
