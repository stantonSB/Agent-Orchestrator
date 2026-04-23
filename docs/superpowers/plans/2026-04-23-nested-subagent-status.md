# Nested Subagent Status Tracking — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface per-subagent status as nested entries in the sidebar when a Claude Code session dispatches parallel subagents.

**Architecture:** Backend-centric approach. The Rust status server detects subagents by tracking unique `session_id` values in hook payloads per `AO_SESSION_ID`. It emits `session-subagents-*` Tauri events containing the full subagent list. The frontend subscribes and renders nested entries in the sidebar. Status bubbling (subagent `needs_attention` → parent) happens in the backend.

**Tech Stack:** Rust (Tauri 2), React, Zustand, TypeScript, xterm.js, Vitest

**Spec:** `docs/superpowers/specs/2026-04-23-nested-subagent-status-design.md`

---

## File Structure

### New Files
- `src-tauri/src/subagent_tracker.rs` — `SubagentInfo`, `SubagentMap` structs; detection logic, status bubbling, event serialization
- `src-tauri/src/subagent_tracker_tests.rs` — Unit tests for subagent tracking logic
- `src/components/SubagentList/SubagentList.tsx` — Renders nested subagent entries under parent session card
- `src/components/SubagentList/SubagentList.module.css` — Styles for subagent entries
- `src/components/SubagentList/SubagentList.test.tsx` — Component tests

### Modified Files
- `src-tauri/src/lib.rs` — Register `subagent_tracker` module, add `session-subagents-*` event emission
- `src-tauri/src/status_parser.rs` — Add `SubagentMap` field to `StatusTracker`, expose subagent methods
- `src-tauri/src/status_server.rs` — Extract `session_id` from hook JSON, route parent vs subagent events
- `src-tauri/src/hook_installer.rs` — Install `SubagentStop` hook entry
- `src/types/session.ts` — Add `SubagentStatus` type
- `src/stores/sessionStore.ts` — Add `subagents` map, event listener for `session-subagents-*`, cleanup timer logic
- `src/stores/sessionStore.test.ts` — Tests for subagent store logic
- `src/components/ProjectGroup/ProjectGroup.tsx` — Pass subagents to `SubagentList` after each `SessionCard`

---

## Chunk 1: Backend — SubagentTracker Module & Tests

### Task 1: Create `SubagentInfo` and `SubagentMap` structs

**Files:**
- Create: `src-tauri/src/subagent_tracker.rs`
- Create: `src-tauri/src/subagent_tracker_tests.rs`
- Modify: `src-tauri/src/lib.rs:1-9`

- [ ] **Step 1: Write the failing test — SubagentMap registers first session_id as parent**

In `src-tauri/src/subagent_tracker_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::subagent_tracker::SubagentMap;

    #[test]
    fn test_first_session_id_becomes_parent() {
        let mut map = SubagentMap::new();
        map.process_event("parent-cc-id", "idle_prompt");
        assert_eq!(map.parent_session_id(), Some("parent-cc-id"));
        assert_eq!(map.subagents().len(), 0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test subagent_tracker_tests`
Expected: FAIL — module does not exist

- [ ] **Step 3: Create the module with minimal implementation**

In `src-tauri/src/subagent_tracker.rs`:

```rust
use crate::status_parser::SessionStatus;
use std::collections::HashMap;
use std::time::Instant;

/// Metadata for a single detected subagent.
#[derive(Debug, Clone)]
pub struct SubagentInfo {
    pub claude_session_id: String,
    pub index: u16,
    pub status: SessionStatus,
    pub name: Option<String>,
    pub finished_at: Option<Instant>,
}

/// Serializable subagent info sent to the frontend via Tauri events.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SubagentStatusPayload {
    pub id: String,
    pub index: u16,
    pub status: SessionStatus,
    pub name: Option<String>,
}

impl From<&SubagentInfo> for SubagentStatusPayload {
    fn from(info: &SubagentInfo) -> Self {
        Self {
            id: info.claude_session_id.clone(),
            index: info.index,
            status: info.status.clone(),
            name: info.name.clone(),
        }
    }
}

/// Tracks subagents for a single parent session.
///
/// The first `session_id` seen via `process_event` is recorded as the parent.
/// Subsequent new `session_id` values are registered as subagents.
pub struct SubagentMap {
    parent_claude_id: Option<String>,
    agents: HashMap<String, SubagentInfo>,
    next_index: u16,
    pending_events: Vec<(String, String)>, // (session_id, notification_type)
}

const MAX_PENDING: usize = 32;

impl SubagentMap {
    pub fn new() -> Self {
        Self {
            parent_claude_id: None,
            agents: HashMap::new(),
            next_index: 1,
            pending_events: Vec::new(),
        }
    }

    /// Returns the parent's Claude Code session_id, if established.
    pub fn parent_session_id(&self) -> Option<&str> {
        self.parent_claude_id.as_deref()
    }

    /// Returns a slice of all tracked subagents.
    pub fn subagents(&self) -> Vec<&SubagentInfo> {
        self.agents.values().collect()
    }

    /// Returns serializable payload for all subagents.
    pub fn payload(&self) -> Vec<SubagentStatusPayload> {
        let mut list: Vec<_> = self.agents.values().map(SubagentStatusPayload::from).collect();
        list.sort_by_key(|s| s.index);
        list
    }

    /// Returns true if any subagent is in NeedsAttention status.
    pub fn any_needs_attention(&self) -> bool {
        self.agents.values().any(|a| a.status == SessionStatus::NeedsAttention)
    }

    /// Process a hook event. Returns true if subagent state changed (triggering a re-emit).
    ///
    /// `claude_session_id`: the session_id from the hook JSON body
    /// `notification_type`: normalized event type (idle_prompt, permission_prompt, stop, etc.)
    pub fn process_event(&mut self, claude_session_id: &str, notification_type: &str) -> bool {
        // Establish parent identity on first event
        if self.parent_claude_id.is_none() {
            self.parent_claude_id = Some(claude_session_id.to_string());
            // Replay any pending events
            let pending = std::mem::take(&mut self.pending_events);
            let mut changed = false;
            for (sid, ntype) in pending {
                if sid != claude_session_id {
                    changed |= self.register_or_update_subagent(&sid, &ntype);
                }
            }
            return changed;
        }

        // Parent event — not our concern
        if self.parent_claude_id.as_deref() == Some(claude_session_id) {
            return false;
        }

        // Subagent event
        self.register_or_update_subagent(claude_session_id, notification_type)
    }

    /// Returns true if this session_id is the known parent.
    pub fn is_parent(&self, claude_session_id: &str) -> bool {
        self.parent_claude_id.as_deref() == Some(claude_session_id)
    }

    /// Returns true if parent identity has not been established yet.
    pub fn parent_unknown(&self) -> bool {
        self.parent_claude_id.is_none()
    }

    /// Buffer an event when parent identity is not yet established.
    pub fn buffer_event(&mut self, claude_session_id: &str, notification_type: &str) {
        if self.pending_events.len() >= MAX_PENDING {
            self.pending_events.remove(0);
        }
        self.pending_events.push((
            claude_session_id.to_string(),
            notification_type.to_string(),
        ));
    }

    fn register_or_update_subagent(&mut self, claude_session_id: &str, notification_type: &str) -> bool {
        if let Some(agent) = self.agents.get_mut(claude_session_id) {
            // Update existing subagent status
            let new_status = Self::notification_to_status(notification_type, &agent.status);
            if let Some(status) = new_status {
                let finished = status == SessionStatus::Finished;
                agent.status = status;
                if finished {
                    agent.finished_at = Some(Instant::now());
                }
                return true;
            }
            false
        } else {
            // New subagent detected
            let status = Self::initial_status(notification_type);
            let index = self.next_index;
            self.next_index += 1;
            let finished = status == SessionStatus::Finished;
            self.agents.insert(
                claude_session_id.to_string(),
                SubagentInfo {
                    claude_session_id: claude_session_id.to_string(),
                    index,
                    status,
                    name: None,
                    finished_at: if finished { Some(Instant::now()) } else { None },
                },
            );
            true
        }
    }

    /// Map a notification type to a new status given the current subagent status.
    /// Key difference from parent: subagents in `Idle` can transition to
    /// `Finished` on stop/idle_prompt, since they may go idle briefly before
    /// their Stop hook fires.
    fn notification_to_status(notification_type: &str, current: &SessionStatus) -> Option<SessionStatus> {
        match notification_type {
            "idle_prompt" | "stop" => match current {
                SessionStatus::Starting => Some(SessionStatus::Idle),
                SessionStatus::Working | SessionStatus::NeedsAttention | SessionStatus::Idle => {
                    Some(SessionStatus::Finished)
                }
                _ => None,
            },
            "permission_prompt" | "elicitation_dialog" => match current {
                SessionStatus::Working | SessionStatus::Starting | SessionStatus::Idle => {
                    Some(SessionStatus::NeedsAttention)
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Map the first notification type seen to an initial status.
    fn initial_status(notification_type: &str) -> SessionStatus {
        match notification_type {
            "idle_prompt" | "stop" => SessionStatus::Idle,
            "permission_prompt" | "elicitation_dialog" => SessionStatus::NeedsAttention,
            _ => SessionStatus::Working,
        }
    }
}
```

- [ ] **Step 4: Register module in lib.rs**

In `src-tauri/src/lib.rs`, add after line 5 (`pub mod status_parser;`):

```rust
pub mod subagent_tracker;
```

And add the test module after `mod status_parser_tests;` (line 9):

```rust
#[cfg(test)]
mod subagent_tracker_tests;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd src-tauri && cargo test subagent_tracker_tests`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/subagent_tracker.rs src-tauri/src/subagent_tracker_tests.rs src-tauri/src/lib.rs
git commit -m "feat: add SubagentMap and SubagentInfo structs with detection logic"
```

### Task 2: Write comprehensive SubagentMap tests

**Files:**
- Modify: `src-tauri/src/subagent_tracker_tests.rs`

- [ ] **Step 1: Add tests for subagent detection, buffering, status transitions, and payload serialization**

Append to `src-tauri/src/subagent_tracker_tests.rs` inside the `tests` module:

```rust
    #[test]
    fn test_second_session_id_becomes_subagent() {
        let mut map = SubagentMap::new();
        map.process_event("parent-id", "idle_prompt");
        let changed = map.process_event("child-id", "idle_prompt");
        assert!(changed);
        assert_eq!(map.subagents().len(), 1);
        assert_eq!(map.subagents()[0].claude_session_id, "child-id");
        assert_eq!(map.subagents()[0].index, 1);
    }

    #[test]
    fn test_multiple_subagents_get_sequential_indexes() {
        let mut map = SubagentMap::new();
        map.process_event("parent", "idle_prompt");
        map.process_event("child-a", "idle_prompt");
        map.process_event("child-b", "permission_prompt");
        map.process_event("child-c", "idle_prompt");

        let mut agents: Vec<_> = map.subagents().into_iter().collect();
        agents.sort_by_key(|a| a.index);
        assert_eq!(agents.len(), 3);
        assert_eq!(agents[0].index, 1);
        assert_eq!(agents[1].index, 2);
        assert_eq!(agents[2].index, 3);
    }

    #[test]
    fn test_subagent_status_transitions() {
        let mut map = SubagentMap::new();
        map.process_event("parent", "idle_prompt");
        map.process_event("child", "idle_prompt"); // initial: Idle

        let changed = map.process_event("child", "permission_prompt");
        assert!(changed);
        // Find the child agent
        let child = map.subagents().into_iter().find(|a| a.claude_session_id == "child").unwrap();
        assert_eq!(child.status, crate::status_parser::SessionStatus::NeedsAttention);
    }

    #[test]
    fn test_subagent_stop_sets_finished_and_finished_at() {
        let mut map = SubagentMap::new();
        map.process_event("parent", "idle_prompt");
        map.process_event("child", "permission_prompt"); // NeedsAttention

        let changed = map.process_event("child", "stop");
        assert!(changed);
        let child = map.subagents().into_iter().find(|a| a.claude_session_id == "child").unwrap();
        assert_eq!(child.status, crate::status_parser::SessionStatus::Finished);
        assert!(child.finished_at.is_some());
    }

    #[test]
    fn test_parent_event_returns_false() {
        let mut map = SubagentMap::new();
        map.process_event("parent", "idle_prompt");
        let changed = map.process_event("parent", "permission_prompt");
        assert!(!changed);
        assert_eq!(map.subagents().len(), 0);
    }

    #[test]
    fn test_any_needs_attention() {
        let mut map = SubagentMap::new();
        map.process_event("parent", "idle_prompt");
        map.process_event("child-a", "idle_prompt");
        assert!(!map.any_needs_attention());

        map.process_event("child-b", "permission_prompt");
        assert!(map.any_needs_attention());
    }

    #[test]
    fn test_buffering_when_parent_unknown() {
        let mut map = SubagentMap::new();
        // Buffer events before parent is established
        map.buffer_event("child-1", "idle_prompt");
        map.buffer_event("child-2", "permission_prompt");
        assert!(map.parent_unknown());
        assert_eq!(map.subagents().len(), 0);

        // Parent arrives — buffered events are replayed
        let changed = map.process_event("parent", "idle_prompt");
        assert!(changed); // buffered child events were replayed
        assert_eq!(map.subagents().len(), 2);
    }

    #[test]
    fn test_buffer_capacity_drops_oldest() {
        let mut map = SubagentMap::new();
        for i in 0..35 {
            map.buffer_event(&format!("child-{i}"), "idle_prompt");
        }
        // Buffer should be capped at 32
        map.process_event("parent", "idle_prompt");
        // First 3 dropped, 32 remain, but parent matches none of them
        assert_eq!(map.subagents().len(), 32);
    }

    #[test]
    fn test_payload_serialization() {
        let mut map = SubagentMap::new();
        map.process_event("parent", "idle_prompt");
        map.process_event("child-a", "idle_prompt");
        map.process_event("child-b", "permission_prompt");

        let payload = map.payload();
        assert_eq!(payload.len(), 2);
        assert_eq!(payload[0].index, 1); // sorted by index
        assert_eq!(payload[1].index, 2);

        // Verify it serializes to JSON
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("child-a"));
        assert!(json.contains("child-b"));
    }

    #[test]
    fn test_is_parent() {
        let mut map = SubagentMap::new();
        map.process_event("parent-id", "idle_prompt");
        assert!(map.is_parent("parent-id"));
        assert!(!map.is_parent("other-id"));
    }

    #[test]
    fn test_duplicate_subagent_event_no_change_returns_false() {
        let mut map = SubagentMap::new();
        map.process_event("parent", "idle_prompt");
        map.process_event("child", "idle_prompt"); // Idle
        let changed = map.process_event("child", "idle_prompt"); // Idle → Idle: no transition
        assert!(!changed);
    }
```

- [ ] **Step 2: Run all subagent tracker tests**

Run: `cd src-tauri && cargo test subagent_tracker_tests`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/subagent_tracker_tests.rs
git commit -m "test: add comprehensive SubagentMap unit tests"
```

### Task 3: Install `SubagentStop` hook

**Files:**
- Modify: `src-tauri/src/hook_installer.rs:68-95` (is_already_installed)
- Modify: `src-tauri/src/hook_installer.rs:161-237` (merge_hook_settings)

- [ ] **Step 1: Write the failing test — SubagentStop hook is installed**

Add to the existing `#[cfg(test)] mod tests` block in `src-tauri/src/hook_installer.rs`:

```rust
    #[test]
    fn test_subagent_stop_hook_installed() {
        let home = temp_home();
        ensure_hooks_installed_in(home.path());

        let content = fs::read_to_string(settings_path(&home)).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(hook_array_has_our_script(&val, "SubagentStop"));
    }

    #[test]
    fn test_adds_subagent_stop_when_only_notification_and_stop_exist() {
        let home = temp_home();
        fs::create_dir_all(claude_dir(&home)).unwrap();

        let existing = serde_json::json!({
            "hooks": {
                "Notification": [{
                    "matcher": "",
                    "hooks": [{ "type": "command", "command": "${HOME}/.claude/agent-orchestrator-notify.sh" }]
                }],
                "Stop": [{
                    "matcher": "",
                    "hooks": [{ "type": "command", "command": "${HOME}/.claude/agent-orchestrator-notify.sh" }]
                }]
            }
        });
        fs::write(settings_path(&home), serde_json::to_string_pretty(&existing).unwrap()).unwrap();
        write_hook_script(&script_path(&home)).unwrap();
        set_idle_threshold(&profile_path(&home)).unwrap();

        let result = ensure_hooks_installed_in(home.path());
        assert_eq!(result, HookInstallResult::Installed);
        assert!(hook_array_has_our_script(
            &serde_json::from_str::<serde_json::Value>(&fs::read_to_string(settings_path(&home)).unwrap()).unwrap(),
            "SubagentStop"
        ));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test test_subagent_stop`
Expected: FAIL — SubagentStop hook not installed

- [ ] **Step 3: Update `is_already_installed` to check for SubagentStop**

In `src-tauri/src/hook_installer.rs`, add to `is_already_installed` function after the `settings_has_our_stop_hook` check (around line 87):

```rust
    if !settings_has_our_subagent_stop_hook(settings_path) {
        return false;
    }
```

Add the helper function after `settings_has_our_stop_hook`:

```rust
fn settings_has_our_subagent_stop_hook(settings_path: &Path) -> bool {
    let content = match fs::read_to_string(settings_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let val: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };
    hook_array_has_our_script(&val, "SubagentStop")
}
```

- [ ] **Step 4: Update `merge_hook_settings` to install SubagentStop hook**

In `merge_hook_settings`, add alongside the existing `need_notification` and `need_stop` checks:

```rust
    let need_subagent_stop = !hook_array_has_our_script(&val, "SubagentStop");
```

Update the early return check:

```rust
    if !need_notification && !need_stop && !need_subagent_stop {
        return Ok(());
    }
```

Add the SubagentStop hook installation block after the Stop hook block:

```rust
    if need_subagent_stop {
        let subagent_stop_hooks = hooks_obj
            .entry("SubagentStop")
            .or_insert_with(|| serde_json::Value::Array(vec![]));
        subagent_stop_hooks
            .as_array_mut()
            .ok_or("SubagentStop is not an array")?
            .push(our_hook_entry.clone());
    }
```

**Important:** The existing code moves `our_hook_entry` into the Stop push (no `.clone()`). Since we now have a third usage, change the existing Stop push from `.push(our_hook_entry)` to `.push(our_hook_entry.clone())` so that `our_hook_entry` is still available for the SubagentStop push. All three push sites should use `.clone()` (or the last one can consume the value).

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test hook_installer`
Expected: All PASS (including existing tests)

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/hook_installer.rs
git commit -m "feat: install SubagentStop hook alongside Notification and Stop hooks"
```

### Task 4: Integrate SubagentMap into StatusTracker

**Files:**
- Modify: `src-tauri/src/status_parser.rs:39-48` (StatusTracker struct and constructor)

- [ ] **Step 1: Write the failing test — StatusTracker has subagent_map**

Add to `src-tauri/src/status_parser_tests.rs`:

```rust
    // -----------------------------------------------------------------------
    // SubagentMap integration
    // -----------------------------------------------------------------------

    #[test]
    fn test_status_tracker_has_subagent_map() {
        let tracker = StatusTracker::new();
        assert!(tracker.subagent_map().parent_session_id().is_none());
    }

    #[test]
    fn test_subagent_map_mut_allows_modification() {
        let mut tracker = StatusTracker::new();
        tracker.subagent_map_mut().process_event("parent", "idle_prompt");
        assert_eq!(tracker.subagent_map().parent_session_id(), Some("parent"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test status_parser_tests::tests::test_status_tracker_has_subagent_map`
Expected: FAIL — method does not exist

- [ ] **Step 3: Add SubagentMap field to StatusTracker**

In `src-tauri/src/status_parser.rs`, add the import at the top:

```rust
use crate::subagent_tracker::SubagentMap;
```

Update the struct and impl:

```rust
pub struct StatusTracker {
    status: SessionStatus,
    subagent_map: SubagentMap,
}

impl StatusTracker {
    pub fn new() -> Self {
        Self {
            status: SessionStatus::Starting,
            subagent_map: SubagentMap::new(),
        }
    }

    pub fn subagent_map(&self) -> &SubagentMap {
        &self.subagent_map
    }

    pub fn subagent_map_mut(&mut self) -> &mut SubagentMap {
        &mut self.subagent_map
    }

    // ... existing methods unchanged ...
```

- [ ] **Step 4: Run all status_parser tests to verify nothing broke**

Run: `cd src-tauri && cargo test status_parser`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/status_parser.rs src-tauri/src/status_parser_tests.rs
git commit -m "feat: integrate SubagentMap into StatusTracker"
```

### Task 5: Update status server to route subagent events

**Files:**
- Modify: `src-tauri/src/status_server.rs:71-146` (handle_request function)
- Modify: `src-tauri/src/lib.rs:56-65` (add subagents event emission callback)

- [ ] **Step 1: Write the failing test — subagent detection via status server**

Add to `src-tauri/src/status_server.rs` tests module:

```rust
    #[test]
    fn test_subagent_detected_from_different_session_id() {
        let trackers = make_trackers();
        trackers.lock().unwrap().insert("ao-sess".into(), StatusTracker::new());

        let (server, port) = StatusServer::start(trackers.clone(), noop_callback(), noop_subagent_callback());

        // First event establishes parent
        let body = r#"{"session_id":"cc-parent","notification_type":"idle_prompt"}"#;
        post(port, "/status/ao-sess", body);

        // Second event from different session_id is a subagent
        let body = r#"{"session_id":"cc-child","notification_type":"idle_prompt"}"#;
        let line = post(port, "/status/ao-sess", body);
        assert_eq!(status_code(&line), 200);

        // Verify subagent was registered
        let map = trackers.lock().unwrap();
        let tracker = map.get("ao-sess").unwrap();
        assert_eq!(tracker.subagent_map().subagents().len(), 1);

        server.stop();
    }
```

This requires adding a second callback parameter to `StatusServer::start` for subagent events.

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test test_subagent_detected`
Expected: FAIL — signature mismatch

- [ ] **Step 3: Add subagent callback to StatusServer::start**

Add a new callback type in `src-tauri/src/pty_manager.rs` (near existing callback types, line 116-118):

```rust
pub type SubagentCallback = Box<dyn Fn(SessionId, Vec<crate::subagent_tracker::SubagentStatusPayload>) + Send + Sync + 'static>;
```

Update `StatusServer::start` in `src-tauri/src/status_server.rs` to accept the new callback:

```rust
    pub fn start(
        trackers: Arc<Mutex<HashMap<String, StatusTracker>>>,
        on_status: Arc<StatusCallback>,
        on_subagents: Arc<SubagentCallback>,
    ) -> (Self, u16) {
```

Thread it through to `accept_loop` and `handle_request`.

- [ ] **Step 4: Update `handle_request` to extract `session_id` and route**

Replace the notification_type extraction and tracker lookup in `handle_request` with:

```rust
    // Extract session_id from the JSON body (required for subagent routing).
    let claude_session_id = json.get("session_id").and_then(|v| v.as_str()).map(|s| s.to_string());

    // Extract the event type.
    let notification_type = if let Some(t) = json.get("notification_type").and_then(|v| v.as_str()) {
        t.to_string()
    } else if json.get("hook_event_name").and_then(|v| v.as_str()) == Some("Stop") {
        "stop".to_string()
    } else if json.get("hook_event_name").and_then(|v| v.as_str()) == Some("SubagentStop") {
        "subagent_stop".to_string()
    } else {
        let _ = request.respond(tiny_http::Response::empty(400));
        return;
    };

    // Look up tracker and apply the event.
    let (transition, subagent_changed) = {
        let mut map = trackers.lock().unwrap();
        match map.get_mut(&ao_session_id) {
            Some(tracker) => {
                let mut subagent_changed = false;

                if let Some(ref cc_id) = claude_session_id {
                    let submap = tracker.subagent_map_mut();
                    if submap.parent_unknown() {
                        // First event establishes parent, may replay buffered
                        subagent_changed = submap.process_event(cc_id, &notification_type);
                    } else if submap.is_parent(cc_id) {
                        // Parent event — handle SubagentStop specially
                        if notification_type == "subagent_stop" {
                            // SubagentStop fires on parent; metadata extraction is best-effort
                            // No status transition for the parent from this event
                        }
                        // Normal parent event processing continues below
                    } else {
                        // Subagent event
                        subagent_changed = submap.process_event(cc_id, &notification_type);
                    }
                }

                // Only process parent status transitions for parent events (or when no session_id)
                let is_parent_event = claude_session_id.as_ref()
                    .map(|id| tracker.subagent_map().is_parent(id) || tracker.subagent_map().parent_unknown())
                    .unwrap_or(true);

                let transition = if is_parent_event && notification_type != "subagent_stop" {
                    // Check for status bubbling after subagent changes
                    let base_transition = tracker.notify_hook_event(&notification_type);

                    // Bubble: if any subagent needs attention and parent is working/idle,
                    // emit needs_attention for the parent
                    if subagent_changed && tracker.subagent_map().any_needs_attention() {
                        match tracker.status() {
                            SessionStatus::Working | SessionStatus::Idle => {
                                Some(SessionStatus::NeedsAttention)
                            }
                            _ => base_transition,
                        }
                    } else {
                        base_transition
                    }
                } else {
                    // Subagent or SubagentStop event — check if bubbling needed
                    if subagent_changed && tracker.subagent_map().any_needs_attention() {
                        match tracker.status() {
                            SessionStatus::Working | SessionStatus::Idle => {
                                Some(SessionStatus::NeedsAttention)
                            }
                            _ => None,
                        }
                    } else if subagent_changed && !tracker.subagent_map().any_needs_attention() {
                        // Subagent resolved — re-emit parent's true status so frontend
                        // can restore from the bubbled needs_attention state
                        Some(tracker.status().clone())
                    } else {
                        None
                    }
                };

                (transition, subagent_changed)
            }
            None => {
                drop(map);
                let _ = request.respond(tiny_http::Response::empty(404));
                return;
            }
        }
    };

    // Emit subagent list update if changed
    if subagent_changed {
        let payload = {
            let map = trackers.lock().unwrap();
            if let Some(tracker) = map.get(&ao_session_id) {
                Some(tracker.subagent_map().payload())
            } else {
                None
            }
        };
        if let Some(payload) = payload {
            on_subagents(ao_session_id.clone(), payload);
        }
    }

    // Emit parent status change if occurred
    match transition {
        Some(new_status) => {
            on_status(ao_session_id, new_status.as_str().to_string());
            let _ = request.respond(tiny_http::Response::empty(200));
        }
        None => {
            let code = if subagent_changed { 200 } else { 204 };
            let _ = request.respond(tiny_http::Response::empty(code));
        }
    }
```

- [ ] **Step 5: Add `noop_subagent_callback` helper to tests and update existing test calls**

```rust
    fn noop_subagent_callback() -> Arc<SubagentCallback> {
        Arc::new(Box::new(|_id: String, _payload: Vec<crate::subagent_tracker::SubagentStatusPayload>| {}))
    }
```

Update all existing `StatusServer::start` calls in tests to pass the third parameter.

- [ ] **Step 6: Update `lib.rs` to wire subagent callback and emit events**

In `src-tauri/src/lib.rs`, add a new event handle clone (after line 23):

```rust
            let handle_for_subagents = app.handle().clone();
```

Add the subagent callback (after the `on_status` closure, around line 65):

```rust
            let on_subagents: pty_manager::SubagentCallback =
                Box::new(move |id, payload| {
                    let event_name = format!("session-subagents-{}", id);
                    let _ = handle_for_subagents.emit(&event_name, payload);
                });
```

Update `StatusServer::start` call to pass the new callback:

```rust
            let on_subagents_arc: Arc<pty_manager::SubagentCallback> = Arc::new(on_subagents);
            let (status_server, status_port) =
                status_server::StatusServer::start(status_trackers.clone(), on_status_for_server, on_subagents_arc);
```

- [ ] **Step 7: Run all backend tests**

Run: `cd src-tauri && cargo test`
Expected: All PASS

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/status_server.rs src-tauri/src/lib.rs src-tauri/src/pty_manager.rs
git commit -m "feat: route subagent events in status server, emit session-subagents events"
```

---

## Chunk 2: Frontend — Types, Store, and SubagentList Component

### Task 6: Add `SubagentStatus` TypeScript type

**Files:**
- Modify: `src/types/session.ts`

- [ ] **Step 1: Add SubagentStatus interface**

Append to `src/types/session.ts`:

```typescript
export interface SubagentStatus {
  id: string;
  index: number;
  status: SessionStatus;
  name: string | null;
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `npx tsc --noEmit`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add src/types/session.ts
git commit -m "feat: add SubagentStatus type"
```

### Task 7: Add subagent state and event listeners to sessionStore

**Files:**
- Modify: `src/stores/sessionStore.ts`
- Modify: `src/stores/sessionStore.test.ts`

- [ ] **Step 1: Write the failing test — subagent map in store**

Add to `src/stores/sessionStore.test.ts`:

```typescript
  describe("subagents", () => {
    it("initializes with empty subagents map", () => {
      const { subagents } = useSessionStore.getState();
      expect(subagents.size).toBe(0);
    });

    it("updates subagents for a session", () => {
      const store = useSessionStore.getState();
      store.updateSubagents("session-1", [
        { id: "cc-child-1", index: 1, status: "working", name: null },
        { id: "cc-child-2", index: 2, status: "idle", name: "Exploring" },
      ]);

      const { subagents } = useSessionStore.getState();
      expect(subagents.get("session-1")?.length).toBe(2);
      expect(subagents.get("session-1")?.[0].status).toBe("working");
      expect(subagents.get("session-1")?.[1].name).toBe("Exploring");
    });

    it("clears subagents when session is removed", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "session-1",
        name: "Test",
        status: "working",
        createdAt: Date.now(),
        cwd: "/test",
      });
      store.updateSubagents("session-1", [
        { id: "cc-child-1", index: 1, status: "working", name: null },
      ]);
      store.removeSession("session-1");

      const { subagents } = useSessionStore.getState();
      expect(subagents.has("session-1")).toBe(false);
    });

    it("clears subagents when session is dismissed", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "session-1",
        name: "Test",
        status: "finished",
        createdAt: Date.now(),
        cwd: "/test",
      });
      store.updateSubagents("session-1", [
        { id: "cc-child-1", index: 1, status: "finished", name: null },
      ]);
      store.dismissSession("session-1");

      const { subagents } = useSessionStore.getState();
      expect(subagents.has("session-1")).toBe(false);
    });

    it("registers listener for session-subagents event", async () => {
      const { listen } = await import("@tauri-apps/api/event");
      const store = useSessionStore.getState();
      store.setupEventListeners("test-session");

      expect(listen).toHaveBeenCalledWith(
        "session-subagents-test-session",
        expect.any(Function)
      );
    });
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/stores/sessionStore.test.ts`
Expected: FAIL — `updateSubagents` does not exist

- [ ] **Step 3: Add subagent state and methods to sessionStore**

In `src/stores/sessionStore.ts`:

Add import for SubagentStatus:

```typescript
import type { SessionInfo, SessionStatus, SubagentStatus } from "../types/session";
```

Add to `SessionState` interface:

```typescript
  subagents: Map<string, SubagentStatus[]>;
  updateSubagents: (sessionId: string, subagents: SubagentStatus[]) => void;
```

Add initial state:

```typescript
  subagents: new Map(),
```

Add `updateSubagents` method:

```typescript
  updateSubagents: (sessionId, subagentList) =>
    set((state) => {
      const next = new Map(state.subagents);
      if (subagentList.length === 0) {
        next.delete(sessionId);
      } else {
        next.set(sessionId, subagentList);
      }
      return { subagents: next };
    }),
```

Update `removeSession` to also clean up subagents:

```typescript
  removeSession: (id) =>
    set((state) => {
      const next = new Map(state.sessions);
      next.delete(id);
      const nextSubagents = new Map(state.subagents);
      nextSubagents.delete(id);
      // ... rest of existing cleanup
      return {
        sessions: next,
        subagents: nextSubagents,
        activeSessionId: state.activeSessionId === id ? null : state.activeSessionId,
      };
    }),
```

Update `dismissSession` to also clean up subagents. Add after `next.delete(id)`:

```typescript
      const nextSubagents = new Map(state.subagents);
      nextSubagents.delete(id);
```

And include `subagents: nextSubagents` in the return object:

```typescript
      return { sessions: next, subagents: nextSubagents, activeSessionId };
```

Add `session-subagents-*` listener in `setupEventListeners`:

```typescript
    cleanups.push(
      listen<SubagentStatus[]>(`session-subagents-${sessionId}`, (event) => {
        get().updateSubagents(sessionId, event.payload);
      })
    );
```

- [ ] **Step 4: Update beforeEach in tests to reset subagents**

In `sessionStore.test.ts`, update `beforeEach`:

```typescript
    useSessionStore.setState({
      sessions: new Map(),
      activeSessionId: null,
      lastUsedDirectory: null,
      subagents: new Map(),
    });
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `npx vitest run src/stores/sessionStore.test.ts`
Expected: All PASS

- [ ] **Step 6: Commit**

```bash
git add src/stores/sessionStore.ts src/stores/sessionStore.test.ts src/types/session.ts
git commit -m "feat: add subagent tracking to session store with event listeners"
```

### Task 8: Create SubagentList component

**Files:**
- Create: `src/components/SubagentList/SubagentList.tsx`
- Create: `src/components/SubagentList/SubagentList.module.css`
- Create: `src/components/SubagentList/SubagentList.test.tsx`

- [ ] **Step 1: Write the failing test**

In `src/components/SubagentList/SubagentList.test.tsx`:

```tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { SubagentList } from "./SubagentList";
import type { SubagentStatus } from "../../types/session";

describe("SubagentList", () => {
  it("renders nothing when subagents is empty", () => {
    const { container } = render(<SubagentList subagents={[]} />);
    expect(container.innerHTML).toBe("");
  });

  it("renders a dot and name for each subagent", () => {
    const subagents: SubagentStatus[] = [
      { id: "a", index: 1, status: "working", name: "Exploring codebase" },
      { id: "b", index: 2, status: "idle", name: null },
    ];
    render(<SubagentList subagents={subagents} />);
    expect(screen.getByText("Exploring codebase")).toBeTruthy();
    expect(screen.getByText("Agent 2")).toBeTruthy();
  });

  it("renders finished subagents with dimmed class", () => {
    const subagents: SubagentStatus[] = [
      { id: "a", index: 1, status: "finished", name: "Done agent" },
    ];
    const { container } = render(<SubagentList subagents={subagents} />);
    const entry = container.querySelector("[class*='finished']");
    expect(entry).toBeTruthy();
  });

  it("shows correct status dot classes", () => {
    const subagents: SubagentStatus[] = [
      { id: "a", index: 1, status: "needs_attention", name: null },
    ];
    const { container } = render(<SubagentList subagents={subagents} />);
    const dot = container.querySelector("[class*='NeedsAttention']");
    expect(dot).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/components/SubagentList/SubagentList.test.tsx`
Expected: FAIL — module does not exist

- [ ] **Step 3: Create the CSS module**

In `src/components/SubagentList/SubagentList.module.css`:

```css
.list {
  margin-left: 28px;
  padding-left: 8px;
  border-left: 1px solid rgba(255, 255, 255, 0.08);
}

.entry {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 3px 8px;
  font-size: 11px;
  color: #c0caf5;
}

.finished {
  opacity: 0.4;
}

.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.statusStarting { background-color: #3b82f6; }
.statusWorking {
  background-color: #22c55e;
  animation: pulse 1.5s ease-in-out infinite;
}
.statusIdle { background-color: #6b7280; }
.statusNeedsAttention { background-color: #f97316; }
.statusFinished { background-color: #6b7280; }
.statusError { background-color: #ef4444; }

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}

.name {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
```

- [ ] **Step 4: Create the component**

In `src/components/SubagentList/SubagentList.tsx`:

```tsx
import type { SubagentStatus, SessionStatus } from "../../types/session";
import styles from "./SubagentList.module.css";

interface SubagentListProps {
  subagents: SubagentStatus[];
}

const DOT_CLASS: Record<SessionStatus, string> = {
  starting: styles.statusStarting,
  working: styles.statusWorking,
  idle: styles.statusIdle,
  needs_attention: styles.statusNeedsAttention,
  finished: styles.statusFinished,
  error: styles.statusError,
};

export function SubagentList({ subagents }: SubagentListProps) {
  if (subagents.length === 0) return null;

  return (
    <div className={styles.list}>
      {subagents.map((agent) => (
        <div
          key={agent.id}
          className={`${styles.entry} ${agent.status === "finished" ? styles.finished : ""}`}
        >
          <span className={`${styles.dot} ${DOT_CLASS[agent.status]}`} />
          <span className={styles.name}>
            {agent.name ?? `Agent ${agent.index}`}
          </span>
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `npx vitest run src/components/SubagentList/SubagentList.test.tsx`
Expected: All PASS

- [ ] **Step 6: Commit**

```bash
git add src/components/SubagentList/
git commit -m "feat: add SubagentList component for nested sidebar display"
```

### Task 9: Integrate SubagentList into ProjectGroup

**Files:**
- Modify: `src/components/ProjectGroup/ProjectGroup.tsx`
- Modify: `src/components/SessionPanel/SessionPanel.tsx`
- Modify: `src/components/SessionPanel/SessionPanel.test.tsx`

- [ ] **Step 1: Write the failing test — subagents render in sidebar**

Append this test case inside the existing `describe("SessionPanel", ...)` block in `src/components/SessionPanel/SessionPanel.test.tsx` (after the last `it(...)` at line 123):

```tsx
  it("renders subagent entries beneath parent session", async () => {
    // Set subagent state directly via the store (already imported in test file)
    const { useSessionStore } = await import("../../stores/sessionStore");
    useSessionStore.setState({
      subagents: new Map([
        ["1", [
          { id: "cc-child-1", index: 1, status: "working", name: "Exploring" },
          { id: "cc-child-2", index: 2, status: "idle", name: null },
        ]],
      ]),
    });

    const sessions: SessionInfo[] = [
      { id: "1", name: "Feature work", status: "working", createdAt: 1000, cwd: "/projects/app" },
    ];
    render(
      <SessionPanel
        sessions={sessions}
        activeSessionId="1"
        onSessionClick={vi.fn()}
        onNewSession={vi.fn()}
      />
    );
    expect(screen.getByText("Exploring")).toBeTruthy();
    expect(screen.getByText("Agent 2")).toBeTruthy();
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/components/SessionPanel/SessionPanel.test.tsx`
Expected: FAIL — subagent text not found

- [ ] **Step 3: Update ProjectGroup to accept and render subagents**

In `src/components/ProjectGroup/ProjectGroup.tsx`:

```tsx
import type { SessionInfo, SubagentStatus } from "../../types/session";
import { SessionCard } from "../SessionCard/SessionCard";
import { SubagentList } from "../SubagentList/SubagentList";
import styles from "./ProjectGroup.module.css";

interface ProjectGroupProps {
  projectName: string;
  sessions: SessionInfo[];
  activeSessionId: string | null;
  isCollapsed: boolean;
  onToggleCollapse: () => void;
  onSessionClick: (id: string) => void;
  onClose: (id: string) => Promise<void>;
  onDismiss: (id: string) => void;
  onRename?: (id: string, name: string) => void;
  subagentsBySession: Map<string, SubagentStatus[]>;
}
```

Add `subagentsBySession` to the destructured function parameters (line 27 of `ProjectGroup.tsx`):

```tsx
export function ProjectGroup({
  projectName,
  sessions,
  activeSessionId,
  isCollapsed,
  onToggleCollapse,
  onSessionClick,
  onClose,
  onDismiss,
  onRename,
  subagentsBySession,
}: ProjectGroupProps) {
```

Update the sessions render block:

```tsx
      {!isCollapsed && (
        <div className={styles.sessions}>
          {sessions.map((session) => (
            <div key={session.id}>
              <SessionCard
                session={session}
                isActive={session.id === activeSessionId}
                onClick={onSessionClick}
                onClose={onClose}
                onDismiss={onDismiss}
                onRename={onRename}
              />
              <SubagentList subagents={subagentsBySession.get(session.id) ?? []} />
            </div>
          ))}
        </div>
      )}
```

- [ ] **Step 4: Update SessionPanel to pass subagents through**

In `src/components/SessionPanel/SessionPanel.tsx`, add:

```tsx
import type { SubagentStatus } from "../../types/session";
```

Add store selector:

```tsx
  const subagents = useSessionStore((s) => s.subagents);
```

Pass to ProjectGroup:

```tsx
              <ProjectGroup
                key={group.cwd}
                projectName={group.displayName}
                sessions={group.sessions}
                activeSessionId={activeSessionId}
                isCollapsed={collapsedGroups.has(group.cwd)}
                onToggleCollapse={() => toggleCollapse(group.cwd)}
                onSessionClick={onSessionClick}
                onClose={closeSession}
                onDismiss={dismissSession}
                onRename={renameSession}
                subagentsBySession={subagents}
              />
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `npx vitest run`
Expected: All PASS

- [ ] **Step 6: Commit**

```bash
git add src/components/ProjectGroup/ProjectGroup.tsx src/components/SessionPanel/SessionPanel.tsx src/components/SessionPanel/SessionPanel.test.tsx
git commit -m "feat: integrate SubagentList into sidebar rendering"
```

### Task 10: Add subagent cleanup timer logic

**Files:**
- Modify: `src/stores/sessionStore.ts`
- Modify: `src/stores/sessionStore.test.ts`

- [ ] **Step 1: Write the failing test — cleanup timer fires after 30s when parent is active**

Add to `src/stores/sessionStore.test.ts`:

```typescript
  describe("subagent cleanup", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it("removes finished subagents 30s after parent becomes active", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "session-1",
        name: "Test",
        status: "working",
        createdAt: Date.now(),
        cwd: "/test",
      });
      store.setActiveSession("session-1");
      store.updateSubagents("session-1", [
        { id: "child-1", index: 1, status: "finished", name: null },
        { id: "child-2", index: 2, status: "working", name: null },
      ]);

      // After 30s, finished subagent should be removed
      vi.advanceTimersByTime(30_000);

      const { subagents } = useSessionStore.getState();
      const list = subagents.get("session-1");
      expect(list?.length).toBe(1);
      expect(list?.[0].id).toBe("child-2");
    });

    it("does not remove finished subagents if parent is not active", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "session-1",
        name: "Test",
        status: "working",
        createdAt: Date.now(),
        cwd: "/test",
      });
      store.addSession({
        id: "session-2",
        name: "Other",
        status: "idle",
        createdAt: Date.now(),
        cwd: "/other",
      });
      store.setActiveSession("session-2"); // different session active
      store.updateSubagents("session-1", [
        { id: "child-1", index: 1, status: "finished", name: null },
      ]);

      vi.advanceTimersByTime(60_000);

      const { subagents } = useSessionStore.getState();
      expect(subagents.get("session-1")?.length).toBe(1);
    });

    it("cancels timer when switching away from parent session", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "session-1",
        name: "Test",
        status: "working",
        createdAt: Date.now(),
        cwd: "/test",
      });
      store.addSession({
        id: "session-2",
        name: "Other",
        status: "idle",
        createdAt: Date.now(),
        cwd: "/other",
      });
      store.setActiveSession("session-1");
      store.updateSubagents("session-1", [
        { id: "child-1", index: 1, status: "finished", name: null },
      ]);

      // Timer is running. Switch away before it fires.
      store.setActiveSession("session-2");
      vi.advanceTimersByTime(60_000);

      // Subagent should still be present
      const { subagents } = useSessionStore.getState();
      expect(subagents.get("session-1")?.length).toBe(1);
    });

    it("starts timer when switching to parent session with finished subagents", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "session-1",
        name: "Test",
        status: "working",
        createdAt: Date.now(),
        cwd: "/test",
      });
      store.addSession({
        id: "session-2",
        name: "Other",
        status: "idle",
        createdAt: Date.now(),
        cwd: "/other",
      });
      store.setActiveSession("session-2");
      store.updateSubagents("session-1", [
        { id: "child-1", index: 1, status: "finished", name: null },
      ]);

      // Switch to parent session
      store.setActiveSession("session-1");
      vi.advanceTimersByTime(30_000);

      const { subagents } = useSessionStore.getState();
      expect(subagents.has("session-1")).toBe(false);
    });
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/stores/sessionStore.test.ts`
Expected: FAIL — cleanup doesn't happen

- [ ] **Step 3: Implement cleanup timer logic**

Add a module-level `Map` to track cleanup timers (alongside `eventCleanups`):

```typescript
const subagentCleanupTimers = new Map<string, ReturnType<typeof setTimeout>>();
```

Add a helper function outside the store:

```typescript
function scheduleSubagentCleanup(sessionId: string) {
  // Cancel any existing timer for this session
  const existing = subagentCleanupTimers.get(sessionId);
  if (existing) clearTimeout(existing);

  const timer = setTimeout(() => {
    subagentCleanupTimers.delete(sessionId);
    const state = useSessionStore.getState();
    const list = state.subagents.get(sessionId);
    if (!list) return;

    // Only clean up if this session is still active
    if (state.activeSessionId !== sessionId) return;

    const remaining = list.filter((a) => a.status !== "finished");
    state.updateSubagents(sessionId, remaining);
  }, 30_000);

  subagentCleanupTimers.set(sessionId, timer);
}

function cancelSubagentCleanup(sessionId: string) {
  const timer = subagentCleanupTimers.get(sessionId);
  if (timer) {
    clearTimeout(timer);
    subagentCleanupTimers.delete(sessionId);
  }
}
```

In `updateSubagents`, after setting the state, check if cleanup should be scheduled:

```typescript
  updateSubagents: (sessionId, subagentList) =>
    set((state) => {
      const next = new Map(state.subagents);
      if (subagentList.length === 0) {
        next.delete(sessionId);
        cancelSubagentCleanup(sessionId);
      } else {
        next.set(sessionId, subagentList);
        // Schedule cleanup if parent is active and any subagent is finished
        if (state.activeSessionId === sessionId && subagentList.some((a) => a.status === "finished")) {
          scheduleSubagentCleanup(sessionId);
        }
      }
      return { subagents: next };
    }),
```

Replace the existing `setActiveSession` one-liner (`(id) => set({ activeSessionId: id })` at line 93 of sessionStore.ts) with a multi-statement function that manages cleanup timers:

```typescript
  setActiveSession: (id) => {
    // Cancel cleanup for previous active session
    const prevActive = useSessionStore.getState().activeSessionId;
    if (prevActive) cancelSubagentCleanup(prevActive);

    set({ activeSessionId: id });

    // Schedule cleanup for new active session if it has finished subagents
    const subagents = useSessionStore.getState().subagents.get(id);
    if (subagents?.some((a) => a.status === "finished")) {
      scheduleSubagentCleanup(id);
    }
  },
```

Also cancel cleanup timers in `removeSession` and `dismissSession`. Add `cancelSubagentCleanup(id)` as the first line inside each method, before the `set()` call. For example, in `removeSession`:

```typescript
  removeSession: (id) => {
    cancelSubagentCleanup(id);
    set((state) => {
      // ... existing cleanup logic with subagent additions ...
    });
  },
```

And identically for `dismissSession` — add `cancelSubagentCleanup(id)` before the `set()` call. Note: this requires converting both methods from arrow-expression form (`(id) => set(...)`) to arrow-block form (`(id) => { ...; set(...); }`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `npx vitest run src/stores/sessionStore.test.ts`
Expected: All PASS

- [ ] **Step 5: Run all frontend tests**

Run: `npx vitest run`
Expected: All PASS

- [ ] **Step 6: Commit**

```bash
git add src/stores/sessionStore.ts src/stores/sessionStore.test.ts
git commit -m "feat: add 30-second cleanup timer for finished subagents"
```

### Task 11: Final integration test — build and verify

**Files:** None (verification only)

- [ ] **Step 1: Run all Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All PASS

- [ ] **Step 2: Run all frontend tests**

Run: `npx vitest run`
Expected: All PASS

- [ ] **Step 3: Verify TypeScript compiles**

Run: `npx tsc --noEmit`
Expected: No errors

- [ ] **Step 4: Verify the app builds**

Run: `npm run tauri build`
Expected: Build succeeds

- [ ] **Step 5: Commit any remaining fixes**

If any compilation or test fixes were needed, commit them.
