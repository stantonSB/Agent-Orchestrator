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
