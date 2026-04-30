use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use crate::pty_manager::{PtyResponse, SessionListEntry};
use crate::state::AppState;

/// Serializable session info returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub name: String,
    pub cwd: PathBuf,
    pub created_at_epoch_ms: u128,
    pub session_type: String,
}

impl From<SessionListEntry> for SessionInfo {
    fn from(e: SessionListEntry) -> Self {
        Self {
            id: e.id,
            name: e.name,
            cwd: e.cwd,
            created_at_epoch_ms: e.created_at_epoch_ms,
            session_type: e.session_type,
        }
    }
}

#[tauri::command]
pub fn create_session(
    state: State<'_, AppState>,
    name: String,
    cwd: String,
    cols: Option<u16>,
    rows: Option<u16>,
    session_type: Option<String>,
    session_mode: Option<String>,
    is_git_repo: Option<bool>,
    pull_latest: Option<bool>,
) -> Result<String, String> {
    let path = PathBuf::from(&cwd);
    if !path.exists() {
        return Err(format!("Directory does not exist: {cwd}"));
    }

    let session_type = match session_type.as_deref() {
        Some("terminal") => crate::pty_manager::SessionType::Terminal,
        _ => crate::pty_manager::SessionType::Claude,
    };

    let claude_mode = match session_mode.as_deref() {
        Some("auto") => crate::pty_manager::ClaudeMode::Auto,
        Some("skip") => crate::pty_manager::ClaudeMode::Skip,
        Some("plan") => crate::pty_manager::ClaudeMode::Plan,
        _ => crate::pty_manager::ClaudeMode::Default,
    };

    let is_git_repo = is_git_repo.unwrap_or(false);

    // If requested, pull latest main before spawning the session.
    if pull_latest.unwrap_or(false) {
        git_pull_main_internal(&path)?;
    }

    let cols = cols.unwrap_or(80);
    let rows = rows.unwrap_or(24);

    match state.pty.create(name, path, cols, rows, session_type, claude_mode, is_git_repo) {
        PtyResponse::Created { id } => Ok(id),
        PtyResponse::Error(msg) => Err(msg),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub fn close_session(state: State<'_, AppState>, id: String) -> Result<(), String> {
    match state.pty.kill(id) {
        PtyResponse::Killed => Ok(()),
        PtyResponse::Error(msg) => Err(msg),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub fn write_to_session(
    state: State<'_, AppState>,
    id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    match state.pty.write(id, data) {
        PtyResponse::WriteOk => Ok(()),
        PtyResponse::Error(msg) => Err(msg),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub fn resize_session(
    state: State<'_, AppState>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    match state.pty.resize(id, cols, rows) {
        PtyResponse::ResizeOk => Ok(()),
        PtyResponse::Error(msg) => Err(msg),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub fn rename_session(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<(), String> {
    match state.pty.rename(id, name) {
        PtyResponse::RenameOk => Ok(()),
        PtyResponse::Error(msg) => Err(msg),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionInfo>, String> {
    match state.pty.list() {
        PtyResponse::Sessions(entries) => {
            Ok(entries.into_iter().map(SessionInfo::from).collect())
        }
        PtyResponse::Error(msg) => Err(msg),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Internal helper — not exposed as an IPC command.
fn git_pull_main_internal(path: &std::path::Path) -> Result<(), String> {
    // Checkout main branch
    let checkout = std::process::Command::new("git")
        .args(["checkout", "main"])
        .current_dir(path)
        .output()
        .map_err(|e| format!("Failed to run git checkout: {e}"))?;

    if !checkout.status.success() {
        let stderr = String::from_utf8_lossy(&checkout.stderr);
        return Err(format!("git checkout main failed: {stderr}"));
    }

    // Pull latest
    let pull = std::process::Command::new("git")
        .args(["pull", "origin", "main"])
        .current_dir(path)
        .output()
        .map_err(|e| format!("Failed to run git pull: {e}"))?;

    if !pull.status.success() {
        let stderr = String::from_utf8_lossy(&pull.stderr);
        return Err(format!("git pull origin main failed: {stderr}"));
    }

    Ok(())
}

#[tauri::command]
pub fn get_session_status(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<String>, String> {
    let trackers = state
        .status_trackers
        .lock()
        .map_err(|e| format!("Failed to lock status trackers: {e}"))?;
    Ok(trackers.get(&id).map(|t| t.status().as_str().to_string()))
}

#[tauri::command]
pub fn check_is_git_repo(cwd: String) -> Result<bool, String> {
    let path = PathBuf::from(&cwd);
    if !path.exists() {
        return Err(format!("Directory does not exist: {cwd}"));
    }

    let output = std::process::Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(&path)
        .output()
        .map_err(|e| format!("Failed to run git: {e}"))?;

    Ok(output.status.success())
}
