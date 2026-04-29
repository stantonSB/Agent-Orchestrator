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
