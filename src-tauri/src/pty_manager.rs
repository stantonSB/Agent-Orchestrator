//! PTY manager module.
//!
//! Owns all PTY state on a dedicated thread. Communicates with callers
//! via channel-based messages (PtyRequest / PtyResponse).
//!
//! Design: PTY handles from portable-pty are not Send/Sync, so they
//! cannot be shared across threads. All PTY state lives exclusively on
//! the manager thread. External code sends requests via an mpsc channel
//! and receives responses via oneshot channels.

use crate::status_parser::StatusTracker;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Unique identifier for a PTY session.
pub type SessionId = String;

/// Requests sent to the PTY manager thread.
pub enum PtyRequest {
    Create {
        name: String,
        cwd: PathBuf,
        command: String,
        args: Vec<String>,
        cols: u16,
        rows: u16,
        reply: mpsc::Sender<PtyResponse>,
    },
    Write {
        id: SessionId,
        data: Vec<u8>,
        reply: mpsc::Sender<PtyResponse>,
    },
    Resize {
        id: SessionId,
        cols: u16,
        rows: u16,
        reply: mpsc::Sender<PtyResponse>,
    },
    Rename {
        id: SessionId,
        name: String,
        reply: mpsc::Sender<PtyResponse>,
    },
    Kill {
        id: SessionId,
        reply: mpsc::Sender<PtyResponse>,
    },
    List {
        reply: mpsc::Sender<PtyResponse>,
    },
    Shutdown,
}

/// Responses from the PTY manager thread.
#[derive(Debug)]
pub enum PtyResponse {
    Created { id: SessionId },
    WriteOk,
    ResizeOk,
    RenameOk,
    Killed,
    Sessions(Vec<SessionListEntry>),
    Error(String),
}

/// Entry returned by the List request.
#[derive(Debug, Clone)]
pub struct SessionListEntry {
    pub id: SessionId,
    pub name: String,
    pub cwd: PathBuf,
    pub created_at_epoch_ms: u128,
}

pub type OutputCallback = Box<dyn Fn(SessionId, Vec<u8>) + Send + Sync + 'static>;
pub type ExitCallback = Box<dyn Fn(SessionId, Option<u32>) + Send + Sync + 'static>;
pub type StatusCallback = Box<dyn Fn(SessionId, String) + Send + Sync + 'static>;

// ---------------------------------------------------------------------------
// Internal session state (lives exclusively on the manager thread)
// ---------------------------------------------------------------------------

struct Session {
    id: SessionId,
    name: String,
    #[allow(dead_code)]
    cwd: PathBuf,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn std::io::Write + Send>,
    #[allow(dead_code)]
    created_at: Instant,
    created_at_epoch_ms: u128,
    _reader_handle: thread::JoinHandle<()>,
}

// ---------------------------------------------------------------------------
// PtyManager handle (clone-friendly, Send + Sync)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PtyManagerHandle {
    tx: mpsc::Sender<PtyRequest>,
}

impl PtyManagerHandle {
    fn request(&self, build: impl FnOnce(mpsc::Sender<PtyResponse>) -> PtyRequest) -> PtyResponse {
        let (reply_tx, reply_rx) = mpsc::channel();
        let req = build(reply_tx);
        if self.tx.send(req).is_err() {
            return PtyResponse::Error("PTY manager thread has shut down".into());
        }
        reply_rx
            .recv()
            .unwrap_or(PtyResponse::Error("PTY manager did not reply".into()))
    }

    pub fn create(
        &self,
        name: String,
        cwd: PathBuf,
        command: String,
        args: Vec<String>,
        cols: u16,
        rows: u16,
    ) -> PtyResponse {
        self.request(|reply| PtyRequest::Create {
            name,
            cwd,
            command,
            args,
            cols,
            rows,
            reply,
        })
    }

    pub fn write(&self, id: SessionId, data: Vec<u8>) -> PtyResponse {
        self.request(|reply| PtyRequest::Write { id, data, reply })
    }

    pub fn resize(&self, id: SessionId, cols: u16, rows: u16) -> PtyResponse {
        self.request(|reply| PtyRequest::Resize {
            id,
            cols,
            rows,
            reply,
        })
    }

    pub fn rename(&self, id: SessionId, name: String) -> PtyResponse {
        self.request(|reply| PtyRequest::Rename { id, name, reply })
    }

    pub fn kill(&self, id: SessionId) -> PtyResponse {
        self.request(|reply| PtyRequest::Kill { id, reply })
    }

    pub fn list(&self) -> PtyResponse {
        self.request(|reply| PtyRequest::List { reply })
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(PtyRequest::Shutdown);
    }
}

// ---------------------------------------------------------------------------
// Manager thread
// ---------------------------------------------------------------------------

pub fn start(
    on_output: OutputCallback,
    on_exit: ExitCallback,
    on_status: StatusCallback,
) -> PtyManagerHandle {
    let (tx, rx) = mpsc::channel::<PtyRequest>();

    let on_output = Arc::new(on_output);
    let on_exit = Arc::new(on_exit);
    let on_status = Arc::new(on_status);

    // Shared status trackers accessible from both manager and reader threads.
    let status_trackers: Arc<Mutex<HashMap<SessionId, StatusTracker>>> =
        Arc::new(Mutex::new(HashMap::new()));

    thread::Builder::new()
        .name("pty-manager".into())
        .spawn(move || {
            manager_loop(rx, on_output, on_exit, on_status, status_trackers);
        })
        .expect("failed to spawn PTY manager thread");

    PtyManagerHandle { tx }
}

fn manager_loop(
    rx: mpsc::Receiver<PtyRequest>,
    on_output: Arc<OutputCallback>,
    on_exit: Arc<ExitCallback>,
    on_status: Arc<StatusCallback>,
    status_trackers: Arc<Mutex<HashMap<SessionId, StatusTracker>>>,
) {
    let mut sessions: HashMap<SessionId, Session> = HashMap::new();
    let pty_system = native_pty_system();

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(request) => match request {
                PtyRequest::Create {
                    name,
                    cwd,
                    command,
                    args,
                    cols,
                    rows,
                    reply,
                } => {
                    let id = uuid::Uuid::new_v4().to_string();
                    let size = PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    };

                    let pair = match pty_system.openpty(size) {
                        Ok(pair) => pair,
                        Err(e) => {
                            let _ =
                                reply.send(PtyResponse::Error(format!("Failed to open PTY: {e}")));
                            continue;
                        }
                    };

                    let mut cmd = CommandBuilder::new(&command);
                    cmd.args(&args);
                    cmd.cwd(&cwd);

                    let child = match pair.slave.spawn_command(cmd) {
                        Ok(child) => child,
                        Err(e) => {
                            let _ = reply
                                .send(PtyResponse::Error(format!("Failed to spawn command: {e}")));
                            continue;
                        }
                    };

                    drop(pair.slave);

                    let writer = match pair.master.take_writer() {
                        Ok(w) => w,
                        Err(e) => {
                            let _ = reply
                                .send(PtyResponse::Error(format!("Failed to get PTY writer: {e}")));
                            continue;
                        }
                    };

                    let mut reader = match pair.master.try_clone_reader() {
                        Ok(r) => r,
                        Err(e) => {
                            let _ = reply
                                .send(PtyResponse::Error(format!("Failed to get PTY reader: {e}")));
                            continue;
                        }
                    };

                    // Insert a new status tracker for this session.
                    {
                        let mut trackers = status_trackers.lock().unwrap();
                        trackers.insert(id.clone(), StatusTracker::new());
                    }

                    let reader_id = id.clone();
                    let cb = on_output.clone();
                    let exit_cb = on_exit.clone();
                    let status_cb = on_status.clone();
                    let trackers_for_reader = status_trackers.clone();
                    let mut child_for_wait = child;
                    let reader_handle = thread::Builder::new()
                        .name(format!("pty-reader-{}", &id[..8]))
                        .spawn(move || {
                            let mut buf = [0u8; 4096];
                            loop {
                                match reader.read(&mut buf) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        let data = buf[..n].to_vec();
                                        cb(reader_id.clone(), data.clone());

                                        // Feed output to the status tracker.
                                        let status_change = {
                                            let mut trackers = trackers_for_reader.lock().unwrap();
                                            if let Some(tracker) = trackers.get_mut(&reader_id) {
                                                tracker.feed_output(&data)
                                            } else {
                                                None
                                            }
                                        };
                                        if let Some(new_status) = status_change {
                                            status_cb(
                                                reader_id.clone(),
                                                new_status.as_str().to_string(),
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        if e.kind() != std::io::ErrorKind::Other {
                                            eprintln!(
                                                "PTY read error for {}: {e}",
                                                &reader_id[..8]
                                            );
                                        }
                                        break;
                                    }
                                }
                            }
                            let exit_code = child_for_wait
                                .wait()
                                .ok()
                                .map(|status| status.exit_code());
                            exit_cb(reader_id.clone(), exit_code);

                            // Notify the status tracker of exit.
                            let code = exit_code.unwrap_or(1) as i32;
                            let status_change = {
                                let mut trackers = trackers_for_reader.lock().unwrap();
                                if let Some(tracker) = trackers.get_mut(&reader_id) {
                                    Some(tracker.notify_exit(code))
                                } else {
                                    None
                                }
                            };
                            if let Some(new_status) = status_change {
                                status_cb(reader_id, new_status.as_str().to_string());
                            }
                        })
                        .expect("failed to spawn PTY reader thread");

                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis();

                    sessions.insert(
                        id.clone(),
                        Session {
                            id: id.clone(),
                            name,
                            cwd,
                            master: pair.master,
                            writer,
                            created_at: Instant::now(),
                            created_at_epoch_ms: now,
                            _reader_handle: reader_handle,
                        },
                    );

                    let _ = reply.send(PtyResponse::Created { id });
                }

                PtyRequest::Write { id, data, reply } => {
                    if let Some(session) = sessions.get_mut(&id) {
                        match session.writer.write_all(&data) {
                            Ok(()) => {
                                let _ = session.writer.flush();
                                let _ = reply.send(PtyResponse::WriteOk);
                            }
                            Err(e) => {
                                let _ =
                                    reply.send(PtyResponse::Error(format!("Write failed: {e}")));
                            }
                        }
                    } else {
                        let _ =
                            reply.send(PtyResponse::Error(format!("Session not found: {id}")));
                    }
                }

                PtyRequest::Resize {
                    id,
                    cols,
                    rows,
                    reply,
                } => {
                    if let Some(session) = sessions.get(&id) {
                        let size = PtySize {
                            rows,
                            cols,
                            pixel_width: 0,
                            pixel_height: 0,
                        };
                        match session.master.resize(size) {
                            Ok(()) => {
                                let _ = reply.send(PtyResponse::ResizeOk);
                            }
                            Err(e) => {
                                let _ =
                                    reply.send(PtyResponse::Error(format!("Resize failed: {e}")));
                            }
                        }
                    } else {
                        let _ =
                            reply.send(PtyResponse::Error(format!("Session not found: {id}")));
                    }
                }

                PtyRequest::Rename { id, name, reply } => {
                    if let Some(session) = sessions.get_mut(&id) {
                        session.name = name;
                        let _ = reply.send(PtyResponse::RenameOk);
                    } else {
                        let _ =
                            reply.send(PtyResponse::Error(format!("Session not found: {id}")));
                    }
                }

                PtyRequest::Kill { id, reply } => {
                    if let Some(session) = sessions.remove(&id) {
                        drop(session.writer);
                        drop(session.master);
                        // Remove the status tracker for this session.
                        let mut trackers = status_trackers.lock().unwrap();
                        trackers.remove(&id);
                        let _ = reply.send(PtyResponse::Killed);
                    } else {
                        let _ =
                            reply.send(PtyResponse::Error(format!("Session not found: {id}")));
                    }
                }

                PtyRequest::List { reply } => {
                    let entries: Vec<SessionListEntry> = sessions
                        .values()
                        .map(|s| SessionListEntry {
                            id: s.id.clone(),
                            name: s.name.clone(),
                            cwd: s.cwd.clone(),
                            created_at_epoch_ms: s.created_at_epoch_ms,
                        })
                        .collect();
                    let _ = reply.send(PtyResponse::Sessions(entries));
                }

                PtyRequest::Shutdown => {
                    let ids: Vec<SessionId> = sessions.keys().cloned().collect();
                    for id in ids {
                        if let Some(session) = sessions.remove(&id) {
                            drop(session.writer);
                            drop(session.master);
                        }
                    }
                    let mut trackers = status_trackers.lock().unwrap();
                    trackers.clear();
                    break;
                }
            },

            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Tick all active status trackers to detect idle/needs_attention.
                let mut trackers = status_trackers.lock().unwrap();
                for (id, tracker) in trackers.iter_mut() {
                    if let Some(new_status) = tracker.tick() {
                        on_status(id.clone(), new_status.as_str().to_string());
                    }
                }
            }

            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn test_manager() -> (
        PtyManagerHandle,
        Arc<Mutex<Vec<(SessionId, Vec<u8>)>>>,
        Arc<Mutex<Vec<(SessionId, Option<u32>)>>>,
    ) {
        let output_log: Arc<Mutex<Vec<(SessionId, Vec<u8>)>>> = Arc::new(Mutex::new(Vec::new()));
        let exit_log: Arc<Mutex<Vec<(SessionId, Option<u32>)>>> = Arc::new(Mutex::new(Vec::new()));

        let ol = output_log.clone();
        let el = exit_log.clone();

        let handle = start(
            Box::new(move |id, data| {
                ol.lock().unwrap().push((id, data));
            }),
            Box::new(move |id, code| {
                el.lock().unwrap().push((id, code));
            }),
            Box::new(|_id, _status| {}),
        );

        (handle, output_log, exit_log)
    }

    #[test]
    fn test_create_and_list() {
        let (handle, _output, _exit) = test_manager();
        let resp = handle.create(
            "test-session".into(),
            std::env::temp_dir(),
            "echo".into(),
            vec!["hello".into()],
            80,
            24,
        );
        let id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };
        thread::sleep(Duration::from_millis(200));
        let resp = handle.list();
        match resp {
            PtyResponse::Sessions(entries) => {
                assert!(
                    entries.iter().any(|e| e.id == id),
                    "Session {} not found in list: {:?}",
                    id,
                    entries
                );
                let entry = entries.iter().find(|e| e.id == id).unwrap();
                assert_eq!(entry.name, "test-session");
            }
            other => panic!("Expected Sessions, got: {:?}", other),
        }
        handle.shutdown();
    }

    #[test]
    fn test_output_received() {
        let (handle, output_log, _exit) = test_manager();
        let resp = handle.create(
            "echo-test".into(),
            std::env::temp_dir(),
            "echo".into(),
            vec!["hello world".into()],
            80,
            24,
        );
        let _id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };
        thread::sleep(Duration::from_millis(500));
        let log = output_log.lock().unwrap();
        let all_output: Vec<u8> = log.iter().flat_map(|(_, data)| data.clone()).collect();
        let output_str = String::from_utf8_lossy(&all_output);
        assert!(
            output_str.contains("hello world"),
            "Expected 'hello world' in output, got: {:?}",
            output_str
        );
        handle.shutdown();
    }

    #[test]
    fn test_exit_callback() {
        let (handle, _output, exit_log) = test_manager();
        let resp = handle.create(
            "exit-test".into(),
            std::env::temp_dir(),
            "true".into(),
            vec![],
            80,
            24,
        );
        let id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };
        thread::sleep(Duration::from_millis(500));
        let log = exit_log.lock().unwrap();
        assert!(
            log.iter().any(|(eid, code)| eid == &id && *code == Some(0)),
            "Expected exit code 0 for session {}, got: {:?}",
            id,
            *log
        );
        handle.shutdown();
    }

    #[test]
    fn test_write_to_session() {
        let (handle, output_log, _exit) = test_manager();
        let resp = handle.create(
            "cat-test".into(),
            std::env::temp_dir(),
            "cat".into(),
            vec![],
            80,
            24,
        );
        let id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };
        let resp = handle.write(id.clone(), b"ping\n".to_vec());
        match resp {
            PtyResponse::WriteOk => {}
            other => panic!("Expected WriteOk, got: {:?}", other),
        }
        thread::sleep(Duration::from_millis(500));
        let log = output_log.lock().unwrap();
        let all_output: Vec<u8> = log
            .iter()
            .filter(|(eid, _)| eid == &id)
            .flat_map(|(_, data)| data.clone())
            .collect();
        let output_str = String::from_utf8_lossy(&all_output);
        assert!(
            output_str.contains("ping"),
            "Expected 'ping' in output, got: {:?}",
            output_str
        );
        handle.shutdown();
    }

    #[test]
    fn test_resize() {
        let (handle, _output, _exit) = test_manager();
        let resp = handle.create(
            "resize-test".into(),
            std::env::temp_dir(),
            "cat".into(),
            vec![],
            80,
            24,
        );
        let id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };
        let resp = handle.resize(id.clone(), 120, 40);
        match resp {
            PtyResponse::ResizeOk => {}
            other => panic!("Expected ResizeOk, got: {:?}", other),
        }
        handle.shutdown();
    }

    #[test]
    fn test_rename_session() {
        let (handle, _output, _exit) = test_manager();
        let resp = handle.create(
            "original-name".into(),
            std::env::temp_dir(),
            "cat".into(),
            vec![],
            80,
            24,
        );
        let id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };
        let resp = handle.rename(id.clone(), "new-name".into());
        match resp {
            PtyResponse::RenameOk => {}
            other => panic!("Expected RenameOk, got: {:?}", other),
        }
        let resp = handle.list();
        match resp {
            PtyResponse::Sessions(entries) => {
                let entry = entries.iter().find(|e| e.id == id).unwrap();
                assert_eq!(entry.name, "new-name");
            }
            other => panic!("Expected Sessions, got: {:?}", other),
        }
        handle.shutdown();
    }

    #[test]
    fn test_kill_session() {
        let (handle, _output, _exit) = test_manager();
        let resp = handle.create(
            "kill-test".into(),
            std::env::temp_dir(),
            "cat".into(),
            vec![],
            80,
            24,
        );
        let id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };
        let resp = handle.kill(id.clone());
        match resp {
            PtyResponse::Killed => {}
            other => panic!("Expected Killed, got: {:?}", other),
        }
        let resp = handle.list();
        match resp {
            PtyResponse::Sessions(entries) => {
                assert!(
                    !entries.iter().any(|e| e.id == id),
                    "Session should have been removed after kill"
                );
            }
            other => panic!("Expected Sessions, got: {:?}", other),
        }
        handle.shutdown();
    }

    #[test]
    fn test_write_to_nonexistent_session() {
        let (handle, _output, _exit) = test_manager();
        let resp = handle.write("nonexistent-id".into(), b"data".to_vec());
        match resp {
            PtyResponse::Error(msg) => {
                assert!(
                    msg.contains("not found"),
                    "Error should mention 'not found': {msg}"
                );
            }
            other => panic!("Expected Error, got: {:?}", other),
        }
        handle.shutdown();
    }

    #[test]
    fn test_kill_nonexistent_session() {
        let (handle, _output, _exit) = test_manager();
        let resp = handle.kill("nonexistent-id".into());
        match resp {
            PtyResponse::Error(msg) => {
                assert!(
                    msg.contains("not found"),
                    "Error should mention 'not found': {msg}"
                );
            }
            other => panic!("Expected Error, got: {:?}", other),
        }
        handle.shutdown();
    }

    #[test]
    fn test_nonzero_exit_code() {
        let (handle, _output, exit_log) = test_manager();
        let resp = handle.create(
            "fail-test".into(),
            std::env::temp_dir(),
            "false".into(),
            vec![],
            80,
            24,
        );
        let id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };
        thread::sleep(Duration::from_millis(500));
        let log = exit_log.lock().unwrap();
        assert!(
            log.iter().any(|(eid, code)| eid == &id && *code == Some(1)),
            "Expected exit code 1 for session {}, got: {:?}",
            id,
            *log
        );
        handle.shutdown();
    }

    #[test]
    fn test_shutdown_kills_all_sessions() {
        let (handle, _output, exit_log) = test_manager();
        let mut ids = Vec::new();
        for i in 0..3 {
            let resp = handle.create(
                format!("session-{i}"),
                std::env::temp_dir(),
                "cat".into(),
                vec![],
                80,
                24,
            );
            match resp {
                PtyResponse::Created { id } => ids.push(id),
                other => panic!("Expected Created, got: {:?}", other),
            }
        }
        let resp = handle.list();
        match resp {
            PtyResponse::Sessions(entries) => {
                assert_eq!(
                    entries.len(),
                    3,
                    "Expected 3 sessions, got {}",
                    entries.len()
                );
            }
            other => panic!("Expected Sessions, got: {:?}", other),
        }
        handle.shutdown();
        thread::sleep(Duration::from_millis(1000));
        let log = exit_log.lock().unwrap();
        for id in &ids {
            assert!(
                log.iter().any(|(eid, _)| eid == id),
                "Expected exit callback for session {}, got: {:?}",
                id,
                *log
            );
        }
    }
}
