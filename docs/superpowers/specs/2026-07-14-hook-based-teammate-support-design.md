# Hook-Based Teammate Support

## Problem

Agent Orchestrator's fleet view shows regular Task-tool subagents (e.g. `general-purpose`, `Explore`, `code-reviewer`) under their parent Claude session, but it does **not** show *in-process teammates* — the newer named agents spawned via the Agent tool with a `name`, addressable via SendMessage (Claude Code's "FleetView" teammates). On disk these are marked `"taskKind":"in_process_teammate"` (e.g. `deck-impl`), whereas regular subagents have no `taskKind`.

Root cause: AO discovers subagents **exclusively** through hook events, and registers a subagent only from the `SubagentStart` hook (`status_server.rs` → `SubagentMap::process_start`). In-process teammates do not reliably drive that path — their lifecycle is expressed through the team hooks `TaskCreated` / `TaskCompleted` / `TeammateIdle` (there is no `TeammateStart`). AO subscribes to none of them, so `process_start` is never called, `subagent_changed` stays `false`, no event is emitted, and `SubagentList` renders nothing. The parent session still shows correctly (its `Stop`/`Notification` hooks work), which confirms the pipeline is healthy — AO simply never asked for the hooks a teammate fires.

## Solution

Extend AO's existing hook-driven subagent tracking to also recognise teammate lifecycle hooks, and make the tracker robust by keying records on the Claude Code `agent_id`. Teammates appear in the fleet list exactly like subagents, with a binary **Working / Finished** status. The change is **backend-only** — the frontend payload shape and rendering are unchanged.

Consistent with the project convention (`CLAUDE.md`: "Status detection is hook-driven only"), no terminal output or on-disk `subagents/*.meta.json` files are parsed.

## Design

### 1. Hook Registration (`hook_installer.rs`)

Add two events to `HOOK_EVENTS` (5 → 7):

- **`TaskCreated`** — teammate *appearance* trigger (fires when a teammate is given a task).
- **`TeammateIdle`** — teammate *finish* trigger (fires when a teammate is about to go idle).

`session_hook_settings()` already iterates `HOOK_EVENTS` when building the per-session `--settings` payload, so both events register automatically once added to the array. Update the array length and the doc comment.

`TaskCompleted` is intentionally **not** registered: a teammate completing one task does not mean the teammate is done (it may have more tasks).

### 2. Status-Server Parsing (`status_server.rs`)

- Extract `agent_id` from the JSON body alongside the existing `agent_type` and `prompt`:
  ```rust
  let agent_id = json.get("agent_id").and_then(|v| v.as_str()).map(|s| s.to_string());
  ```
- Map `hook_event_name` into two lifecycle buckets:
  - **start-like** = `SubagentStart` ∪ `TaskCreated` → `process_start(agent_id, agent_type, display_name)`
  - **stop-like** = `SubagentStop` ∪ `TeammateIdle` → `process_stop(agent_id, agent_type)`
- `is_subagent_event` becomes true for any of the four. `Notification`, `Stop`, and `PreToolUse` handling is unchanged.
- Emission, bubbling (`any_needs_attention`), and response-code logic are unchanged. Teammates never set `NeedsAttention`, so bubbling is unaffected.

### 3. Tracker Refactor (`subagent_tracker.rs`) — key by `agent_id`

`SubagentInfo` gains one **backend-internal** field:

```rust
pub agent_id: Option<String>,   // Claude Code agent_id, when the hook provides it
```

Matching precedence:

1. **`agent_id` exact match** (when present) — the robust path for both teammates and modern subagents.
2. **Fallback (no `agent_id`)** — today's behaviour: `agent_type` oldest-Working, then any oldest-Working (preserves compatibility with hook payloads that omit `agent_id`).

Method behaviour:

- `process_start(agent_id: Option<&str>, agent_type: &str, display_name: Option<String>) -> bool`
  - If `agent_id` matches an existing record → **upsert**: revive `Finished`→`Working` if needed, fill in a better `display_name`/`agent_type` if the record lacked one. Returns `true` only if something changed.
  - Otherwise → push a new `Working` record (as today). Returns `true`.
  - Consequence: repeated `TaskCreated` events for the same teammate collapse to **one** row.
- `process_stop(agent_id: Option<&str>, agent_type: &str) -> bool`
  - If `agent_id` matches a record → mark it `Finished`.
  - Else → existing FIFO fallback (oldest-Working-of-type, then any-Working).
- `created_at` is set on first insert; on a revive it is **not** reset (stable timer).

### 4. Frontend — no changes

The serialized payload (`SubagentStatusPayload` → `SubagentStatus { id, index, status, name, created_at }`) is unchanged; `agent_id` stays internal to the backend. `SubagentList.tsx` already renders `working` (green) and `finished` (grey) dots and falls back to `Agent {index}` when `name` is null. `sessionStore.updateSubagents` already handles list replacement and the 30 s finished-row cleanup. Nothing on the frontend needs to change.

### 5. Data Flow (unchanged pipeline, new inputs)

```
Claude Code teammate hook (TaskCreated / TeammateIdle)
  → ~/.claude/agent-orchestrator-notify.sh  (POST, forwards stdin verbatim)
  → status_server.rs  POST /status/{AO_SESSION_ID}
  → StatusTracker.subagent_map_mut().process_start/stop  (keyed by agent_id)
  → on_subagents → Tauri event "session-subagents-{id}"
  → sessionStore.updateSubagents → SessionPanel → ProjectGroup → SubagentList
```

### 6. Status Mapping

| Hook event | Bucket | Effect |
|---|---|---|
| `SubagentStart` | start-like | upsert → `Working` (regular subagents; teammates too if it fires) |
| `TaskCreated` | start-like | upsert → `Working` (teammate appearance) |
| `SubagentStop` | stop-like | match → `Finished` |
| `TeammateIdle` | stop-like | match → `Finished` |
| `TaskCompleted` | — | not registered / ignored |

### 7. Backend Changes Summary

| Component | Change |
|---|---|
| `hook_installer.rs` | Add `TaskCreated`, `TeammateIdle` to `HOOK_EVENTS` (5 → 7); update comment |
| `status_server.rs` | Extract `agent_id`; route `TaskCreated`→start-like, `TeammateIdle`→stop-like; pass `agent_id` into tracker |
| `subagent_tracker.rs` | Add `agent_id` to `SubagentInfo`; upsert by `agent_id` in `process_start`; match by `agent_id` in `process_stop`; FIFO fallback preserved |

### 8. Error Handling

All existing behaviour is preserved: unknown session → 404, malformed/empty body → 400, unrecognised `hook_event_name` → 400. Missing `agent_id` → FIFO fallback (backward compatible with older Claude Code payloads).

### 9. Testing (Rust unit tests — hook-driven, no output parsing)

`subagent_tracker.rs` tests:
- Same-`agent_id` `process_start` twice → one record (dedup).
- Distinct `agent_id`s → distinct records.
- `process_stop` by `agent_id` finishes the correct record.
- `TeammateIdle`-style stop then `TaskCreated`-style start with the same `agent_id` → record revives `Finished`→`Working`.
- `agent_id: None` preserves the existing FIFO behaviour (all current tests still pass).
- Name fill-in: a later event supplies `display_name`/`agent_type` when the first lacked it.

`status_server.rs` tests:
- POST `TaskCreated` with `agent_id`+`agent_type` → registers `Working`, emits `on_subagents`, 200.
- POST `TeammateIdle` with `agent_id` → marks `Finished`, emits, 200.
- POST `SubagentStart` still registers (regression).
- Two `TaskCreated` with the same `agent_id` → one entry in the emitted payload.

## Assumptions (verify at implementation)

This design was intentionally produced without a live hook capture; the teammate-hook payload fields below are **not documented by Claude Code** and must be confirmed before finalising. A capture harness exists (`capture-settings.json` + `log-hooks.sh`) that logs every candidate hook's raw payload while a teammate is spawned — the implementation plan opens with running it.

- **V1 — stable `agent_id`.** `SubagentStart` / `TaskCreated` / `TeammateIdle` for the same teammate carry a consistent `agent_id`. If they diverge, a teammate could appear as duplicate rows. (`SubagentStart`/`SubagentStop` `agent_id` is documented; the team hooks list it only as "when applicable".)
- **V2 — teammate name.** The name arrives via `agent_type` on the appearance event. If `TaskCreated` omits it, the row shows `agent_id`/`Agent N` until a named event fills it in via upsert.
- **V3 — `TaskCreated.agent_id` identifies the assignee** teammate, not the orchestrator that created the task.
- **V4 — `TeammateIdle`→`Finished` is a heuristic.** Idle is transient; a teammate may show `Finished` during an idle gap and revive on its next task (self-healing via upsert). Accepted under the binary Working/Finished model chosen for this feature.

If the capture contradicts V1–V3 (different field names / id semantics), adjust the extraction constants in `status_server.rs` and the matching key accordingly — the tracker's upsert-by-key structure does not change.

## Out of Scope

- A distinct "idle" teammate state in the UI (deferred; binary Working/Finished chosen for this feature).
- Registering/handling `TaskCompleted` for per-task progress display.
- Discovering teammates from on-disk `subagents/*.meta.json` (rejected: violates the hook-only convention and relies on an undocumented layout).
- Any frontend changes.
- Removing lingering teammate rows beyond the existing 30 s finished-row cleanup.
