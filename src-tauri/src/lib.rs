pub mod commands;
pub mod hook_installer;
pub mod persistence;
pub mod pty_manager;
pub mod state;
pub mod status_parser;
pub mod status_server;
pub mod subagent_tracker;

#[cfg(test)]
mod status_parser_tests;
#[cfg(test)]
mod subagent_tracker_tests;

use state::AppState;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Capture the user's login-shell environment early, before the Tauri
    // runtime spawns its threads.  The capture itself does fork+exec, so
    // running it now (while the process is still mostly single-threaded)
    // avoids the macOS "multi-threaded process forked" crash.
    pty_manager::warm_shell_env();

    tauri::Builder::default()
        .setup(|app| {
            let handle_for_output = app.handle().clone();
            let handle_for_exit = app.handle().clone();
            let handle_for_status = app.handle().clone();
            let handle_for_hook = app.handle().clone();
            let handle_for_subagents = app.handle().clone();

            // Install Claude Code notification hooks. Emit a warning event if
            // installation fails, but do not block startup.
            match hook_installer::ensure_hooks_installed() {
                hook_installer::HookInstallResult::Installed => {
                    eprintln!("[agent-orchestrator] Hook scripts installed.");
                }
                hook_installer::HookInstallResult::AlreadyInstalled => {
                    // Nothing to do.
                }
                hook_installer::HookInstallResult::Failed(err) => {
                    eprintln!("[agent-orchestrator] Hook installation failed: {err}");
                    let _ = handle_for_hook.emit("hook-setup-failed", serde_json::json!({ "error": err }));
                }
            }

            let on_output: pty_manager::OutputCallback =
                Box::new(move |id, data| {
                    let event_name = format!("session-output-{}", id);
                    let _ = handle_for_output.emit(
                        &event_name,
                        serde_json::json!({ "data": data }),
                    );
                });

            let on_exit: pty_manager::ExitCallback =
                Box::new(move |id, code| {
                    let event_name = format!("session-exit-{}", id);
                    let _ = handle_for_exit.emit(
                        &event_name,
                        serde_json::json!({ "code": code }),
                    );
                });

            let on_status: pty_manager::StatusCallback =
                Box::new(move |id, status| {
                    let event_name = format!("session-status-{}", id);
                    let _ = handle_for_status.emit(
                        &event_name,
                        serde_json::json!({ "status": status }),
                    );
                });

            let on_subagents: pty_manager::SubagentCallback =
                Box::new(move |id, payload| {
                    let event_name = format!("session-subagents-{}", id);
                    let _ = handle_for_subagents.emit(&event_name, payload);
                });

            // Create shared status trackers — used by both the PTY manager
            // (to insert/remove trackers per session) and the HTTP status
            // server (to receive hook events and update tracker state).
            let status_trackers: Arc<Mutex<HashMap<String, status_parser::StatusTracker>>> =
                Arc::new(Mutex::new(HashMap::new()));

            // Wrap on_status in an Arc so it can be shared with the status server.
            let on_status_arc: Arc<pty_manager::StatusCallback> = Arc::new(on_status);
            let on_status_for_server = on_status_arc.clone();

            // Start the HTTP status server. It receives POST requests from
            // Claude Code hook scripts and fires the on_status callback.
            let on_subagents_arc: Arc<pty_manager::SubagentCallback> = Arc::new(on_subagents);
            let (status_server, status_port) =
                status_server::StatusServer::start(status_trackers.clone(), on_status_for_server, on_subagents_arc);

            // Start the PTY manager, giving it the shared trackers and the
            // port so newly spawned sessions get the correct env vars.
            let status_trackers_for_state = status_trackers.clone();
            let pty_handle = pty_manager::start(
                on_output,
                on_exit,
                // The on_status callback is wrapped; we need to pass a plain Box.
                // Unwrap the Arc by cloning the inner closure via a thin wrapper.
                Box::new(move |id, status| on_status_arc(id, status)),
                status_trackers,
                status_port,
            );

            let persistence_dir = app.path().app_data_dir()
                .expect("failed to resolve app data dir")
                .join("persistence");

            app.manage(AppState {
                pty: pty_handle,
                status_server,
                status_trackers: status_trackers_for_state,
                persistence_dir,
                persistence_lock: Mutex::new(()),
            });

            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::create_session,
            commands::close_session,
            commands::write_to_session,
            commands::resize_session,
            commands::rename_session,
            commands::list_sessions,
            commands::check_is_git_repo,
            commands::get_session_status,
            commands::save_sessions,
            commands::save_single_session,
            commands::list_persisted_sessions,
            commands::get_session_scrollback,
            commands::delete_persisted_session,
            commands::save_dropped_image,
        ])
        .on_window_event(|_window, _event| {
            // Shutdown moved to RunEvent::Exit to allow frontend save-on-close
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    state.pty.shutdown();
                    state.status_server.stop();
                }
            }
        });
}
