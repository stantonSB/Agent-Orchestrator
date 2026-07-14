//! PTY manager module.
//!
//! Owns all PTY state on a dedicated thread. Communicates with callers
//! via channel-based messages (PtyRequest / PtyResponse).
//!
//! Design: PTY handles from portable-pty are not Send/Sync, so they
//! cannot be shared across threads. All PTY state lives exclusively on
//! the manager thread. External code sends requests via an mpsc channel
//! and receives responses via oneshot channels.

use crate::status_parser::{SessionStatus, StatusTracker};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

/// Capture the user's full login-shell environment.
///
/// macOS .app bundles launched from Finder inherit a minimal environment
/// (PATH=/usr/bin:/bin:/usr/sbin:/sbin) — none of the user's shell profile
/// variables (NODE_EXTRA_CA_CERTS, custom PATH entries, etc.) are present.
///
/// This runs `$SHELL -li -c env` once, parses the output, and caches it
/// for the lifetime of the process. If it fails for any reason, we fall
/// back to the process's own (minimal) environment.
/// Eagerly initialise the cached login-shell environment.
///
/// Call this once during startup — before the PTY manager thread and other
/// background threads are created — so that the internal `fork+exec` of
/// `$SHELL -li -c env` runs while the process has the fewest threads.  This
/// reduces the window for the macOS "multi-threaded process forked" crash.
pub fn warm_shell_env() {
    let _ = shell_env();
}

fn shell_env() -> &'static HashMap<String, String> {
    static ENV: OnceLock<HashMap<String, String>> = OnceLock::new();
    ENV.get_or_init(|| {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        std::process::Command::new(&shell)
            .args(["-li", "-c", "env"])
            .output()
            .ok()
            .and_then(|out| {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let map: HashMap<String, String> = stdout
                    .lines()
                    .filter_map(|line| line.split_once('='))
                    .map(|(k, v)| (k.to_owned(), v.to_owned()))
                    .collect();
                if map.is_empty() { None } else { Some(map) }
            })
            .unwrap_or_default()
    })
}

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Whether this session runs Claude Code or a plain shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionType {
    Claude,
    Terminal,
}

impl SessionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionType::Claude => "claude",
            SessionType::Terminal => "terminal",
        }
    }
}

/// Claude session mode — controls which CLI flags are passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeMode {
    Default,
    Auto,
    Skip,
    Plan,
}

/// Unique identifier for a PTY session.
pub type SessionId = String;

/// Requests sent to the PTY manager thread.
pub enum PtyRequest {
    Create {
        name: String,
        cwd: PathBuf,
        command: String,
        args: Vec<String>,
        session_type: SessionType,
        is_git_repo: bool,
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
    pub created_at_epoch_ms: u64,
    pub session_type: SessionType,
    pub is_git_repo: bool,
}

pub type OutputCallback = Box<dyn Fn(SessionId, Vec<u8>) + Send + Sync + 'static>;
pub type ExitCallback = Box<dyn Fn(SessionId, Option<u32>) + Send + Sync + 'static>;
pub type StatusCallback = Box<dyn Fn(SessionId, String) + Send + Sync + 'static>;
pub type SubagentCallback = Box<dyn Fn(SessionId, Vec<crate::subagent_tracker::SubagentStatusPayload>) + Send + Sync + 'static>;
pub type WorktreeCwdCallback = Box<dyn Fn(SessionId, String) + Send + Sync + 'static>;

// ---------------------------------------------------------------------------
// Output coalescing
// ---------------------------------------------------------------------------

/// Flush accumulated PTY output once it reaches this many bytes.
const COALESCE_SIZE_CAP: usize = 64 * 1024;
/// Flush accumulated PTY output at most this long after its first byte.
const COALESCE_WINDOW: Duration = Duration::from_millis(8);

/// Accumulates PTY output so the reader emits fewer, larger chunks instead of
/// one IPC event per `read()` (which are <=4 KB). Output is flushed when either
/// the size cap is reached or the time window elapses since the batch's first
/// byte — whichever comes first.
///
/// This is a pure buffering helper: it holds no PTY state, so it stays
/// trivially unit-testable and does not violate the "PTY state lives only on
/// the manager thread" invariant.
struct OutputCoalescer {
    buf: Vec<u8>,
    deadline: Option<Instant>,
    size_cap: usize,
    window: Duration,
}

impl OutputCoalescer {
    fn new(size_cap: usize, window: Duration) -> Self {
        Self {
            buf: Vec::with_capacity(size_cap),
            deadline: None,
            size_cap,
            window,
        }
    }

    /// Append `data`. Returns `Some(bytes)` to emit immediately when the size
    /// cap is reached; otherwise `None` (a time-based flush will follow). `now`
    /// is injected so the deadline is deterministic in tests.
    fn push(&mut self, data: &[u8], now: Instant) -> Option<Vec<u8>> {
        if self.buf.is_empty() {
            self.deadline = Some(now + self.window);
        }
        self.buf.extend_from_slice(data);
        if self.buf.len() >= self.size_cap {
            self.deadline = None;
            Some(std::mem::take(&mut self.buf))
        } else {
            None
        }
    }

    /// Instant at which pending bytes must be flushed, if any are buffered.
    fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    /// Drain whatever is buffered (used on the time-based flush and at stream
    /// end). Returns `None` if nothing is pending.
    fn take(&mut self) -> Option<Vec<u8>> {
        self.deadline = None;
        if self.buf.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.buf))
        }
    }

    #[cfg(test)]
    fn has_pending(&self) -> bool {
        !self.buf.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Internal session state (lives exclusively on the manager thread)
// ---------------------------------------------------------------------------

struct Session {
    id: SessionId,
    name: String,
    #[allow(dead_code)]
    cwd: PathBuf,
    session_type: SessionType,
    is_git_repo: bool,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn std::io::Write + Send>,
    #[allow(dead_code)]
    created_at: Instant,
    created_at_epoch_ms: u64,
    _reader_handle: thread::JoinHandle<()>,
}

// ---------------------------------------------------------------------------
// PtyManager handle (clone-friendly, Send + Sync)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PtyManagerHandle {
    tx: mpsc::Sender<PtyRequest>,
}

/// Resolve `claude` to its absolute path via the captured login-shell
/// environment's PATH.  Using an absolute path prevents `portable-pty`'s
/// `CommandBuilder::search_path` from matching a *directory* named `claude`
/// inside the session's cwd (it checks `cwd.join(exe).exists()` before
/// consulting PATH, and `.exists()` is true for directories).
fn resolve_claude_path() -> String {
    // Check the login-shell PATH first (covers .app launched from Finder).
    if let Some(path_val) = shell_env().get("PATH") {
        for dir in std::env::split_paths(&std::ffi::OsString::from(path_val)) {
            let candidate = dir.join("claude");
            if candidate.is_file() {
                if let Some(s) = candidate.to_str() {
                    return s.to_string();
                }
            }
        }
    }

    // Fall back to the process's own PATH.
    if let Ok(path_val) = std::env::var("PATH") {
        for dir in std::env::split_paths(&std::ffi::OsString::from(&path_val)) {
            let candidate = dir.join("claude");
            if candidate.is_file() {
                if let Some(s) = candidate.to_str() {
                    return s.to_string();
                }
            }
        }
    }

    // Last resort — let the OS figure it out.
    "claude".to_string()
}

/// Derive the command and arguments for a PTY session.
/// This is the security-load-bearing logic that prevents arbitrary command
/// execution — only known-safe commands are produced.
pub fn derive_argv(session_type: SessionType, claude_mode: ClaudeMode, is_git_repo: bool) -> (String, Vec<String>) {
    match session_type {
        SessionType::Claude => {
            let mut a: Vec<String> = Vec::new();
            match claude_mode {
                ClaudeMode::Auto => {
                    a.push("--permission-mode".to_string());
                    a.push("auto".to_string());
                }
                ClaudeMode::Skip => a.push("--dangerously-skip-permissions".to_string()),
                ClaudeMode::Plan => {
                    a.push("--permission-mode".to_string());
                    a.push("plan".to_string());
                }
                ClaudeMode::Default => {}
            }
            if is_git_repo {
                a.push("--worktree".to_string());
            }
            // Status hooks are scoped to this session via --settings rather
            // than installed into the user's global ~/.claude/settings.json,
            // so non-AO sessions never run the notify script.
            a.push("--settings".to_string());
            a.push(crate::hook_installer::session_hook_settings());
            (resolve_claude_path(), a)
        }
        SessionType::Terminal => {
            let shell = std::env::var("SHELL")
                .unwrap_or_else(|_| "/bin/sh".to_string());
            (shell, Vec::new())
        }
    }
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

    /// Create a new PTY session. Command and args are derived from
    /// session_type + claude_mode — callers cannot specify arbitrary
    /// commands (defense-in-depth against IPC abuse).
    pub fn create(
        &self,
        name: String,
        cwd: PathBuf,
        cols: u16,
        rows: u16,
        session_type: SessionType,
        claude_mode: ClaudeMode,
        is_git_repo: bool,
    ) -> PtyResponse {
        let (command, args) = derive_argv(session_type, claude_mode, is_git_repo);
        self.request(|reply| PtyRequest::Create {
            name,
            cwd,
            command,
            args,
            session_type,
            is_git_repo,
            cols,
            rows,
            reply,
        })
    }

    /// Test-only: create a session with explicit command and args.
    /// This bypasses the command derivation and should never be
    /// exposed on the IPC surface.
    #[cfg(test)]
    pub fn create_raw(
        &self,
        name: String,
        cwd: PathBuf,
        command: String,
        args: Vec<String>,
        cols: u16,
        rows: u16,
        session_type: SessionType,
    ) -> PtyResponse {
        self.request(|reply| PtyRequest::Create {
            name,
            cwd,
            command,
            args,
            session_type,
            is_git_repo: false,
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
    status_trackers: Arc<Mutex<HashMap<SessionId, StatusTracker>>>,
    status_port: u16,
) -> PtyManagerHandle {
    let (tx, rx) = mpsc::channel::<PtyRequest>();

    let on_output = Arc::new(on_output);
    let on_exit = Arc::new(on_exit);
    let on_status = Arc::new(on_status);

    thread::Builder::new()
        .name("pty-manager".into())
        .spawn(move || {
            manager_loop(rx, on_output, on_exit, on_status, status_trackers, status_port);
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
    status_port: u16,
) {
    let mut sessions: HashMap<SessionId, Session> = HashMap::new();
    let pty_system = native_pty_system();

    loop {
        match rx.recv() {
            Ok(request) => match request {
                PtyRequest::Create {
                    name,
                    cwd,
                    command,
                    args,
                    session_type,
                    is_git_repo,
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

                    // macOS .app bundles inherit a minimal environment when
                    // launched from Finder. Apply the user's full login-shell
                    // environment so things like PATH, NODE_EXTRA_CA_CERTS,
                    // and other profile-set variables are available.
                    for (key, value) in shell_env() {
                        cmd.env(key, value);
                    }

                    // Override TERM/COLORTERM *after* shell_env() so that
                    // values like "dumb" or "screen" don't leak through.
                    cmd.env("TERM", "xterm-256color");
                    cmd.env("COLORTERM", "truecolor");

                    // Remove CLAUDECODE so spawned sessions don't detect
                    // nesting when AO itself runs inside Claude Code.
                    cmd.env_remove("CLAUDECODE");

                    // Pass session identity and status server port so that
                    // Claude Code hook scripts can report status back to us.
                    if session_type == SessionType::Claude {
                        cmd.env("AO_SESSION_ID", &id);
                        cmd.env("AO_STATUS_PORT", status_port.to_string());
                    }

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

                    // Insert a new status tracker for this session (Claude only).
                    if session_type == SessionType::Claude {
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
                            // Two cooperating threads:
                            //   * an inner I/O thread does the blocking PTY
                            //     reads and forwards each chunk over a channel;
                            //   * this thread coalesces those chunks and emits
                            //     fewer, larger payloads via `cb`.
                            // Splitting them lets the coalescer flush pending
                            // bytes on a short timer even while the I/O thread
                            // is parked in a blocking `read()`, so output never
                            // stalls when the child goes idle mid-batch.
                            let (chunk_tx, chunk_rx) = mpsc::channel::<Vec<u8>>();

                            let io_reader_id = reader_id.clone();
                            let io_handle = thread::Builder::new()
                                .name(format!("pty-io-{}", &io_reader_id[..8]))
                                .spawn(move || {
                                    let mut buf = [0u8; 4096];
                                    loop {
                                        match reader.read(&mut buf) {
                                            Ok(0) => break,
                                            Ok(n) => {
                                                if chunk_tx.send(buf[..n].to_vec()).is_err() {
                                                    break; // coalescer gone
                                                }
                                            }
                                            Err(e) => {
                                                if e.kind() != std::io::ErrorKind::Other {
                                                    eprintln!(
                                                        "PTY read error for {}: {e}",
                                                        &io_reader_id[..8]
                                                    );
                                                }
                                                break;
                                            }
                                        }
                                    }
                                    // chunk_tx is dropped here → the coalescer
                                    // loop below observes Disconnected.
                                })
                                .expect("failed to spawn PTY io thread");

                            let mut coalescer =
                                OutputCoalescer::new(COALESCE_SIZE_CAP, COALESCE_WINDOW);
                            loop {
                                let timeout = match coalescer.deadline() {
                                    Some(d) => d.saturating_duration_since(Instant::now()),
                                    // Nothing pending — block until the next chunk.
                                    None => Duration::from_secs(3600),
                                };
                                match chunk_rx.recv_timeout(timeout) {
                                    Ok(chunk) => {
                                        if let Some(flush) =
                                            coalescer.push(&chunk, Instant::now())
                                        {
                                            cb(reader_id.clone(), flush);
                                        }
                                    }
                                    Err(mpsc::RecvTimeoutError::Timeout) => {
                                        if let Some(flush) = coalescer.take() {
                                            cb(reader_id.clone(), flush);
                                        }
                                    }
                                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                                        if let Some(flush) = coalescer.take() {
                                            cb(reader_id.clone(), flush);
                                        }
                                        break;
                                    }
                                }
                            }

                            // Ensure all output has been emitted before the
                            // exit callback fires, so the terminal shows the
                            // child's final bytes before its "finished" status.
                            let _ = io_handle.join();

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

                    // Spawn a timer that transitions Starting → Idle after
                    // 5 seconds if no hook event has arrived.  Claude Code
                    // does not fire `idle_prompt` on initial startup, so
                    // without this the status would stay "Starting" until
                    // the user presses Enter.
                    if session_type == SessionType::Claude {
                        let timer_id = id.clone();
                        let timer_trackers = status_trackers.clone();
                        let timer_status_cb = on_status.clone();
                        thread::Builder::new()
                            .name(format!("startup-timer-{}", &id[..8]))
                            .spawn(move || {
                                thread::sleep(std::time::Duration::from_secs(5));
                                let transition = {
                                    let mut trackers = timer_trackers.lock().unwrap();
                                    if let Some(tracker) = trackers.get_mut(&timer_id) {
                                        if *tracker.status() == SessionStatus::Starting {
                                            tracker.set_status(SessionStatus::Idle)
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                };
                                if let Some(new_status) = transition {
                                    timer_status_cb(
                                        timer_id,
                                        new_status.as_str().to_string(),
                                    );
                                }
                            })
                            .expect("failed to spawn startup timer thread");
                    }

                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;

                    sessions.insert(
                        id.clone(),
                        Session {
                            id: id.clone(),
                            name,
                            cwd,
                            session_type,
                            is_git_repo,
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

                                // Notify status tracker of user input so it can
                                // detect Enter key presses and transition to Working.
                                let status_change = {
                                    let mut trackers = status_trackers.lock().unwrap();
                                    if let Some(tracker) = trackers.get_mut(&id) {
                                        tracker.notify_user_input(&data)
                                    } else {
                                        None
                                    }
                                };
                                if let Some(new_status) = status_change {
                                    on_status(id.clone(), new_status.as_str().to_string());
                                }

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
                            session_type: s.session_type,
                            is_git_repo: s.is_git_repo,
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

            Err(_) => {
                // Sender disconnected; shut down cleanly.
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

        let status_trackers = Arc::new(Mutex::new(HashMap::new()));

        let handle = start(
            Box::new(move |id, data| {
                ol.lock().unwrap().push((id, data));
            }),
            Box::new(move |id, code| {
                el.lock().unwrap().push((id, code));
            }),
            Box::new(|_id, _status| {}),
            status_trackers,
            0, // status_port: 0 means no status server in tests
        );

        (handle, output_log, exit_log)
    }

    #[test]
    fn test_create_and_list() {
        let (handle, _output, _exit) = test_manager();
        let resp = handle.create_raw(
            "test-session".into(),
            std::env::temp_dir(),
            "echo".into(),
            vec!["hello".into()],
            80,
            24,
            SessionType::Claude,
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
        let resp = handle.create_raw(
            "echo-test".into(),
            std::env::temp_dir(),
            "echo".into(),
            vec!["hello world".into()],
            80,
            24,
            SessionType::Claude,
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
        let resp = handle.create_raw(
            "exit-test".into(),
            std::env::temp_dir(),
            "true".into(),
            vec![],
            80,
            24,
            SessionType::Claude,
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
        let resp = handle.create_raw(
            "cat-test".into(),
            std::env::temp_dir(),
            "cat".into(),
            vec![],
            80,
            24,
            SessionType::Claude,
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
        let resp = handle.create_raw(
            "resize-test".into(),
            std::env::temp_dir(),
            "cat".into(),
            vec![],
            80,
            24,
            SessionType::Claude,
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
        let resp = handle.create_raw(
            "original-name".into(),
            std::env::temp_dir(),
            "cat".into(),
            vec![],
            80,
            24,
            SessionType::Claude,
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
        let resp = handle.create_raw(
            "kill-test".into(),
            std::env::temp_dir(),
            "cat".into(),
            vec![],
            80,
            24,
            SessionType::Claude,
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
        let resp = handle.create_raw(
            "fail-test".into(),
            std::env::temp_dir(),
            "false".into(),
            vec![],
            80,
            24,
            SessionType::Claude,
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
            let resp = handle.create_raw(
                format!("session-{i}"),
                std::env::temp_dir(),
                "cat".into(),
                vec![],
                80,
                24,
                SessionType::Claude,
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

    #[test]
    fn test_terminal_session_no_tracker() {
        let status_trackers = Arc::new(Mutex::new(HashMap::new()));
        let status_trackers_clone = status_trackers.clone();

        let handle = start(
            Box::new(|_id, _data| {}),
            Box::new(|_id, _code| {}),
            Box::new(|_id, _status| {}),
            status_trackers_clone,
            0,
        );

        let resp = handle.create_raw(
            "terminal-test".into(),
            std::env::temp_dir(),
            "echo".into(),
            vec!["hello".into()],
            80,
            24,
            SessionType::Terminal,
        );
        let id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };

        let trackers = status_trackers.lock().unwrap();
        assert!(
            !trackers.contains_key(&id),
            "Terminal session should not have a status tracker"
        );

        drop(trackers);
        handle.shutdown();
    }

    /// Expected argv: the mode/worktree flags followed by the per-session
    /// hook settings that every Claude session receives.
    fn with_settings(base: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = base.iter().map(|s| s.to_string()).collect();
        v.push("--settings".to_string());
        v.push(crate::hook_installer::session_hook_settings());
        v
    }

    #[test]
    fn test_derive_argv_claude_default() {
        let (cmd, args) = derive_argv(SessionType::Claude, ClaudeMode::Default, false);
        assert!(cmd.ends_with("claude"), "Command should end with 'claude', got: {cmd}");
        assert_eq!(args, with_settings(&[]));
    }

    #[test]
    fn test_derive_argv_claude_default_git() {
        let (cmd, args) = derive_argv(SessionType::Claude, ClaudeMode::Default, true);
        assert!(cmd.ends_with("claude"), "Command should end with 'claude', got: {cmd}");
        assert_eq!(args, with_settings(&["--worktree"]));
    }

    #[test]
    fn test_derive_argv_claude_auto() {
        let (cmd, args) = derive_argv(SessionType::Claude, ClaudeMode::Auto, false);
        assert!(cmd.ends_with("claude"), "Command should end with 'claude', got: {cmd}");
        assert_eq!(args, with_settings(&["--permission-mode", "auto"]));
    }

    #[test]
    fn test_derive_argv_claude_auto_git() {
        let (cmd, args) = derive_argv(SessionType::Claude, ClaudeMode::Auto, true);
        assert!(cmd.ends_with("claude"), "Command should end with 'claude', got: {cmd}");
        assert_eq!(args, with_settings(&["--permission-mode", "auto", "--worktree"]));
    }

    #[test]
    fn test_derive_argv_claude_skip() {
        let (cmd, args) = derive_argv(SessionType::Claude, ClaudeMode::Skip, false);
        assert!(cmd.ends_with("claude"), "Command should end with 'claude', got: {cmd}");
        assert_eq!(args, with_settings(&["--dangerously-skip-permissions"]));
    }

    #[test]
    fn test_derive_argv_claude_skip_git() {
        let (cmd, args) = derive_argv(SessionType::Claude, ClaudeMode::Skip, true);
        assert!(cmd.ends_with("claude"), "Command should end with 'claude', got: {cmd}");
        assert_eq!(args, with_settings(&["--dangerously-skip-permissions", "--worktree"]));
    }

    #[test]
    fn test_derive_argv_claude_plan() {
        let (cmd, args) = derive_argv(SessionType::Claude, ClaudeMode::Plan, false);
        assert!(cmd.ends_with("claude"), "Command should end with 'claude', got: {cmd}");
        assert_eq!(args, with_settings(&["--permission-mode", "plan"]));
    }

    #[test]
    fn test_derive_argv_claude_plan_git() {
        let (cmd, args) = derive_argv(SessionType::Claude, ClaudeMode::Plan, true);
        assert!(cmd.ends_with("claude"), "Command should end with 'claude', got: {cmd}");
        assert_eq!(args, with_settings(&["--permission-mode", "plan", "--worktree"]));
    }

    #[test]
    fn test_derive_argv_claude_settings_is_valid_json_with_hooks() {
        let (_cmd, args) = derive_argv(SessionType::Claude, ClaudeMode::Default, false);
        let flag_pos = args.iter().position(|a| a == "--settings").expect("--settings present");
        let payload: serde_json::Value =
            serde_json::from_str(&args[flag_pos + 1]).expect("settings payload is valid JSON");
        assert!(payload.get("hooks").is_some(), "settings payload should register hooks");
    }

    #[test]
    fn test_derive_argv_claude_resolves_absolute_path() {
        let (cmd, _args) = derive_argv(SessionType::Claude, ClaudeMode::Default, false);
        // Should resolve to an absolute path when claude is installed
        if cmd != "claude" {
            assert!(cmd.starts_with('/'), "Resolved path should be absolute, got: {cmd}");
            assert!(std::path::Path::new(&cmd).is_file(), "Resolved path should be an existing file: {cmd}");
        }
    }

    #[test]
    fn test_derive_argv_terminal() {
        let (cmd, args) = derive_argv(SessionType::Terminal, ClaudeMode::Default, false);
        assert!(!cmd.is_empty(), "Terminal should have a shell command");
        assert!(args.is_empty(), "Terminal should have no args");
        // Command should be a shell, not "claude"
        assert_ne!(cmd, "claude");
    }

    #[test]
    fn test_terminal_session_list_type() {
        let (handle, _output, _exit) = test_manager();
        let resp = handle.create_raw(
            "terminal-list".into(),
            std::env::temp_dir(),
            "cat".into(),
            vec![],
            80,
            24,
            SessionType::Terminal,
        );
        let id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };

        let resp = handle.list();
        match resp {
            PtyResponse::Sessions(entries) => {
                let entry = entries.iter().find(|e| e.id == id).unwrap();
                assert_eq!(entry.session_type, SessionType::Terminal);
            }
            other => panic!("Expected Sessions, got: {:?}", other),
        }
        handle.shutdown();
    }

    // ---- OutputCoalescer ----

    #[test]
    fn coalescer_buffers_below_cap_then_drains() {
        let mut c = OutputCoalescer::new(64 * 1024, Duration::from_millis(8));
        let now = Instant::now();
        assert!(c.push(b"hello ", now).is_none());
        assert!(c.push(b"world", now).is_none());
        assert!(c.has_pending());
        assert_eq!(c.take(), Some(b"hello world".to_vec()));
        assert!(!c.has_pending());
        // Nothing left to drain.
        assert_eq!(c.take(), None);
    }

    #[test]
    fn coalescer_flushes_on_size_cap() {
        let mut c = OutputCoalescer::new(8, Duration::from_millis(8));
        let now = Instant::now();
        assert!(c.push(b"abc", now).is_none());
        // Crossing the cap returns the whole accumulated batch.
        assert_eq!(c.push(b"defghij", now), Some(b"abcdefghij".to_vec()));
        assert!(!c.has_pending());
        // Deadline is cleared after a size-cap flush.
        assert_eq!(c.deadline(), None);
    }

    #[test]
    fn coalescer_deadline_tracks_first_byte_of_batch() {
        let mut c = OutputCoalescer::new(64 * 1024, Duration::from_millis(8));
        assert_eq!(c.deadline(), None);
        let start = Instant::now();
        c.push(b"x", start);
        let deadline = c.deadline().expect("deadline set on first byte");
        assert_eq!(deadline, start + Duration::from_millis(8));
        // A later push in the same batch must not move the deadline back.
        c.push(b"y", start + Duration::from_millis(5));
        assert_eq!(c.deadline(), Some(deadline));
        // After draining, the next batch starts a fresh deadline.
        let drained = c.take().unwrap();
        assert_eq!(drained, b"xy".to_vec());
        assert_eq!(c.deadline(), None);
        let later = start + Duration::from_millis(20);
        c.push(b"z", later);
        assert_eq!(c.deadline(), Some(later + Duration::from_millis(8)));
    }
}
