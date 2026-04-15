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
}

impl From<SessionListEntry> for SessionInfo {
    fn from(e: SessionListEntry) -> Self {
        Self {
            id: e.id,
            name: e.name,
            cwd: e.cwd,
            created_at_epoch_ms: e.created_at_epoch_ms,
        }
    }
}

#[tauri::command]
pub fn create_session(
    state: State<'_, AppState>,
    name: String,
    cwd: String,
    command: Option<String>,
    args: Option<Vec<String>>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<String, String> {
    let path = PathBuf::from(&cwd);
    if !path.exists() {
        return Err(format!("Directory does not exist: {cwd}"));
    }

    let command = command.unwrap_or_else(|| {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    });
    let args = args.unwrap_or_default();
    let cols = cols.unwrap_or(80);
    let rows = rows.unwrap_or(24);

    match state.pty.create(name, path, command, args, cols, rows) {
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
