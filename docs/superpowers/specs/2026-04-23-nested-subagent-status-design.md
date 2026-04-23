# Nested Subagent Status Tracking — Design Spec

## Overview

Add visibility into subagent activity within Agent Orchestrator. When a Claude Code session dispatches parallel subagents, their status appears as nested entries in the sidebar beneath the parent session card. No separate terminal output — status tracking only.

## Goals

- Show per-subagent status (working, idle, needs_attention, finished) nested under the parent session in the sidebar
- Detect subagents passively through hook events — no manual creation
- Bubble up `needs_attention` from any subagent to the parent's displayed status
- Clean up finished subagent entries automatically after a delay

## Non-Goals

- Individual terminal output per subagent (they share the parent's PTY)
- Hierarchical nesting beyond one level (grandchild subagents appear as flat entries, not nested)
- Subagent creation from the UI

## Approach: Backend-Centric

All subagent detection and correlation logic lives in the Rust backend. The frontend receives pre-correlated subagent lists via Tauri events and renders them.

## Detection Mechanism

Subagents are detected by observing the `session_id` field in hook JSON payloads. Each Claude Code process (parent and subagents) has a unique `session_id`, but all inherit the same `AO_SESSION_ID` from the parent PTY environment.

- The **first** `session_id` seen for a given `AO_SESSION_ID` is recorded as the parent
- **Subsequent** new `session_id` values are registered as subagents
- The `SubagentStop` hook (newly installed) provides confirmation when a subagent finishes

### Edge Case: Subagent Hook Arrives Before Parent

The parent identity is established by the first `session_id` value seen in any hook payload for a given `AO_SESSION_ID`. Since the parent Claude Code process starts before it can dispatch subagents, its hooks will almost always arrive first. However, if a subagent's hook arrives before the parent's:

- The `SubagentMap` maintains a `pending_events: Vec<(String, HookEvent)>` buffer (capped at 32 entries) for events that arrive before the parent identity is set
- Once the parent's first hook arrives, `pending_events` are replayed in order
- If the buffer reaches capacity, oldest events are dropped (this would indicate a pathological case)
- There is no timeout — the buffer is cleaned up when the parent session is closed

## Data Model

### Rust — `SubagentInfo`

```rust
struct SubagentInfo {
    claude_session_id: String,    // Claude Code's own session_id
    index: u16,                    // 1-based, for "Agent N" fallback naming
    status: SessionStatus,         // reuses existing enum
    name: Option<String>,          // descriptive name from hook data, if available
    finished_at: Option<Instant>,  // when it entered Finished, for cleanup timing
}
```

### Rust — `SubagentMap`

A `HashMap<String, SubagentInfo>` keyed by Claude Code `session_id`, plus a field for the parent's `claude_session_id`. Lives as a field on `StatusTracker` since the two are always accessed together.

### TypeScript — `SubagentStatus`

```typescript
interface SubagentStatus {
    id: string;         // claude_session_id
    index: number;
    status: SessionStatus;
    name: string | null;
}
```

The Zustand store adds `subagents: Map<string, SubagentStatus[]>` keyed by parent `AO_SESSION_ID`. The `SessionInfo` type is unchanged — subagents are not sessions.

No changes to `create_session` — subagents are detected passively.

## Hook Installation

### New Hook: `SubagentStop`

`hook_installer.rs` adds a `SubagentStop` entry alongside existing `Notification` and `Stop` hooks. Same shell script (`agent-orchestrator-notify.sh`), same matcher pattern. No script changes needed — the existing script forwards whatever JSON it receives via stdin.

The `is_already_installed` check is extended to verify the `SubagentStop` hook is present, ensuring upgrading users get it added.

## Status Server Protocol

The `POST /status/{ao_session_id}` endpoint gains subagent-aware routing. The server uses the `session_id` field from the JSON body to distinguish parent from subagent events:

1. Extract `session_id` from the JSON body (required — return 400 if missing)
2. Look up the `ao_session_id` in the trackers
3. If no parent `session_id` is recorded yet, record this `session_id` as the parent and process normally
4. If `session_id` matches the known parent, process as a parent event (existing logic)
5. If `session_id` does not match the parent, it's a subagent — route to the `SubagentMap`

For subagent events, the routing depends on event type:
- `notification_type` (Notification hook) → update subagent status based on notification type
- `hook_event_name: "Stop"` → mark subagent as finished
- `hook_event_name: "SubagentStop"` → this fires on the **parent** process, confirming a child finished. The payload's `session_id` is the parent's, so this is matched by rule 4. Any additional metadata (subagent description) is extracted if present and applied to the matching subagent entry.

This means parent `Stop` vs subagent `Stop` is disambiguated entirely by `session_id` matching, not by `hook_event_name`.

## Event Emission

When a subagent is first detected or its status changes, the backend emits:

```
session-subagents-{ao_session_id}
```

Payload is a JSON array of subagent entries:

```json
[
  {
    "id": "claude-session-id-abc",
    "index": 1,
    "status": "working",
    "name": null
  },
  {
    "id": "claude-session-id-def",
    "index": 2,
    "status": "needs_attention",
    "name": "Exploring codebase"
  }
]
```

Note: `finished_at` from the Rust `SubagentInfo` is not serialized to the frontend — it's internal to the backend for future use. The frontend subscribes to this event alongside existing `session-status-*` events.

## Status Bubbling

The `StatusTracker` gains a method that considers the subagent map when determining the parent's effective status:

- If the parent is `working` or `idle` and any subagent is `needs_attention`, the parent's effective status becomes `needs_attention`
- The parent's original status is preserved internally and restores when all subagents resolve
- Bubbled status is emitted via the existing `session-status-*` event

## Frontend — Sidebar Rendering

### New `SubagentList` Component

Lives in `components/SessionCard/`. Renders as an indented list below the parent `SessionCard` when the parent has subagents.

Each entry shows:
- A small status dot (same color mapping as sessions)
- The subagent name (or "Agent N" fallback)
- Dimmed styling when finished

Conditionally rendered — if no subagents exist for a session, nothing renders.

### `ProjectGroup` Changes

Currently renders a flat list of `SessionCard` components. Now renders each card followed by its `SubagentList` (if any). The subagent list is visually nested with a left border and indentation.

### `SessionCard` Changes

None. The parent card's status dot reflects whatever status the store holds (which may be the bubbled-up status from the backend).

## Subagent Cleanup

Finished subagent entries are removed from the sidebar 30 seconds after **both** conditions are met:
1. The subagent has finished
2. The parent session is the active (focused) session

If the user switches away from the parent before the timer fires, it cancels. The timer restarts when the user returns to the parent.

When a parent session is closed or dismissed, all its subagent entries are cleaned up immediately.

Cleanup logic lives in the Zustand store's event listener setup, not in React components.

## Subagent Naming

The primary naming strategy is the numbered fallback: "Agent 1", "Agent 2", etc. (1-based index assigned at detection time).

If Claude Code hook payloads include a descriptive field (such as a `tool_name`, `description`, or similar metadata), the name is updated when first seen. However, as of the current Claude Code hook protocol, there is no guaranteed field containing a subagent description. The `SubagentStop` hook fired on the parent may include metadata about the finished subagent — if it does, the name is backfilled at that point.

This is a best-effort enhancement. The numbered fallback is always available and is the documented primary behavior. If future Claude Code versions add richer hook metadata, naming can be improved without architectural changes.

## Nesting Depth

One level only. Grandchild subagents (spawned by a subagent) are silently ignored. Their hooks will arrive with the same `AO_SESSION_ID` and a new `session_id`. The server will register them as additional subagent entries in the flat list — they appear alongside their parent subagent, not nested beneath it. This is acceptable because the user sees them as "more agents working" without needing to understand the hierarchy.

## Event Flow Lifecycle

1. **Detection** — Subagent's first hook fires → status server sees new `session_id` → registers subagent → emits `session-subagents-*` event
2. **Status updates** — Subsequent hooks update subagent status → emit updated event → if `needs_attention`, bubble to parent via `session-status-*`
3. **Completion** — Subagent's `Stop` hook → status becomes `finished`. Parent's `SubagentStop` hook fires as confirmation.
4. **Cleanup** — 30-second timer after parent is focused + subagent is finished → remove from store → sidebar re-renders
5. **Parent closes** — All subagent entries cleaned up immediately

## Testing

### Rust Unit Tests

- **`status_server.rs`** — Subagent detection (first session_id = parent, second = subagent). `SubagentStop` handling. Buffering when parent hasn't been seen yet.
- **`status_parser.rs`** — Status bubbling (parent working + subagent needs_attention → parent effective status). Restoration when subagent resolves.
- **`hook_installer.rs`** — `SubagentStop` hook installation, idempotency, upgrade path.

### Frontend Unit Tests (Vitest)

- **`sessionStore`** — Subagent map updates on events. Cleanup timer logic: starts when subagent finishes + parent is active, cancels on focus switch, restarts on return.
- **`SubagentList`** — Renders correct count, status dots, names/fallbacks, dimmed styling for finished.
- **`ProjectGroup`** — Renders `SubagentList` beneath parent card when subagents exist, omits when empty.

### Manual Integration Test

Launch a session, give it a task that spawns subagents, verify dots appear nested in sidebar, verify status bubbling, verify cleanup after focus + 30 seconds.
