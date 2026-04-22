//! HTTP server for receiving Claude Code Notification hook events.
//!
//! Listens on 127.0.0.1:0 (OS-assigned port). Claude Code's hook shell
//! script POSTs JSON to `POST /status/{ao_session_id}` and this module
//! updates the corresponding [`StatusTracker`] in the shared map.

use crate::pty_manager::StatusCallback;
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
    ) -> (Self, u16) {
        let server =
            tiny_http::Server::http("127.0.0.1:0").expect("failed to bind status HTTP server");
        let port = server.server_addr().to_ip().expect("expected IP address").port();
        let server = Arc::new(server);

        let server_clone = server.clone();
        thread::Builder::new()
            .name("status-server".into())
            .spawn(move || {
                accept_loop(server_clone, trackers, on_status);
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
) {
    for request in server.incoming_requests() {
        handle_request(request, &trackers, &on_status);
    }
}

fn handle_request(
    mut request: tiny_http::Request,
    trackers: &Arc<Mutex<HashMap<String, StatusTracker>>>,
    on_status: &Arc<StatusCallback>,
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

    // Extract notification_type.
    let notification_type = match json.get("notification_type").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        None => {
            let _ = request.respond(tiny_http::Response::empty(400));
            return;
        }
    };

    // Extract session_id from body (for the callback).
    let session_id = json
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or(&ao_session_id)
        .to_string();

    // Look up tracker and apply the event.
    let transition = {
        let mut map = trackers.lock().unwrap();
        match map.get_mut(&ao_session_id) {
            Some(tracker) => tracker.notify_hook_event(&notification_type),
            None => {
                drop(map);
                let _ = request.respond(tiny_http::Response::empty(404));
                return;
            }
        }
    };

    match transition {
        Some(new_status) => {
            on_status(session_id, new_status.as_str().to_string());
            let _ = request.respond(tiny_http::Response::empty(200));
        }
        None => {
            let _ = request.respond(tiny_http::Response::empty(204));
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
        let (_server, port) = StatusServer::start(trackers, noop_callback());
        assert!(port > 0, "port should be non-zero");
    }

    #[test]
    fn test_post_valid_json_returns_200_on_transition() {
        let trackers = make_trackers();
        // Insert a tracker starting in Starting state; idle_prompt -> Idle.
        trackers.lock().unwrap().insert("sess1".into(), StatusTracker::new());

        let (server, port) = StatusServer::start(trackers, noop_callback());

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

        let (server, port) = StatusServer::start(trackers, noop_callback());

        let body = r#"{"session_id":"cc-sess-2","notification_type":"idle_prompt"}"#;
        let line = post(port, "/status/sess2", body);
        assert_eq!(status_code(&line), 204, "expected 204, got: {line}");

        server.stop();
    }

    #[test]
    fn test_post_unknown_session_id_returns_404() {
        let trackers = make_trackers();
        let (server, port) = StatusServer::start(trackers, noop_callback());

        let body = r#"{"session_id":"x","notification_type":"idle_prompt"}"#;
        let line = post(port, "/status/unknown-session", body);
        assert_eq!(status_code(&line), 404, "expected 404, got: {line}");

        server.stop();
    }

    #[test]
    fn test_post_malformed_json_returns_400() {
        let trackers = make_trackers();
        trackers.lock().unwrap().insert("sess3".into(), StatusTracker::new());

        let (server, port) = StatusServer::start(trackers, noop_callback());

        let line = post(port, "/status/sess3", "not-json{{");
        assert_eq!(status_code(&line), 400, "expected 400, got: {line}");

        server.stop();
    }

    #[test]
    fn test_post_missing_notification_type_returns_400() {
        let trackers = make_trackers();
        trackers.lock().unwrap().insert("sess4".into(), StatusTracker::new());

        let (server, port) = StatusServer::start(trackers, noop_callback());

        let body = r#"{"session_id":"x","message":"no type here"}"#;
        let line = post(port, "/status/sess4", body);
        assert_eq!(status_code(&line), 400, "expected 400, got: {line}");

        server.stop();
    }

    #[test]
    fn test_get_request_returns_405() {
        let trackers = make_trackers();
        let (server, port) = StatusServer::start(trackers, noop_callback());

        let line = get(port, "/status/any-session");
        assert_eq!(status_code(&line), 405, "expected 405, got: {line}");

        server.stop();
    }

    #[test]
    fn test_stop_works_cleanly() {
        let trackers = make_trackers();
        let (server, _port) = StatusServer::start(trackers, noop_callback());
        // Just verifying stop() doesn't panic.
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

        let (server, port) = StatusServer::start(trackers, cb);

        let body = r#"{"session_id":"cc-5","notification_type":"idle_prompt"}"#;
        post(port, "/status/sess5", body);

        assert!(called.load(Ordering::SeqCst), "on_status callback should have been called");

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
}
