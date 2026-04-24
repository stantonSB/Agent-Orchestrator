# Subagent Descriptive Names Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show descriptive names (derived from the hook `prompt` field) for subagents in the session panel instead of the generic `agent_type`.

**Architecture:** Backend-only change. Extract the `prompt` field from SubagentStart hook JSON in `status_server.rs`, derive a short display name (first sentence, max 40 chars), store it in `SubagentInfo`, and use it for the `name` field in `SubagentStatusPayload`. No frontend changes needed.

**Tech Stack:** Rust (Tauri backend), Cargo test

**Spec:** `docs/superpowers/specs/2026-04-24-subagent-descriptive-names-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src-tauri/src/subagent_tracker.rs` | Modify | Add `display_name` field, update `process_start()` signature, update `From` impl |
| `src-tauri/src/status_server.rs` | Modify | Extract `prompt` from JSON, derive display name, pass to `process_start()` |

---

## Chunk 1: Implementation

### Task 1: Add `display_name` to `SubagentInfo` and update `process_start()`

**Files:**
- Modify: `src-tauri/src/subagent_tracker.rs:6-13` (SubagentInfo struct)
- Modify: `src-tauri/src/subagent_tracker.rs:25-34` (From impl)
- Modify: `src-tauri/src/subagent_tracker.rs:74-91` (process_start)
- Modify: `src-tauri/src/subagent_tracker.rs:128-231` (tests)

- [ ] **Step 1: Write failing tests for `display_name` in `process_start`**

Add these tests to the `mod tests` block in `subagent_tracker.rs`:

```rust
#[test]
fn test_process_start_with_display_name() {
    let mut map = SubagentMap::new();
    map.process_start("general-purpose", Some("Review plan chunk 1".to_string()));
    assert_eq!(map.subagents()[0].agent_type, "general-purpose");
    assert_eq!(map.subagents()[0].display_name, Some("Review plan chunk 1".to_string()));
}

#[test]
fn test_process_start_without_display_name() {
    let mut map = SubagentMap::new();
    map.process_start("code-reviewer", None);
    assert_eq!(map.subagents()[0].display_name, None);
}

#[test]
fn test_payload_uses_display_name_over_agent_type() {
    let mut map = SubagentMap::new();
    map.process_start("general-purpose", Some("Review plan chunk 1".to_string()));
    let payload = map.payload();
    assert_eq!(payload[0].name, Some("Review plan chunk 1".to_string()));
}

#[test]
fn test_payload_falls_back_to_agent_type() {
    let mut map = SubagentMap::new();
    map.process_start("code-reviewer", None);
    let payload = map.payload();
    assert_eq!(payload[0].name, Some("code-reviewer".to_string()));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test subagent_tracker -- --nocapture 2>&1 | head -30`
Expected: Compilation errors — `process_start` doesn't accept a second argument, `display_name` field doesn't exist.

- [ ] **Step 3: Add `display_name` field to `SubagentInfo`**

In `subagent_tracker.rs`, add the field to the struct (after `agent_type`):

```rust
pub struct SubagentInfo {
    pub id: String,
    pub index: u16,
    pub status: SessionStatus,
    pub agent_type: String,
    pub display_name: Option<String>,
    pub created_at: u64,
    pub finished_at: Option<Instant>,
}
```

- [ ] **Step 4: Update `process_start()` to accept and store `display_name`**

Change the signature and body:

```rust
pub fn process_start(&mut self, agent_type: &str, display_name: Option<String>) -> bool {
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
        agent_type: agent_type.to_string(),
        display_name,
        created_at,
        finished_at: None,
    });
    true
}
```

- [ ] **Step 5: Update `SubagentStatusPayload::from()` to prefer `display_name`**

```rust
impl From<&SubagentInfo> for SubagentStatusPayload {
    fn from(info: &SubagentInfo) -> Self {
        Self {
            id: info.id.clone(),
            index: info.index,
            status: info.status.clone(),
            name: Some(info.display_name.clone().unwrap_or_else(|| info.agent_type.clone())),
            created_at: info.created_at,
        }
    }
}
```

- [ ] **Step 6: Fix existing tests — add `None` as second arg to all `process_start` calls**

Every existing `process_start("...")` call in the test module needs to become `process_start("...", None)`. There are 14 call sites in the existing tests. Update all of them.

- [ ] **Step 7: Run tests to verify they pass**

Run: `cd src-tauri && cargo test subagent_tracker -- --nocapture`
Expected: All tests pass (existing + new).

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/subagent_tracker.rs
git commit -m "feat: add display_name to SubagentInfo for descriptive subagent names"
```

---

### Task 2: Extract `prompt` from hook JSON, derive display name, pass to tracker

**Files:**
- Modify: `src-tauri/src/status_server.rs:111-143` (handle_request extraction + subagent_start branch)
- Modify: `src-tauri/src/status_server.rs:429-491` (tests)

- [ ] **Step 1: Write failing test for prompt-based display name**

Add this test to the `mod tests` block in `status_server.rs`:

```rust
#[test]
fn test_subagent_start_extracts_display_name_from_prompt() {
    let trackers = make_trackers();
    trackers.lock().unwrap().insert("ao-sess".into(), StatusTracker::new());

    let (server, port) = StatusServer::start(trackers.clone(), noop_callback(), noop_subagent_callback());

    let body = r#"{"session_id":"cc-parent","hook_event_name":"SubagentStart","agent_type":"general-purpose","prompt":"Review plan chunk 1 of the implementation"}"#;
    post(port, "/status/ao-sess", body);

    let map = trackers.lock().unwrap();
    let tracker = map.get("ao-sess").unwrap();
    let payload = tracker.subagent_map().payload();
    assert_eq!(payload[0].name, Some("Review plan chunk 1 of the implementatio...".to_string()));

    server.stop();
}

#[test]
fn test_subagent_start_prompt_sentence_split() {
    let trackers = make_trackers();
    trackers.lock().unwrap().insert("ao-sess".into(), StatusTracker::new());

    let (server, port) = StatusServer::start(trackers.clone(), noop_callback(), noop_subagent_callback());

    let body = r#"{"session_id":"cc-parent","hook_event_name":"SubagentStart","agent_type":"general-purpose","prompt":"Check auth module. Be thorough about edge cases."}"#;
    post(port, "/status/ao-sess", body);

    let map = trackers.lock().unwrap();
    let tracker = map.get("ao-sess").unwrap();
    let payload = tracker.subagent_map().payload();
    assert_eq!(payload[0].name, Some("Check auth module".to_string()));

    server.stop();
}

#[test]
fn test_subagent_start_no_prompt_falls_back_to_agent_type() {
    let trackers = make_trackers();
    trackers.lock().unwrap().insert("ao-sess".into(), StatusTracker::new());

    let (server, port) = StatusServer::start(trackers.clone(), noop_callback(), noop_subagent_callback());

    let body = r#"{"session_id":"cc-parent","hook_event_name":"SubagentStart","agent_type":"code-reviewer"}"#;
    post(port, "/status/ao-sess", body);

    let map = trackers.lock().unwrap();
    let tracker = map.get("ao-sess").unwrap();
    let payload = tracker.subagent_map().payload();
    assert_eq!(payload[0].name, Some("code-reviewer".to_string()));

    server.stop();
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test status_server -- --nocapture 2>&1 | head -30`
Expected: Compilation error — `process_start` now requires two arguments but the call site in `status_server.rs` still passes one.

- [ ] **Step 3: Add `derive_display_name` helper function**

Add this function above `handle_request` in `status_server.rs` (or at module level):

```rust
/// Derive a short display name from a subagent's prompt.
/// Takes the first sentence (up to first '.' or '\n'), trims whitespace,
/// and truncates to 40 chars at a char boundary.
fn derive_display_name(prompt: &str) -> Option<String> {
    let first_sentence = prompt
        .split_once('.')
        .map(|(s, _)| s)
        .unwrap_or(prompt);
    let first_sentence = first_sentence
        .split_once('\n')
        .map(|(s, _)| s)
        .unwrap_or(first_sentence);
    let trimmed = first_sentence.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().count() > 40 {
        let truncated: String = trimmed.chars().take(40).collect();
        Some(format!("{}...", truncated.trim_end()))
    } else {
        Some(trimmed.to_string())
    }
}
```

- [ ] **Step 4: Extract `prompt` and pass derived name to `process_start`**

In `handle_request`, after the existing `agent_type` extraction (line ~112), add:

```rust
let prompt = json.get("prompt").and_then(|v| v.as_str()).map(|s| s.to_string());
```

Then update the `subagent_start` branch (line ~143) from:

```rust
"subagent_start" => submap.process_start(type_name),
```

to:

```rust
"subagent_start" => {
    let display_name = prompt.as_deref().and_then(derive_display_name);
    submap.process_start(type_name, display_name)
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test -- --nocapture`
Expected: All tests pass (both `subagent_tracker` and `status_server` modules).

- [ ] **Step 6: Add unit tests for `derive_display_name` edge cases**

Add these tests to `status_server.rs` test module:

```rust
#[test]
fn test_derive_display_name_basic() {
    assert_eq!(derive_display_name("Find config files"), Some("Find config files".to_string()));
}

#[test]
fn test_derive_display_name_period_split() {
    assert_eq!(
        derive_display_name("Check auth module. Be thorough."),
        Some("Check auth module".to_string())
    );
}

#[test]
fn test_derive_display_name_newline_split() {
    assert_eq!(
        derive_display_name("Fix the bug\nAlso check tests"),
        Some("Fix the bug".to_string())
    );
}

#[test]
fn test_derive_display_name_truncation() {
    let long = "Review plan chunk 1 of the implementation that covers auth and routing";
    let result = derive_display_name(long).unwrap();
    assert!(result.ends_with("..."));
    // 40 chars + "..." = 43 total
    assert!(result.chars().count() <= 43);
}

#[test]
fn test_derive_display_name_empty() {
    assert_eq!(derive_display_name(""), None);
    assert_eq!(derive_display_name("   "), None);
}

#[test]
fn test_derive_display_name_period_before_newline() {
    assert_eq!(
        derive_display_name("Fix auth.rs\nAlso check tests"),
        Some("Fix auth".to_string())
    );
}
```

- [ ] **Step 7: Run all tests**

Run: `cd src-tauri && cargo test -- --nocapture`
Expected: All tests pass.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/status_server.rs
git commit -m "feat: derive subagent display names from hook prompt field"
```
