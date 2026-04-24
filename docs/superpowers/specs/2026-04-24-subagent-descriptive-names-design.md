# Subagent Descriptive Names

**Date:** 2026-04-24
**Status:** Approved

## Problem

Subagents in the session panel display their `agent_type` (e.g., "general-purpose") instead of a descriptive name. Claude Code's terminal UI shows descriptive names like "Review plan chunk 1" from the Agent tool's `description` parameter, but this field is not included in the `SubagentStart` hook payload.

The hook payload does include the `prompt` field — the full task prompt given to the subagent — which typically starts with a meaningful description of what the subagent will do.

## Solution

Extract a display name from the `prompt` field of the `SubagentStart` hook payload in the Rust backend. Use the first sentence (up to 40 characters) as the subagent's display name in the session panel.

## Design

### Name derivation logic

1. Extract the `prompt` field from the SubagentStart hook JSON.
2. Take the first sentence: text up to (but excluding) the first `.`, `\n`, or end of string.
3. Trim leading/trailing whitespace.
4. If longer than 40 characters, truncate at a Rust `char` boundary and append `...`.
5. If no prompt is present or the result is empty, fall back to `agent_type` (current behavior).

**Accepted limitation:** Some prompts may start with boilerplate framing (e.g., "You are a helpful assistant.") rather than a task description. The first-sentence heuristic will extract this boilerplate. This is acceptable for v1 — most Agent tool prompts lead with the task description. No special sanitization is needed beyond what React already provides (HTML escaping).

### Backend changes

**`status_server.rs`:**
- Extract `prompt` from the hook JSON alongside `agent_type`.
- Derive the display name using the logic above.
- Pass the display name to `SubagentMap::process_start()`.

**`subagent_tracker.rs`:**
- Add `display_name: Option<String>` field to `SubagentInfo`.
- Change `process_start()` signature from `(&mut self, agent_type: &str)` to `(&mut self, agent_type: &str, display_name: Option<String>)`.
- In `SubagentStatusPayload::from()`, populate `name` from `display_name` when present, falling back to `agent_type`.

### What doesn't change

- **Stop matching:** `SubagentStop` continues to match by `agent_type`, not display name.
- **Frontend:** `SubagentList.tsx` already renders `agent.name ?? "Agent " + agent.index`. No changes needed.
- **Hook installation:** No new hooks or payload modifications required.
- **No terminal output parsing:** All data comes from the structured hook JSON.

### Examples

| Prompt | Trigger | Display name |
|--------|---------|-------------|
| `"Review plan chunk 1 of the implementation that covers auth and routing"` | 40-char truncation | `"Review plan chunk 1 of the implementat..."` |
| `"Find all config files"` | End of string | `"Find all config files"` |
| `"Check if auth module handles edge cases. Be thorough."` | Period split (excluded) | `"Check if auth module handles edge cases"` |
| `"Fix the bug\nAlso check tests"` | Newline split | `"Fix the bug"` |
| `""` or missing | Fallback | agent_type (e.g., `"general-purpose"`) |

## Files to modify

1. `src-tauri/src/status_server.rs` — Extract prompt, derive name, pass to tracker
2. `src-tauri/src/subagent_tracker.rs` — Store display_name, use it in payload serialization
3. `src-tauri/src/status_server.rs` (tests) — Update existing subagent tests, add name derivation tests
4. `src-tauri/src/subagent_tracker.rs` (tests) — Update process_start call sites
