# Hook-Based Teammate Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface in-process teammates (Claude Code "FleetView" teammates, `taskKind: in_process_teammate`) in Agent Orchestrator's fleet view, driven by hooks, alongside regular subagents.

**Architecture:** Backend-only change. Extend the hook-driven subagent tracker to (a) key records by the Claude Code `agent_id` with upsert semantics (FIFO fallback when absent), and (b) recognise the teammate lifecycle hooks `TaskCreated` (appearance) and `TeammateIdle` (finish) in addition to `SubagentStart`/`SubagentStop`. The serialized payload and the entire frontend are unchanged.

**Tech Stack:** Rust (Tauri 2 backend), `tiny_http` status server, `serde_json`. Tests are Rust unit tests run with `cargo test`.

## Global Constraints

- **Backend-only.** No changes to `src/` (TypeScript/React). The payload types `SubagentStatusPayload` (Rust) and `SubagentStatus` (TS) stay byte-identical.
- **Hook-driven only.** Never parse terminal output or on-disk `subagents/*.meta.json`. Status comes only from hook events (project convention in `CLAUDE.md`).
- **Binary status for teammates.** Only `Working` and `Finished`. No new UI/idle state.
- **`agent_id` is backend-internal.** It is stored on `SubagentInfo` but is NOT added to the serialized payload.
- **Run tests from `src-tauri/`:** `cd src-tauri && cargo test`.
- **Commit message trailer:** every commit message ends with
  `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.

---

### Task 1: Key the subagent tracker by `agent_id` (backward-compatible)

Refactor `SubagentMap` so `process_start`/`process_stop` take an optional `agent_id` and upsert/match on it, falling back to today's FIFO-by-type behaviour when it is absent. Update the one production call site and the existing direct-call test sites so everything compiles, then add new tests for the id-keyed behaviour. This task improves regular-subagent matching too; teammate hook *routing* comes in Task 2.

**Files:**
- Modify: `src-tauri/src/subagent_tracker.rs` (struct + `process_start` + `process_stop`, and inline `#[cfg(test)] mod tests`)
- Modify: `src-tauri/src/status_server.rs:145-197` (extract `agent_id`; pass it into the two existing calls; restructure dispatch into `is_start`/`is_stop`)
- Modify: `src-tauri/src/subagent_tracker_tests.rs` (existing call sites → add leading `None,`)
- Modify: `src-tauri/src/status_parser_tests.rs:599` (existing call site → add leading `None,`)

**Interfaces:**
- Produces:
  - `SubagentMap::process_start(agent_id: Option<&str>, agent_type: &str, display_name: Option<String>) -> bool`
  - `SubagentMap::process_stop(agent_id: Option<&str>, agent_type: &str) -> bool`
  - `SubagentInfo { ..., agent_id: Option<String>, ... }`
- Consumes: `crate::status_parser::SessionStatus` (existing).

- [ ] **Step 1: Add the `agent_id` field to `SubagentInfo`**

In `src-tauri/src/subagent_tracker.rs`, change the struct:

```rust
#[derive(Debug, Clone)]
pub struct SubagentInfo {
    pub id: String,
    pub index: u16,
    pub status: SessionStatus,
    /// Claude Code `agent_id`, when the hook payload provides one. Used as the
    /// primary key for upsert/finish matching; `None` falls back to FIFO-by-type.
    pub agent_id: Option<String>,
    pub agent_type: String,
    pub display_name: Option<String>,
    pub created_at: u64,
    pub finished_at: Option<Instant>,
}
```

Leave `SubagentStatusPayload` and its `From<&SubagentInfo>` impl unchanged (`agent_id` is not serialized).

- [ ] **Step 2: Rewrite `process_start` to upsert by `agent_id`**

Replace the whole `process_start` method with:

```rust
    /// Register or refresh a subagent/teammate.
    ///
    /// When `agent_id` is `Some` and matches an existing record, upsert it:
    /// revive `Finished` -> `Working`, and fill in `display_name`/`agent_type`
    /// if they were previously unknown. Otherwise push a new `Working` record.
    /// Returns `true` only if something changed (so duplicate events are no-ops).
    pub fn process_start(
        &mut self,
        agent_id: Option<&str>,
        agent_type: &str,
        display_name: Option<String>,
    ) -> bool {
        if let Some(aid) = agent_id {
            if let Some(existing) = self
                .agents
                .iter_mut()
                .find(|a| a.agent_id.as_deref() == Some(aid))
            {
                let mut changed = false;
                if existing.status != SessionStatus::Working {
                    existing.status = SessionStatus::Working;
                    existing.finished_at = None;
                    changed = true;
                }
                if existing.display_name.is_none() && display_name.is_some() {
                    existing.display_name = display_name;
                    changed = true;
                }
                if existing.agent_type == "unknown" && agent_type != "unknown" {
                    existing.agent_type = agent_type.to_string();
                    changed = true;
                }
                return changed;
            }
        }

        let index = self.next_index;
        self.next_index += 1;
        let id = format!("subagent-{}", index);
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.agents.push(SubagentInfo {
            id,
            index,
            status: SessionStatus::Working,
            agent_id: agent_id.map(|s| s.to_string()),
            agent_type: agent_type.to_string(),
            display_name,
            created_at,
            finished_at: None,
        });
        true
    }
```

- [ ] **Step 3: Rewrite `process_stop` to match by `agent_id`**

Replace the whole `process_stop` method with:

```rust
    /// Mark a subagent/teammate `Finished`.
    ///
    /// When `agent_id` is `Some`, only a record with that exact id is finished;
    /// if none matches, nothing changes (we never finish an unrelated agent on a
    /// mismatched id). When `agent_id` is `None`, fall back to the oldest Working
    /// record of the same type, then the oldest Working of any type.
    /// Returns `true` if state changed.
    pub fn process_stop(&mut self, agent_id: Option<&str>, agent_type: &str) -> bool {
        if let Some(aid) = agent_id {
            if let Some(agent) = self.agents.iter_mut().find(|a| {
                a.agent_id.as_deref() == Some(aid) && a.status == SessionStatus::Working
            }) {
                agent.status = SessionStatus::Finished;
                agent.finished_at = Some(Instant::now());
                return true;
            }
            return false;
        }

        if let Some(agent) = self
            .agents
            .iter_mut()
            .filter(|a| a.agent_type == agent_type && a.status == SessionStatus::Working)
            .min_by_key(|a| a.index)
        {
            agent.status = SessionStatus::Finished;
            agent.finished_at = Some(Instant::now());
            return true;
        }

        if let Some(agent) = self
            .agents
            .iter_mut()
            .filter(|a| a.status == SessionStatus::Working)
            .min_by_key(|a| a.index)
        {
            agent.status = SessionStatus::Finished;
            agent.finished_at = Some(Instant::now());
            return true;
        }

        false
    }
```

- [ ] **Step 4: Update the production call site in `status_server.rs`**

Change the `agent_type`/`prompt` extraction block (around line 145-147) to also read `agent_id`:

```rust
    // Extract subagent/teammate fields if present.
    let agent_type = json.get("agent_type").and_then(|v| v.as_str()).map(|s| s.to_string());
    let agent_id = json.get("agent_id").and_then(|v| v.as_str()).map(|s| s.to_string());
    let prompt = json.get("prompt").and_then(|v| v.as_str()).map(|s| s.to_string());
```

Then replace the dispatch block (currently lines ~181-197, the `let mut subagent_changed = false;` through the closing `}` of the `if is_subagent_event` block) with:

```rust
                let mut subagent_changed = false;
                let is_start = notification_type == "subagent_start";
                let is_stop = notification_type == "subagent_stop";
                let is_subagent_event = is_start || is_stop;

                // Handle subagent/teammate lifecycle events
                if is_subagent_event {
                    let type_name = agent_type.as_deref().unwrap_or("unknown");
                    let aid = agent_id.as_deref();
                    let submap = tracker.subagent_map_mut();
                    subagent_changed = if is_start {
                        let display_name = prompt.as_deref().and_then(derive_display_name);
                        submap.process_start(aid, type_name, display_name)
                    } else {
                        submap.process_stop(aid, type_name)
                    };
                }
```

- [ ] **Step 5: Update existing direct-call test sites to the new signature**

These files call `process_start`/`process_stop` directly and must pass `None` for `agent_id`. Run (macOS `sed`):

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/.claude/worktrees/hook-based-teammate-support
sed -i '' -e 's/\.process_start(/.process_start(None, /g' -e 's/\.process_stop(/.process_stop(None, /g' \
  src-tauri/src/subagent_tracker.rs \
  src-tauri/src/subagent_tracker_tests.rs \
  src-tauri/src/status_parser_tests.rs
```

This matches only method *calls* (leading `.`), never the `pub fn` definitions. Do NOT run it on `status_server.rs` (its calls were hand-edited in Step 4).

- [ ] **Step 6: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: builds with no errors. (Warnings about `agent_id` being unused on the read side are not expected, since it is now stored.)

- [ ] **Step 7: Add id-keyed tests to `subagent_tracker.rs`**

Append these tests inside the existing `#[cfg(test)] mod tests { ... }` block in `src-tauri/src/subagent_tracker.rs`:

```rust
    #[test]
    fn test_upsert_by_agent_id_dedupes() {
        let mut map = SubagentMap::new();
        assert!(map.process_start(Some("t1"), "deck-impl", None));
        // Second start for the same agent_id, already Working, no new info -> no-op.
        assert!(!map.process_start(Some("t1"), "deck-impl", None));
        assert_eq!(map.subagents().len(), 1);
    }

    #[test]
    fn test_distinct_agent_ids_are_separate_rows() {
        let mut map = SubagentMap::new();
        map.process_start(Some("t1"), "deck-impl", None);
        map.process_start(Some("t2"), "deck-scan", None);
        assert_eq!(map.subagents().len(), 2);
    }

    #[test]
    fn test_stop_by_agent_id_finishes_correct_record() {
        let mut map = SubagentMap::new();
        map.process_start(Some("t1"), "deck-impl", None);
        map.process_start(Some("t2"), "deck-scan", None);
        assert!(map.process_stop(Some("t1"), "deck-impl"));
        let a = map.subagents();
        let t1 = a.iter().find(|x| x.agent_id.as_deref() == Some("t1")).unwrap();
        let t2 = a.iter().find(|x| x.agent_id.as_deref() == Some("t2")).unwrap();
        assert_eq!(t1.status, SessionStatus::Finished);
        assert_eq!(t2.status, SessionStatus::Working);
    }

    #[test]
    fn test_stop_then_start_same_agent_id_revives() {
        let mut map = SubagentMap::new();
        map.process_start(Some("t1"), "deck-impl", None);
        map.process_stop(Some("t1"), "deck-impl");
        assert_eq!(map.subagents()[0].status, SessionStatus::Finished);
        // Teammate picks up new work -> revived, still one row.
        assert!(map.process_start(Some("t1"), "deck-impl", None));
        assert_eq!(map.subagents().len(), 1);
        assert_eq!(map.subagents()[0].status, SessionStatus::Working);
    }

    #[test]
    fn test_stop_unmatched_agent_id_is_noop() {
        let mut map = SubagentMap::new();
        map.process_start(Some("t1"), "deck-impl", None);
        assert!(!map.process_stop(Some("does-not-exist"), "deck-impl"));
        assert_eq!(map.subagents()[0].status, SessionStatus::Working);
    }

    #[test]
    fn test_upsert_fills_in_display_name_later() {
        let mut map = SubagentMap::new();
        map.process_start(Some("t1"), "deck-impl", None);
        assert!(map.process_start(Some("t1"), "deck-impl", Some("Redesign the deck".to_string())));
        assert_eq!(map.subagents()[0].display_name, Some("Redesign the deck".to_string()));
    }
```

- [ ] **Step 8: Run tests**

Run: `cd src-tauri && cargo test subagent`
Expected: PASS — the new id-keyed tests plus all migrated `None`-based tests (which now exercise the FIFO fallback path).

Then run the full suite: `cargo test`
Expected: PASS (including `status_server` and `status_parser` tests).

- [ ] **Step 9: Commit**

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/.claude/worktrees/hook-based-teammate-support
git add src-tauri/src/subagent_tracker.rs src-tauri/src/subagent_tracker_tests.rs \
        src-tauri/src/status_parser_tests.rs src-tauri/src/status_server.rs
git commit -m "$(cat <<'EOF'
refactor: key subagent tracker by agent_id with upsert

process_start/process_stop now take an optional agent_id and upsert/match
on it, falling back to FIFO-by-type when absent. Fixes fragile
oldest-working matching for regular subagents and lays the groundwork for
teammate tracking. Backward compatible; payload unchanged.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: Route teammate hooks and register them

Recognise `TaskCreated` (start-like) and `TeammateIdle` (stop-like) in the status server, and register both hooks so AO-launched Claude sessions actually send them. After this task, teammates appear in the fleet list.

**Files:**
- Modify: `src-tauri/src/status_server.rs:150-164` (event mapping) and the `is_start`/`is_stop` lines from Task 1
- Modify: `src-tauri/src/hook_installer.rs:26-37` (`HOOK_EVENTS` 5 → 7 + doc comment)
- Test: `src-tauri/src/status_server.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `SubagentMap::process_start` / `process_stop` (from Task 1).
- Produces: no new public symbols; behavioural change only.

- [ ] **Step 1: Add failing tests for teammate routing**

Append to the `#[cfg(test)] mod tests` block in `src-tauri/src/status_server.rs`:

```rust
    #[test]
    fn test_task_created_registers_teammate() {
        let trackers = make_trackers();
        trackers.lock().unwrap().insert("ao-sess".into(), StatusTracker::new());
        let (server, port) = StatusServer::start(trackers.clone(), noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        let body = r#"{"session_id":"cc-parent","hook_event_name":"TaskCreated","agent_type":"deck-impl","agent_id":"t1"}"#;
        let line = post(port, "/status/ao-sess", body);
        assert_eq!(status_code(&line), 200);

        let map = trackers.lock().unwrap();
        let tracker = map.get("ao-sess").unwrap();
        let payload = tracker.subagent_map().payload();
        assert_eq!(payload.len(), 1);
        assert_eq!(payload[0].name, Some("deck-impl".to_string()));
        assert_eq!(payload[0].status, crate::status_parser::SessionStatus::Working);

        server.stop();
    }

    #[test]
    fn test_task_created_same_agent_id_dedupes() {
        let trackers = make_trackers();
        trackers.lock().unwrap().insert("ao-sess".into(), StatusTracker::new());
        let (server, port) = StatusServer::start(trackers.clone(), noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        let body = r#"{"session_id":"cc-parent","hook_event_name":"TaskCreated","agent_type":"deck-impl","agent_id":"t1"}"#;
        post(port, "/status/ao-sess", body);
        // A second task for the same teammate must not create a second row.
        post(port, "/status/ao-sess", body);

        let map = trackers.lock().unwrap();
        let tracker = map.get("ao-sess").unwrap();
        assert_eq!(tracker.subagent_map().subagents().len(), 1);

        server.stop();
    }

    #[test]
    fn test_teammate_idle_marks_finished() {
        let trackers = make_trackers();
        trackers.lock().unwrap().insert("ao-sess".into(), StatusTracker::new());
        let (server, port) = StatusServer::start(trackers.clone(), noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        let start = r#"{"session_id":"cc-parent","hook_event_name":"TaskCreated","agent_type":"deck-impl","agent_id":"t1"}"#;
        post(port, "/status/ao-sess", start);
        let idle = r#"{"session_id":"cc-parent","hook_event_name":"TeammateIdle","agent_id":"t1"}"#;
        let line = post(port, "/status/ao-sess", idle);
        assert_eq!(status_code(&line), 200);

        let map = trackers.lock().unwrap();
        let tracker = map.get("ao-sess").unwrap();
        let subagents = tracker.subagent_map().subagents();
        assert_eq!(subagents.len(), 1);
        assert_eq!(subagents[0].status, crate::status_parser::SessionStatus::Finished);

        server.stop();
    }
```

- [ ] **Step 2: Run the new tests to verify they fail**

Run: `cd src-tauri && cargo test --lib status_server::tests::test_task_created_registers_teammate`
Expected: FAIL — currently `TaskCreated` is an unrecognised `hook_event_name`, so the server responds 400 (not 200) and no subagent is registered.

- [ ] **Step 3: Add the event mapping in `status_server.rs`**

In the `notification_type` chain (lines ~150-164), add two arms before the final `else` (the 400 branch):

```rust
    } else if hook_event_name == Some("SubagentStart") {
        "subagent_start".to_string()
    } else if hook_event_name == Some("TaskCreated") {
        "task_created".to_string()
    } else if hook_event_name == Some("TeammateIdle") {
        "teammate_idle".to_string()
    } else if hook_event_name == Some("PreToolUse") {
        "pre_tool_use".to_string()
    } else {
        let _ = request.respond(tiny_http::Response::empty(400));
        return;
    };
```

- [ ] **Step 4: Fold the new events into the start/stop buckets**

Change the two lines added in Task 1 Step 4:

```rust
                let is_start = notification_type == "subagent_start"
                    || notification_type == "task_created";
                let is_stop = notification_type == "subagent_stop"
                    || notification_type == "teammate_idle";
```

(The rest of the dispatch block is unchanged — it already routes `is_start` → `process_start` and `is_stop` → `process_stop`.)

- [ ] **Step 5: Run the teammate tests to verify they pass**

Run: `cd src-tauri && cargo test --lib status_server::tests::test_task_created status_server::tests::test_teammate_idle`
Expected: PASS.

- [ ] **Step 6: Register the two hooks in `hook_installer.rs`**

Update the doc comment (lines ~26-30) and the `HOOK_EVENTS` array (lines 31-37):

```rust
/// Hook events forwarded to the status server. Notification drives
/// Working/Idle/NeedsAttention transitions, Stop fires immediately on task
/// completion, SubagentStart/SubagentStop track subagent lifecycles,
/// TaskCreated/TeammateIdle track in-process teammate lifecycles, and
/// PreToolUse exists solely for early worktree CWD discovery via the
/// script's X-Cwd header.
const HOOK_EVENTS: [&str; 7] = [
    "Notification",
    "Stop",
    "SubagentStop",
    "SubagentStart",
    "PreToolUse",
    "TaskCreated",
    "TeammateIdle",
];
```

- [ ] **Step 7: Run the full backend suite**

Run: `cd src-tauri && cargo test`
Expected: PASS. The `hook_installer` tests iterate over `HOOK_EVENTS` (e.g. `test_session_hook_settings_covers_all_events`), so they automatically cover the two new events. If any `hook_installer` test hard-codes the old event set, update its fixture to match; none is expected to.

- [ ] **Step 8: Commit**

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/.claude/worktrees/hook-based-teammate-support
git add src-tauri/src/status_server.rs src-tauri/src/hook_installer.rs
git commit -m "$(cat <<'EOF'
feat: track in-process teammates in the fleet view

Recognise TaskCreated (appearance) and TeammateIdle (finish) hooks and
register them per-session. Teammates now appear in the subagent list,
keyed by agent_id like regular subagents. Binary Working/Finished.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: End-to-end validation (verification before completion)

The unit tests assume specific teammate-hook payload fields (`agent_id`, `agent_type`) that are **not documented** by Claude Code. This task confirms the feature works against a real teammate and validates assumptions V1–V4 from the spec. It requires driving the GUI, so it is a manual task.

**Files:** none (observational).

- [ ] **Step 1: Capture real teammate hook payloads**

A capture harness already exists at
`/private/tmp/claude-502/-Users-stanton-borthwick-Projects-pr-dashboard/786efe4b-7774-474d-8ba6-4f008f9254dd/scratchpad/hook-capture/` (`capture-settings.json`, `log-hooks.sh`). In a project where teammates work, run:

```bash
claude --settings "/private/tmp/claude-502/-Users-stanton-borthwick-Projects-pr-dashboard/786efe4b-7774-474d-8ba6-4f008f9254dd/scratchpad/hook-capture/capture-settings.json"
```

Spawn a named teammate (e.g. "spawn a teammate named probe to run `echo hi`"), let it start and finish, then quit. Inspect the log:

```bash
cat "/private/tmp/claude-502/-Users-stanton-borthwick-Projects-pr-dashboard/786efe4b-7774-474d-8ba6-4f008f9254dd/scratchpad/hook-capture/events.jsonl"
```

Confirm for `TaskCreated` and `TeammateIdle`:
- **V1** `agent_id` is present and identical across the teammate's events.
- **V2** the teammate name is available (`agent_type`, or another field).
- **V3** `TaskCreated`'s `agent_id` is the teammate, not the orchestrator.

If field names differ, adjust the `json.get(...)` keys in `status_server.rs` (Task 1 Step 4 / Task 2 Step 3) and re-run `cargo test`.

- [ ] **Step 2: Confirm the teammate appears in AO**

Run AO from the worktree in dev mode:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/.claude/worktrees/hook-based-teammate-support
npm install    # first run in this worktree only
npm run tauri dev
```

Start a Claude session inside AO, spawn a named teammate, and confirm it appears in the fleet list with a green (Working) dot and its name, and turns grey (Finished) after it goes idle/stops. Regular subagents must still appear correctly (regression check).

- [ ] **Step 3: Record the outcome**

If everything works, the feature is complete. If the capture contradicted V1–V3 and required field-name changes, note them in the spec's Assumptions section and amend the commit(s). If teammates never emit `TeammateIdle` or `SubagentStop` (so they never reach `Finished`), record this as a known limitation and open a follow-up to consider `TaskCompleted` or an idle-based finish.

---

## Self-Review

**Spec coverage:**
- §1 Hook registration → Task 2 Step 6. ✓
- §2 Status-server parsing (`agent_id` extraction + start/stop buckets) → Task 1 Steps 4, Task 2 Steps 3-4. ✓
- §3 Tracker refactor (`agent_id` field, upsert, FIFO fallback) → Task 1 Steps 1-3. ✓
- §4 Frontend unchanged → no task (constraint enforced by not touching `src/`). ✓
- §6 Status mapping (SubagentStart/TaskCreated→start; SubagentStop/TeammateIdle→stop; TaskCompleted ignored) → Task 2 Steps 3-4. ✓
- §9 Testing → Task 1 Step 7, Task 2 Step 1. ✓
- Assumptions V1-V4 → Task 3. ✓

**Placeholder scan:** No TBD/TODO; every code step shows complete code; every command has an expected result. ✓

**Type consistency:** `process_start(Option<&str>, &str, Option<String>) -> bool` and `process_stop(Option<&str>, &str) -> bool` are used identically in the tracker (Task 1 Steps 2-3), the production call site (Task 1 Step 4, passing `agent_id.as_deref()` / `aid`), and the migrated tests (Step 5 adds a leading `None,`). Event strings `"task_created"`/`"teammate_idle"` are produced in Task 2 Step 3 and consumed in Task 2 Step 4. ✓
