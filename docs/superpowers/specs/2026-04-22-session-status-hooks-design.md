# Session Status Detection via Claude Code Hooks

**Date:** 2026-04-22
**Supersedes:** 2026-04-21-session-status-detection-design.md

## Problem

The current session status system relies on timeout-based heuristics to detect Claude Code's state:

- **Starting → Idle**: 3-second timeout after output settles — arbitrary, often too long
- **Working → Finished**: 2-second quiet period after detecting `❯` prompt, or 60-second fallback timeout — fires prematurely while Claude Code is still working
- **Working → NeedsAttention**: 2-second quiet period after detecting question patterns — delayed

These heuristics are fragile. Spinner character detection, ANSI stripping, prompt pattern matching, and conservative timeouts all add complexity while still producing incorrect results. The fundamental issue: inferring state from terminal output is guesswork.

## Solution

Replace terminal output parsing with Claude Code's built-in **Notification hook system**. Claude Code knows its own state and exposes it through hooks that fire for `idle_prompt`, `permission_prompt`, and `elicitation_dialog` events. We configure these hooks to signal the Agent Orchestrator directly, giving us authoritative, event-driven status detection with zero timeouts.

## Architecture

### Components

1. **Status HTTP Server** — Lightweight HTTP server in the Rust backend, listens on `127.0.0.1` for hook notifications
2. **Hook Script** — Shell script at `~/.claude/agent-orchestrator-notify.sh` that forwards Claude Code notifications to the HTTP server
3. **Hook Configuration** — Entries in `~/.claude/settings.json` that tell Claude Code to call our script on Notification events
4. **Revised StatusTracker** — Simplified Rust state machine driven by hook events and user input, no output parsing

### Data Flow

```
Claude Code finishes task / needs permission / etc.
  → Claude Code fires Notification hook (idle_prompt / permission_prompt / elicitation_dialog)
  → Hook script reads JSON from stdin
  → Script POSTs to http://127.0.0.1:{AO_STATUS_PORT}/status/{AO_SESSION_ID}
  → Rust HTTP server receives request
  → Updates StatusTracker state
  → Emits Tauri event: session-status-{id}
  → Frontend updates SessionCard UI
```

### What Changes

**Removed:**
- 500-byte output buffer and all buffer management
- `feed_output()` / `feed_output_with_time()` methods
- `tick()` / `tick_with_time()` periodic polling
- `check_needs_attention()` question pattern matching
- `check_idle_prompt()` prompt detection
- `strip_ansi_escapes()` function
- Spinner character detection (`SPINNER_CHARS`, `last_spinner_at`)
- All timeout constants (3s, 2s, 1.5s, 60s)
- The 1-second polling loop in `pty_manager.rs`
- `last_output_at`, `has_received_output` fields

**Added:**
- Status HTTP server (new module)
- `notify_hook_event()` method on StatusTracker
- Hook script and installation logic
- `AO_STATUS_PORT` and `AO_SESSION_ID` environment variables on spawned sessions

**Unchanged:**
- `notify_user_input()` — Enter key detection for transitioning to Working
- `notify_exit()` — Process exit handling
- Frontend event listeners and UI rendering
- SessionStatus type and SessionCard component

## Status HTTP Server

### Startup

When the PTY manager initializes, start an HTTP server on `127.0.0.1:0` (OS-assigned port). Store the assigned port for use when spawning sessions.

### Endpoint

`POST /status/{ao_session_id}`

Request body (JSON from Claude Code hook stdin):
```json
{
  "session_id": "claude-code-session-id",
  "notification_type": "idle_prompt",
  "message": "Claude is ready for input",
  "title": "Idle"
}
```

### Processing

Extract `notification_type` from the JSON body and `ao_session_id` from the URL path. Map to status transitions:

| `notification_type` | Current Status | New Status |
|---|---|---|
| `idle_prompt` | `starting` | `idle` |
| `idle_prompt` | `working` | `finished` |
| `permission_prompt` | `working` | `needs_attention` |
| `elicitation_dialog` | `working` | `needs_attention` |

Ignore notifications that don't match a valid transition (e.g., `idle_prompt` when already `idle`).

On state change, emit `session-status-{ao_session_id}` Tauri event with `{ "status": "<new_status>" }`.

### Implementation

Use the `tiny_http` crate. The server runs on its own thread. Minimal dependencies, no async runtime needed.

## Hook Script

Located at `~/.claude/agent-orchestrator-notify.sh`:

```bash
#!/bin/bash
# Forward Claude Code notifications to Agent Orchestrator.
# No-ops silently when Agent Orchestrator is not running.
if [ -n "$AO_STATUS_PORT" ] && [ -n "$AO_SESSION_ID" ]; then
    curl -s -X POST "http://127.0.0.1:${AO_STATUS_PORT}/status/${AO_SESSION_ID}" \
        -H "Content-Type: application/json" -d @- 2>/dev/null || true
fi
```

The script:
- Checks for `AO_STATUS_PORT` and `AO_SESSION_ID` environment variables (set per-session by Agent Orchestrator)
- Forwards the hook's stdin JSON to the HTTP server
- Silently no-ops if env vars are missing or the server is unreachable
- Uses `|| true` to prevent hook failure from affecting Claude Code

## Hook Configuration

### Claude Code Settings

Added to `~/.claude/settings.json` (merged with existing settings):

```json
{
  "hooks": {
    "Notification": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"${HOME}/.claude/agent-orchestrator-notify.sh\""
          }
        ]
      }
    ]
  }
}
```

No matcher specified — captures all notification types. The HTTP server determines which ones are relevant.

### Idle Threshold

Added to `~/.claude.json`:

```json
{
  "messageIdleNotifThresholdMs": 500
}
```

This configures Claude Code to fire `idle_prompt` 500ms after becoming idle, rather than the default 60 seconds. The 500ms delay is Claude Code's own idle detection — it fires only when Claude Code has genuinely finished processing.

### Per-Session Environment Variables

When spawning a Claude Code session in `pty_manager.rs`, set:

- `AO_STATUS_PORT` — The HTTP server's port number
- `AO_SESSION_ID` — The Agent Orchestrator session ID

These are set alongside existing env vars (`TERM`, `COLORTERM`) in the `CommandBuilder`.

## Hook Installation

### Startup Check

On app launch:

1. Check if `~/.claude/agent-orchestrator-notify.sh` exists and is executable
2. Read `~/.claude/settings.json` and check for our Notification hook entry (identify by script path containing `agent-orchestrator-notify`)
3. Check if `~/.claude.json` has `messageIdleNotifThresholdMs` set
4. If all present → no action
5. If any missing → run installation

### Installation Steps

1. Write `~/.claude/agent-orchestrator-notify.sh` and set executable permission (chmod +x)
2. Read existing `~/.claude/settings.json` (or create if absent)
3. Parse as JSON, merge our hook entry into `hooks.Notification` array (preserve existing hooks)
4. Write back the merged settings
5. Read existing `~/.claude.json` (or create if absent)
6. Set `messageIdleNotifThresholdMs: 500` if not already present
7. Write back

### Failure Handling

If installation fails at any step:

- Emit Tauri event `hook-setup-failed` with `{ "error": "<reason>" }`
- Frontend shows a non-blocking banner: "Session status hooks could not be installed: {reason}. [Retry] [Dismiss]"
- Retry button triggers installation again
- Sessions still function without hooks — status stays at `starting` until process exits (degraded but usable)

### Idempotency

Installation is idempotent. Running it multiple times produces the same result. The check-before-install prevents unnecessary writes.

## Revised State Machine

### States

Same 6 states: `starting`, `working`, `idle`, `needs_attention`, `finished`, `error`

### Transitions

All event-driven, zero timeouts:

| Event Source | Event | From | To |
|---|---|---|---|
| PTY spawn | Process created | — | `starting` |
| HTTP server | `idle_prompt` (first) | `starting` | `idle` |
| HTTP server | `idle_prompt` | `working` | `finished` |
| HTTP server | `permission_prompt` | `working` | `needs_attention` |
| HTTP server | `elicitation_dialog` | `working` | `needs_attention` |
| PTY input | User presses Enter | `idle` / `finished` / `needs_attention` | `working` |
| PTY reader | Process exit (code 0) | any | `finished` |
| PTY reader | Process exit (code ≠ 0) | any | `error` |

### StatusTracker Implementation

The StatusTracker reduces to ~50 lines:

```rust
pub struct StatusTracker {
    status: SessionStatus,
}

impl StatusTracker {
    pub fn new() -> Self {
        Self { status: SessionStatus::Starting }
    }

    pub fn status(&self) -> &SessionStatus { &self.status }

    pub fn notify_hook_event(&mut self, notification_type: &str) -> Option<SessionStatus> {
        let new_status = match notification_type {
            "idle_prompt" => match self.status {
                SessionStatus::Starting => Some(SessionStatus::Idle),
                SessionStatus::Working => Some(SessionStatus::Finished),
                _ => None,
            },
            "permission_prompt" | "elicitation_dialog" => match self.status {
                SessionStatus::Working => Some(SessionStatus::NeedsAttention),
                _ => None,
            },
            _ => None,
        };

        if let Some(ref s) = new_status {
            self.status = s.clone();
        }
        new_status
    }

    pub fn notify_user_input(&mut self, data: &[u8]) -> Option<SessionStatus> {
        if !data.contains(&b'\r') && !data.contains(&b'\n') {
            return None;
        }
        match self.status {
            SessionStatus::Idle | SessionStatus::Finished | SessionStatus::NeedsAttention => {
                self.status = SessionStatus::Working;
                Some(SessionStatus::Working)
            }
            _ => None,
        }
    }

    pub fn notify_exit(&mut self, exit_code: i32) -> SessionStatus {
        self.status = if exit_code == 0 {
            SessionStatus::Finished
        } else {
            SessionStatus::Error
        };
        self.status.clone()
    }
}
```

## Testing

### Unit Tests (StatusTracker)

- Each hook event transition (idle_prompt from starting, from working)
- Permission prompt and elicitation dialog transitions
- User input transitions (Enter key from idle, finished, needs_attention)
- Process exit handling (code 0, code non-zero)
- Invalid notification types ignored
- Duplicate events (two idle_prompts in a row → second ignored)
- Events in unexpected states (idle_prompt when already idle → ignored)

### Integration Tests

- HTTP server starts, accepts requests, returns 200
- Malformed JSON handled gracefully (400 response, no crash)
- Unknown session IDs handled (404 response)
- Hook script forwards correctly formatted JSON
- End-to-end: spawn mock process → send hook notification → verify Tauri event emitted

### Hook Installation Tests

- Fresh install (no existing settings files)
- Merge with existing settings (preserving user's other hooks)
- Merge with existing Notification hooks (ours appended, theirs preserved)
- Idempotent re-runs (no changes on second run)
- Handles missing `~/.claude/` directory
- Handles malformed existing settings.json (backup and recreate)
- Handles read-only filesystem (error reported, not crash)

## Migration

The previous status detection spec (2026-04-21-session-status-detection-design.md) is superseded. The implementation involves:

1. Adding the HTTP server module and hook installation logic
2. Rewriting `status_parser.rs` to the simplified version
3. Updating `pty_manager.rs` to remove the polling loop and add env vars to spawned sessions
4. Rewriting `status_parser_tests.rs` for the new state machine
5. No frontend changes needed — same events, same status types

## Files Affected

| File | Change |
|---|---|
| `src-tauri/src/status_parser.rs` | Rewrite: remove output parsing, add hook event handling |
| `src-tauri/src/status_parser_tests.rs` | Rewrite: new test suite for hook-driven state machine |
| `src-tauri/src/pty_manager.rs` | Remove polling loop, add env vars to CommandBuilder, integrate HTTP server |
| `src-tauri/src/lib.rs` | Start HTTP server, pass status callback |
| `src-tauri/src/status_server.rs` | New: HTTP server module |
| `src-tauri/src/hook_installer.rs` | New: hook installation and verification logic |
| `src-tauri/Cargo.toml` | Add `tiny_http` and `serde_json` (if not already present) dependencies |
