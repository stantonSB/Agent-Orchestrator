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

The app is signed and notarized, so Gatekeeper should allow it to open without issues. If you still encounter a warning, try re-downloading the latest `.dmg` from [Releases](https://github.com/stantonSB/Agent-Orchestrator/releases) — older pre-signed builds may require the quarantine workaround. See [Installation](installation.md) for details.

## Sessions Not Creating

Check prerequisites:

1. **Claude Code CLI** — run `claude --version` in your terminal
2. **Node.js** — run `node --version` (v18+ required for development, not for running the app)
3. **Working directory** — the directory you selected in the new session modal must exist and be accessible

## Auto Mode Fails Immediately

If a session in Auto mode exits with an error within seconds of starting, your Claude Code version may be too old.

1. Run `claude update` in your terminal to update to the latest version (v2.1.83+)
2. Try creating the session again

## File Paths Not Clickable

File paths in terminal output should be underlined on hover and open in VS Code with `Cmd+click`.

1. Ensure VS Code is installed and the `code` CLI is available
2. Only file paths with extensions are detected (e.g., `src/main.ts:42:5`)
3. Relative paths are resolved against the session's working directory

## Persisted Sessions Not Appearing

On app restart, finished sessions should appear in the sidebar with an "Exited" status.

1. Check that the persistence directory exists: `~/Library/Application Support/com.xbridge.agent-orchestrator/sessions/`
2. Sessions are only persisted after they exit — sessions that were forcefully killed during an app crash may not be saved
3. Dismiss a persisted session to permanently delete it from disk

## Image Drag & Drop Not Working

Dragging images onto the terminal should type the file path into the session.

1. The active session must not be a persisted (read-only) session
2. Only image files are supported: PNG, JPG, JPEG, GIF, WebP, SVG, BMP, TIFF
3. Both Finder file drags and browser image drags are supported
