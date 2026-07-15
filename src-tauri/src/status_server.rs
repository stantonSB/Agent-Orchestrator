//! HTTP server for receiving Claude Code Notification hook events.
//!
//! Listens on 127.0.0.1:0 (OS-assigned port). Claude Code's hook shell
//! script POSTs JSON to `POST /status/{ao_session_id}` and this module
//! updates the corresponding [`StatusTracker`] in the shared map.

use crate::pty_manager::{StatusCallback, SubagentCallback, WorktreeCwdCallback};
use crate::status_parser::StatusTracker;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

pub struct StatusServer {
    server: Arc<tiny_http::Server>,
    port: u16,
}

impl StatusServer {
    /// Start the HTTP server on 127.0.0.1:0 (OS-assigned port).
    /// Returns the server handle and the assigned port number.
    pub fn start(
        trackers: Arc<Mutex<HashMap<String, StatusTracker>>>,
        on_status: Arc<StatusCallback>,
        on_subagents: Arc<SubagentCallback>,
        on_worktree_cwd: Arc<WorktreeCwdCallback>,
    ) -> (Self, u16) {
        let server =
            tiny_http::Server::http("127.0.0.1:0").expect("failed to bind status HTTP server");
        let port = server.server_addr().to_ip().expect("expected IP address").port();
        let server = Arc::new(server);

        let server_clone = server.clone();
        thread::Builder::new()
            .name("status-server".into())
            .spawn(move || {
                accept_loop(server_clone, trackers, on_status, on_subagents, on_worktree_cwd);
            })
            .expect("failed to spawn status server thread");

        let this = StatusServer { server, port };
        (this, port)
    }

    /// Get the port number the server is listening on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Stop the server (for clean shutdown).
    pub fn stop(&self) {
        self.server.unblock();
    }
}

// ---------------------------------------------------------------------------
// Accept loop
// ---------------------------------------------------------------------------

fn accept_loop(
    server: Arc<tiny_http::Server>,
    trackers: Arc<Mutex<HashMap<String, StatusTracker>>>,
    on_status: Arc<StatusCallback>,
    on_subagents: Arc<SubagentCallback>,
    on_worktree_cwd: Arc<WorktreeCwdCallback>,
) {
    for request in server.incoming_requests() {
        handle_request(request, &trackers, &on_status, &on_subagents, &on_worktree_cwd);
    }
}

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

fn handle_request(
    mut request: tiny_http::Request,
    trackers: &Arc<Mutex<HashMap<String, StatusTracker>>>,
    on_status: &Arc<StatusCallback>,
    on_subagents: &Arc<SubagentCallback>,
    on_worktree_cwd: &Arc<WorktreeCwdCallback>,
) {
    // Only allow POST.
    if *request.method() != tiny_http::Method::Post {
        let _ = request.respond(tiny_http::Response::empty(405));
        return;
    }

    // Parse URL: /status/{ao_session_id}
    let path = request.url().to_string();
    let ao_session_id = match parse_session_id(&path) {
        Some(id) => id.to_string(),
        None => {
            let _ = request.respond(tiny_http::Response::empty(404));
            return;
        }
    };

    // Read body.
    let mut body = String::new();
    if std::io::Read::read_to_string(request.as_reader(), &mut body).is_err() {
        let _ = request.respond(tiny_http::Response::empty(400));
        return;
    }

    // Parse JSON.
    let json: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            let _ = request.respond(tiny_http::Response::empty(400));
            return;
        }
    };

    // Extract X-Cwd header if present.
    let x_cwd: Option<String> = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("X-Cwd"))
        .map(|h| h.value.to_string());

    // Extract subagent/teammate fields if present.
    let agent_type = json.get("agent_type").and_then(|v| v.as_str()).map(|s| s.to_string());
    let agent_id = json.get("agent_id").and_then(|v| v.as_str()).map(|s| s.to_string());
    let prompt = json.get("prompt").and_then(|v| v.as_str()).map(|s| s.to_string());

    // Extract the event type.
    let hook_event_name = json.get("hook_event_name").and_then(|v| v.as_str());
    let notification_type = if let Some(t) = json.get("notification_type").and_then(|v| v.as_str()) {
        t.to_string()
    } else if hook_event_name == Some("Stop") {
        "stop".to_string()
    } else if hook_event_name == Some("SubagentStop") {
        "subagent_stop".to_string()
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

    // Look up tracker and apply the event.
    let mut worktree_cwd_to_emit: Option<(String, String)> = None;
    let (transition, subagent_changed) = {
        let mut map = trackers.lock().unwrap();
        match map.get_mut(&ao_session_id) {
            Some(tracker) => {
                // Check for worktree cwd
                if let Some(ref cwd) = x_cwd {
                    if cwd.contains(".claude/worktrees/") {
                        if tracker.set_worktree_cwd(cwd) {
                            worktree_cwd_to_emit = Some((ao_session_id.clone(), cwd.clone()));
                        }
                    }
                }

                let mut subagent_changed = false;
                let is_start = notification_type == "subagent_start"
                    || notification_type == "task_created";
                let is_stop = notification_type == "subagent_stop"
                    || notification_type == "teammate_idle";
                let is_subagent_event = is_start || is_stop;

                // Handle subagent/teammate lifecycle events
                if is_subagent_event {
                    let type_name = agent_type.as_deref().unwrap_or("unknown");
                    let aid = agent_id.as_deref();
                    let submap = tracker.subagent_map_mut();
                    subagent_changed = if is_start {
                        // Teammate appearance (task_created) labels by agent_type;
                        // only classic subagents derive a display name from the task prompt.
                        let display_name = if notification_type == "subagent_start" {
                            prompt.as_deref().and_then(derive_display_name)
                        } else {
                            None
                        };
                        submap.process_start(aid, type_name, display_name)
                    } else if notification_type == "teammate_idle" && aid.is_none() {
                        // A TeammateIdle we cannot attribute to a specific teammate must
                        // not finish an arbitrary Working agent (could be unrelated).
                        false
                    } else {
                        submap.process_stop(aid, type_name)
                    };
                }

                // Process parent status transitions for non-subagent events
                let transition = if !is_subagent_event {
                    tracker.notify_hook_event(&notification_type)
                } else {
                    // Subagent event — check if bubbling needed
                    if subagent_changed && tracker.subagent_map().any_needs_attention() {
                        match tracker.status() {
                            crate::status_parser::SessionStatus::Working | crate::status_parser::SessionStatus::Idle => {
                                Some(crate::status_parser::SessionStatus::NeedsAttention)
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

    // Emit worktree cwd if detected
    if let Some((session_id, cwd)) = worktree_cwd_to_emit {
        on_worktree_cwd(session_id, cwd);
    }

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
}

/// Extract `{ao_session_id}` from a path of the form `/status/{ao_session_id}`.
/// Returns `None` for any other path shape.
fn parse_session_id(path: &str) -> Option<&str> {
    let path = path.trim_end_matches('/');
    let rest = path.strip_prefix("/status/")?;
    if rest.is_empty() || rest.contains('/') {
        return None;
    }
    Some(rest)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    fn make_trackers() -> Arc<Mutex<HashMap<String, StatusTracker>>> {
        Arc::new(Mutex::new(HashMap::new()))
    }

    fn noop_callback() -> Arc<StatusCallback> {
        Arc::new(Box::new(|_id: String, _status: String| {}))
    }

    fn noop_subagent_callback() -> Arc<SubagentCallback> {
        Arc::new(Box::new(|_id: String, _payload: Vec<crate::subagent_tracker::SubagentStatusPayload>| {}))
    }

    fn noop_worktree_callback() -> Arc<crate::pty_manager::WorktreeCwdCallback> {
        Arc::new(Box::new(|_id: String, _cwd: String| {}))
    }

    /// Send a raw HTTP request over a TcpStream and return the status line.
    fn raw_http(port: u16, request: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect failed");
        stream.write_all(request.as_bytes()).expect("write failed");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read failed");
        // Return just the first line (status line).
        response.lines().next().unwrap_or("").to_string()
    }

    fn post(port: u16, path: &str, body: &str) -> String {
        let request = format!(
            "POST {} HTTP/1.0\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
            path,
            body.len(),
            body
        );
        raw_http(port, &request)
    }

    fn get(port: u16, path: &str) -> String {
        let request = format!("GET {} HTTP/1.0\r\n\r\n", path);
        raw_http(port, &request)
    }

    fn status_code(line: &str) -> u16 {
        line.split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }

    #[test]
    fn test_server_starts_with_valid_port() {
        let trackers = make_trackers();
        let (_server, port) = StatusServer::start(trackers, noop_callback(), noop_subagent_callback(), noop_worktree_callback());
        assert!(port > 0, "port should be non-zero");
    }

    #[test]
    fn test_post_valid_json_returns_200_on_transition() {
        let trackers = make_trackers();
        // Insert a tracker starting in Starting state; idle_prompt -> Idle.
        trackers.lock().unwrap().insert("sess1".into(), StatusTracker::new());

        let (server, port) = StatusServer::start(trackers, noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        let body = r#"{"session_id":"cc-sess-1","notification_type":"idle_prompt"}"#;
        let line = post(port, "/status/sess1", body);
        assert_eq!(status_code(&line), 200, "expected 200, got: {line}");

        server.stop();
    }

    #[test]
    fn test_post_valid_json_returns_204_when_no_transition() {
        let trackers = make_trackers();
        let mut tracker = StatusTracker::new();
        // Put it in Idle state. Another idle_prompt from Idle yields no transition.
        tracker.notify_hook_event("idle_prompt"); // Starting -> Idle
        trackers.lock().unwrap().insert("sess2".into(), tracker);

        let (server, port) = StatusServer::start(trackers, noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        let body = r#"{"session_id":"cc-sess-2","notification_type":"idle_prompt"}"#;
        let line = post(port, "/status/sess2", body);
        assert_eq!(status_code(&line), 204, "expected 204, got: {line}");

        server.stop();
    }

    #[test]
    fn test_post_unknown_session_id_returns_404() {
        let trackers = make_trackers();
        let (server, port) = StatusServer::start(trackers, noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        let body = r#"{"session_id":"x","notification_type":"idle_prompt"}"#;
        let line = post(port, "/status/unknown-session", body);
        assert_eq!(status_code(&line), 404, "expected 404, got: {line}");

        server.stop();
    }

    #[test]
    fn test_post_malformed_json_returns_400() {
        let trackers = make_trackers();
        trackers.lock().unwrap().insert("sess3".into(), StatusTracker::new());

        let (server, port) = StatusServer::start(trackers, noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        let line = post(port, "/status/sess3", "not-json{{");
        assert_eq!(status_code(&line), 400, "expected 400, got: {line}");

        server.stop();
    }

    #[test]
    fn test_post_missing_notification_type_returns_400() {
        let trackers = make_trackers();
        trackers.lock().unwrap().insert("sess4".into(), StatusTracker::new());

        let (server, port) = StatusServer::start(trackers, noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        let body = r#"{"session_id":"x","message":"no type here"}"#;
        let line = post(port, "/status/sess4", body);
        assert_eq!(status_code(&line), 400, "expected 400, got: {line}");

        server.stop();
    }

    #[test]
    fn test_get_request_returns_405() {
        let trackers = make_trackers();
        let (server, port) = StatusServer::start(trackers, noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        let line = get(port, "/status/any-session");
        assert_eq!(status_code(&line), 405, "expected 405, got: {line}");

        server.stop();
    }

    #[test]
    fn test_stop_works_cleanly() {
        let trackers = make_trackers();
        let (server, _port) = StatusServer::start(trackers, noop_callback(), noop_subagent_callback(), noop_worktree_callback());
        // Just verifying stop() doesn't panic.
        server.stop();
    }

    #[test]
    fn test_stop_hook_event_returns_200_on_transition() {
        let trackers = make_trackers();
        // Start in Working state so stop -> Finished is a valid transition.
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting -> Idle
        tracker.notify_user_input(b"task\r"); // Idle -> Working
        trackers.lock().unwrap().insert("sess-stop".into(), tracker);

        let (server, port) = StatusServer::start(trackers, noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        // Stop hook sends hook_event_name instead of notification_type
        let body = r#"{"session_id":"cc-1","hook_event_name":"Stop","cwd":"/tmp"}"#;
        let line = post(port, "/status/sess-stop", body);
        assert_eq!(status_code(&line), 200, "expected 200, got: {line}");

        server.stop();
    }

    #[test]
    fn test_stop_hook_from_starting_returns_200() {
        let trackers = make_trackers();
        trackers.lock().unwrap().insert("sess-stop2".into(), StatusTracker::new());

        let (server, port) = StatusServer::start(trackers, noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        let body = r#"{"session_id":"cc-2","hook_event_name":"Stop"}"#;
        let line = post(port, "/status/sess-stop2", body);
        assert_eq!(status_code(&line), 200, "expected 200, got: {line}");

        server.stop();
    }

    #[test]
    fn test_on_status_callback_called_on_transition() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let trackers = make_trackers();
        trackers.lock().unwrap().insert("sess5".into(), StatusTracker::new());

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let cb: Arc<StatusCallback> =
            Arc::new(Box::new(move |_id: String, status: String| {
                if status == "idle" {
                    called_clone.store(true, Ordering::SeqCst);
                }
            }));

        let (server, port) = StatusServer::start(trackers, cb, noop_subagent_callback(), noop_worktree_callback());

        let body = r#"{"session_id":"cc-5","notification_type":"idle_prompt"}"#;
        post(port, "/status/sess5", body);

        assert!(called.load(Ordering::SeqCst), "on_status callback should have been called");

        server.stop();
    }

    #[test]
    fn test_subagent_start_registers_subagent() {
        let trackers = make_trackers();
        trackers.lock().unwrap().insert("ao-sess".into(), StatusTracker::new());

        let (server, port) = StatusServer::start(trackers.clone(), noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        // SubagentStart event with agent_type
        let body = r#"{"session_id":"cc-parent","hook_event_name":"SubagentStart","agent_type":"code-reviewer"}"#;
        let line = post(port, "/status/ao-sess", body);
        assert_eq!(status_code(&line), 200);

        // Verify subagent was registered
        let map = trackers.lock().unwrap();
        let tracker = map.get("ao-sess").unwrap();
        assert_eq!(tracker.subagent_map().subagents().len(), 1);

        server.stop();
    }

    #[test]
    fn test_subagent_stop_marks_finished() {
        let trackers = make_trackers();
        trackers.lock().unwrap().insert("ao-sess".into(), StatusTracker::new());

        let (server, port) = StatusServer::start(trackers.clone(), noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        // Start then stop
        let body = r#"{"session_id":"cc-parent","hook_event_name":"SubagentStart","agent_type":"code-reviewer"}"#;
        post(port, "/status/ao-sess", body);

        let body = r#"{"session_id":"cc-parent","hook_event_name":"SubagentStop","agent_type":"code-reviewer"}"#;
        let line = post(port, "/status/ao-sess", body);
        assert_eq!(status_code(&line), 200);

        let map = trackers.lock().unwrap();
        let tracker = map.get("ao-sess").unwrap();
        let subagents = tracker.subagent_map().subagents();
        assert_eq!(subagents.len(), 1);
        assert_eq!(subagents[0].status, crate::status_parser::SessionStatus::Finished);

        server.stop();
    }

    #[test]
    fn test_multiple_subagents_tracked() {
        let trackers = make_trackers();
        trackers.lock().unwrap().insert("ao-sess".into(), StatusTracker::new());

        let (server, port) = StatusServer::start(trackers.clone(), noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        // Start 3 subagents (all share parent's session_id, as Claude Code does)
        for _ in 0..3 {
            let body = r#"{"session_id":"cc-parent","hook_event_name":"SubagentStart","agent_type":"code-reviewer"}"#;
            post(port, "/status/ao-sess", body);
        }

        let map = trackers.lock().unwrap();
        let tracker = map.get("ao-sess").unwrap();
        assert_eq!(tracker.subagent_map().subagents().len(), 3);

        server.stop();
    }

    #[test]
    fn test_parse_session_id() {
        assert_eq!(parse_session_id("/status/abc-123"), Some("abc-123"));
        assert_eq!(parse_session_id("/status/abc-123/"), Some("abc-123"));
        assert_eq!(parse_session_id("/status/"), None);
        assert_eq!(parse_session_id("/other/abc"), None);
        assert_eq!(parse_session_id("/status/a/b"), None);
    }

    #[test]
    fn test_subagent_start_extracts_display_name_from_prompt() {
        let trackers = make_trackers();
        trackers.lock().unwrap().insert("ao-sess".into(), StatusTracker::new());

        let (server, port) = StatusServer::start(trackers.clone(), noop_callback(), noop_subagent_callback(), noop_worktree_callback());

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

        let (server, port) = StatusServer::start(trackers.clone(), noop_callback(), noop_subagent_callback(), noop_worktree_callback());

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

        let (server, port) = StatusServer::start(trackers.clone(), noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        let body = r#"{"session_id":"cc-parent","hook_event_name":"SubagentStart","agent_type":"code-reviewer"}"#;
        post(port, "/status/ao-sess", body);

        let map = trackers.lock().unwrap();
        let tracker = map.get("ao-sess").unwrap();
        let payload = tracker.subagent_map().payload();
        assert_eq!(payload[0].name, Some("code-reviewer".to_string()));

        server.stop();
    }

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

    #[test]
    fn test_pre_tool_use_returns_204_no_transition_but_processes_x_cwd() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let trackers = make_trackers();
        trackers.lock().unwrap().insert("sess-ptu".into(), StatusTracker::new());

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let wt_cb: Arc<crate::pty_manager::WorktreeCwdCallback> =
            Arc::new(Box::new(move |_id: String, cwd: String| {
                if cwd.contains(".claude/worktrees/") {
                    called_clone.store(true, Ordering::SeqCst);
                }
            }));

        let (server, port) = StatusServer::start(
            trackers,
            noop_callback(),
            noop_subagent_callback(),
            wt_cb,
        );

        let body = r#"{"session_id":"cc-1","hook_event_name":"PreToolUse"}"#;
        let request = format!(
            "POST /status/sess-ptu HTTP/1.0\r\nContent-Length: {}\r\nContent-Type: application/json\r\nX-Cwd: /projects/app/.claude/worktrees/breezy-frog\r\n\r\n{}",
            body.len(),
            body
        );
        let line = raw_http(port, &request);
        assert_eq!(status_code(&line), 204, "PreToolUse should not cause a status transition");

        assert!(called.load(Ordering::SeqCst), "worktree cwd callback should have been called via PreToolUse");

        server.stop();
    }

    #[test]
    fn test_x_cwd_header_with_worktree_path_triggers_callback() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let trackers = make_trackers();
        trackers.lock().unwrap().insert("sess-wt".into(), StatusTracker::new());

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let wt_cb: Arc<crate::pty_manager::WorktreeCwdCallback> =
            Arc::new(Box::new(move |_id: String, cwd: String| {
                if cwd.contains(".claude/worktrees/") {
                    called_clone.store(true, Ordering::SeqCst);
                }
            }));

        let (server, port) = StatusServer::start(
            trackers,
            noop_callback(),
            noop_subagent_callback(),
            wt_cb,
        );

        let body = r#"{"session_id":"cc-1","notification_type":"idle_prompt"}"#;
        let request = format!(
            "POST /status/sess-wt HTTP/1.0\r\nContent-Length: {}\r\nContent-Type: application/json\r\nX-Cwd: /projects/app/.claude/worktrees/breezy-frog\r\n\r\n{}",
            body.len(),
            body
        );
        raw_http(port, &request);

        assert!(called.load(Ordering::SeqCst), "worktree cwd callback should have been called");

        server.stop();
    }

    #[test]
    fn test_x_cwd_header_without_worktree_path_does_not_trigger_callback() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let trackers = make_trackers();
        trackers.lock().unwrap().insert("sess-no-wt".into(), StatusTracker::new());

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let wt_cb: Arc<crate::pty_manager::WorktreeCwdCallback> =
            Arc::new(Box::new(move |_id: String, _cwd: String| {
                called_clone.store(true, Ordering::SeqCst);
            }));

        let (server, port) = StatusServer::start(
            trackers,
            noop_callback(),
            noop_subagent_callback(),
            wt_cb,
        );

        let body = r#"{"session_id":"cc-1","notification_type":"idle_prompt"}"#;
        let request = format!(
            "POST /status/sess-no-wt HTTP/1.0\r\nContent-Length: {}\r\nContent-Type: application/json\r\nX-Cwd: /projects/app\r\n\r\n{}",
            body.len(),
            body
        );
        raw_http(port, &request);

        assert!(!called.load(Ordering::SeqCst), "callback should NOT fire for non-worktree paths");

        server.stop();
    }

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

    #[test]
    fn test_task_created_labels_by_agent_type_not_prompt() {
        let trackers = make_trackers();
        trackers.lock().unwrap().insert("ao-sess".into(), StatusTracker::new());
        let (server, port) = StatusServer::start(trackers.clone(), noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        let body = r#"{"session_id":"cc-parent","hook_event_name":"TaskCreated","agent_type":"deck-impl","agent_id":"t1","prompt":"Redesign the whole slide deck end to end"}"#;
        post(port, "/status/ao-sess", body);

        let map = trackers.lock().unwrap();
        let tracker = map.get("ao-sess").unwrap();
        let payload = tracker.subagent_map().payload();
        assert_eq!(payload[0].name, Some("deck-impl".to_string()));

        server.stop();
    }

    #[test]
    fn test_teammate_idle_without_agent_id_does_not_finish_others() {
        let trackers = make_trackers();
        trackers.lock().unwrap().insert("ao-sess".into(), StatusTracker::new());
        let (server, port) = StatusServer::start(trackers.clone(), noop_callback(), noop_subagent_callback(), noop_worktree_callback());

        // A regular subagent is Working.
        let start = r#"{"session_id":"cc-parent","hook_event_name":"SubagentStart","agent_type":"code-reviewer"}"#;
        post(port, "/status/ao-sess", start);

        // A TeammateIdle with no agent_id must NOT finish the unrelated subagent.
        let idle = r#"{"session_id":"cc-parent","hook_event_name":"TeammateIdle"}"#;
        let line = post(port, "/status/ao-sess", idle);
        assert_eq!(status_code(&line), 204, "no state change expected");

        let map = trackers.lock().unwrap();
        let tracker = map.get("ao-sess").unwrap();
        let subagents = tracker.subagent_map().subagents();
        assert_eq!(subagents.len(), 1);
        assert_eq!(subagents[0].status, crate::status_parser::SessionStatus::Working);

        server.stop();
    }
}
