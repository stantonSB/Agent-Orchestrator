use std::collections::HashMap;
use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use crate::pty_manager::{self, PtyResponse, SessionListEntry};
use crate::state::AppState;

/// Serializable session info returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub name: String,
    pub cwd: PathBuf,
    pub created_at_epoch_ms: u64,
    pub session_type: String,
    pub is_git_repo: bool,
    pub worktree_cwd: Option<String>,
}

impl SessionInfo {
    fn from_entry(e: SessionListEntry, worktree_cwd: Option<String>) -> Self {
        Self {
            id: e.id,
            name: e.name,
            cwd: e.cwd,
            created_at_epoch_ms: e.created_at_epoch_ms,
            session_type: match e.session_type {
                pty_manager::SessionType::Claude => "claude".to_string(),
                pty_manager::SessionType::Terminal => "terminal".to_string(),
            },
            is_git_repo: e.is_git_repo,
            worktree_cwd,
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
        Some("claude") | None => crate::pty_manager::SessionType::Claude,
        Some(other) => return Err(format!("Unknown session_type: {other}")),
    };

    let claude_mode = match session_mode.as_deref() {
        Some("auto") => crate::pty_manager::ClaudeMode::Auto,
        Some("skip") => crate::pty_manager::ClaudeMode::Skip,
        Some("plan") => crate::pty_manager::ClaudeMode::Plan,
        None => crate::pty_manager::ClaudeMode::Default,
        Some(other) => return Err(format!("Unknown session_mode: {other}")),
    };

    let is_git_repo = match session_type {
        crate::pty_manager::SessionType::Claude => match is_git_repo {
            Some(v) => v,
            None => return Err("is_git_repo is required for Claude sessions".into()),
        },
        crate::pty_manager::SessionType::Terminal => is_git_repo.unwrap_or(false),
    };

    if pull_latest.unwrap_or(false) && session_type == crate::pty_manager::SessionType::Claude {
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
            let trackers = state.status_trackers.lock()
                .map_err(|e| format!("Failed to lock status trackers: {e}"))?;
            Ok(entries.into_iter().map(|e| {
                let worktree_cwd = trackers
                    .get(&e.id)
                    .and_then(|t| t.worktree_cwd().map(|s| s.to_string()));
                SessionInfo::from_entry(e, worktree_cwd)
            }).collect())
        }
        PtyResponse::Error(msg) => Err(msg),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Internal helper — not exposed as an IPC command.
/// Intentionally not a #[tauri::command]; pulling is now coupled to session
/// creation so the frontend cannot trigger pulls without spawning a session.
fn git_pull_main_internal(path: &std::path::Path) -> Result<(), String> {
    let display_path = path.display();

    let run_git = |args: &[&str]| -> Result<std::process::Output, String> {
        std::process::Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    format!("git is not installed or not in PATH (needed for {display_path})")
                } else {
                    format!("Failed to run git {} in {display_path}: {e}", args.join(" "))
                }
            })
    };

    // Detect the remote's default branch (main, master, etc.).
    let default_branch = run_git(&["symbolic-ref", "refs/remotes/origin/HEAD", "--short"])
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                s.strip_prefix("origin/").map(|b| b.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "main".to_string());

    // Capture original branch so we can restore on failure.
    let original_branch = run_git(&["rev-parse", "--abbrev-ref", "HEAD"])?;
    let original_branch = String::from_utf8_lossy(&original_branch.stdout).trim().to_string();

    let checkout = run_git(&["checkout", &default_branch])?;

    if !checkout.status.success() {
        let stderr = String::from_utf8_lossy(&checkout.stderr);
        return Err(format!("git checkout {default_branch} failed in {display_path}: {stderr}"));
    }

    let pull = run_git(&["pull", "origin", &default_branch])?;

    if !pull.status.success() {
        let stderr = String::from_utf8_lossy(&pull.stderr);
        // Attempt to restore the original branch since checkout succeeded but pull failed.
        if !original_branch.is_empty() && original_branch != default_branch {
            let _ = run_git(&["checkout", &original_branch]);
        }
        return Err(format!(
            "git pull origin {default_branch} failed in {display_path}: {stderr}"
        ));
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
pub fn get_session_worktree_cwd(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<String>, String> {
    let trackers = state
        .status_trackers
        .lock()
        .map_err(|e| format!("Failed to lock status trackers: {e}"))?;
    Ok(trackers.get(&id).and_then(|t| t.worktree_cwd().map(|s| s.to_string())))
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
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "git is not installed or not in PATH".to_string()
            } else {
                format!("Failed to run git: {e}")
            }
        })?;

    Ok(output.status.success())
}

// ---------------------------------------------------------------------------
// Persistence IPC commands
// ---------------------------------------------------------------------------

use crate::persistence::{self, PersistedSession};

#[tauri::command]
pub fn save_sessions(
    state: State<'_, AppState>,
    sessions: Vec<PersistedSession>,
    scrollbacks: HashMap<String, String>,
) -> Result<(), String> {
    let _lock = state.persistence_lock.lock().unwrap();
    persistence::save_sessions(&state.persistence_dir, &sessions, &scrollbacks)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_single_session(
    state: State<'_, AppState>,
    session: PersistedSession,
    scrollback: String,
) -> Result<(), String> {
    let _lock = state.persistence_lock.lock().unwrap();
    persistence::save_single_session(&state.persistence_dir, &session, &scrollback)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_persisted_sessions(
    state: State<'_, AppState>,
) -> Vec<PersistedSession> {
    persistence::load_sessions(&state.persistence_dir)
}

#[tauri::command]
pub fn get_session_scrollback(
    state: State<'_, AppState>,
    session_id: String,
) -> Option<String> {
    persistence::load_scrollback(&state.persistence_dir, &session_id)
}

#[tauri::command]
pub fn delete_persisted_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let _lock = state.persistence_lock.lock().unwrap();
    persistence::delete_session(&state.persistence_dir, &session_id)
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct WorktreeRemoveResult {
    pub removed: bool,
    pub dirty: bool,
    pub message: String,
}

#[tauri::command]
pub fn remove_worktree(worktree_path: String, force: bool) -> Result<WorktreeRemoveResult, String> {
    let path = PathBuf::from(&worktree_path);
    if !path.exists() {
        return Ok(WorktreeRemoveResult {
            removed: true,
            dirty: false,
            message: "Worktree directory already removed".into(),
        });
    }

    // Check for uncommitted changes unless forcing
    if !force {
        let status_output = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&path)
            .output()
            .map_err(|e| format!("Failed to run git status: {e}"))?;

        if status_output.status.success() {
            let stdout = String::from_utf8_lossy(&status_output.stdout);
            if !stdout.trim().is_empty() {
                return Ok(WorktreeRemoveResult {
                    removed: false,
                    dirty: true,
                    message: "Worktree has uncommitted changes".into(),
                });
            }
        }
    }

    // Run git worktree remove from the worktree's parent repo
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(&worktree_path);

    // We need to run from the main repo root, not from the worktree itself.
    // Derive it by resolving the git common dir.
    let common_dir_output = std::process::Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(&path)
        .output()
        .map_err(|e| format!("Failed to find git common dir: {e}"))?;

    let repo_root = if common_dir_output.status.success() {
        let git_dir = String::from_utf8_lossy(&common_dir_output.stdout).trim().to_string();
        // git common dir is typically <repo>/.git — parent is the repo root
        let git_path = PathBuf::from(&git_dir);
        if git_path.is_absolute() {
            git_path.parent().unwrap_or(&git_path).to_path_buf()
        } else {
            // Relative path — resolve from the worktree cwd
            let resolved = path.join(&git_path);
            resolved.parent().unwrap_or(&resolved).to_path_buf()
        }
    } else {
        // Fallback: try running from the worktree path itself
        path.clone()
    };

    let remove_output = std::process::Command::new("git")
        .args(&args)
        .current_dir(&repo_root)
        .output()
        .map_err(|e| format!("Failed to run git worktree remove: {e}"))?;

    if remove_output.status.success() {
        Ok(WorktreeRemoveResult {
            removed: true,
            dirty: false,
            message: "Worktree removed successfully".into(),
        })
    } else {
        let stderr = String::from_utf8_lossy(&remove_output.stderr).trim().to_string();
        Err(format!("git worktree remove failed: {stderr}"))
    }
}

#[tauri::command]
pub fn quit_app(app_handle: tauri::AppHandle) {
    app_handle.exit(0);
}

#[tauri::command]
pub fn save_dropped_image(data: Vec<u8>, extension: String) -> Result<String, String> {
    const ALLOWED: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "tiff"];
    let ext = extension.to_lowercase();
    if !ALLOWED.contains(&ext.as_str()) {
        return Err(format!("Unsupported image extension: {extension}"));
    }
    let filename = format!("ao-dropped-{}.{}", uuid::Uuid::new_v4(), ext);
    let path = std::env::temp_dir().join(&filename);
    std::fs::write(&path, &data)
        .map_err(|e| format!("Failed to write temp image: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}
