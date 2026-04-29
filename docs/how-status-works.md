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
