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

    let builder = tauri::Builder::default();

    // On macOS the default app menu's Quit item sends the native
    // `terminate:` selector, which kills the process without ever firing
    // RunEvent::ExitRequested — so Cmd+Q bypassed the quit confirmation
    // dialog. Replace it with a custom item that routes through the same
    // "quit-requested" event the frontend already listens for.
    #[cfg(target_os = "macos")]
    let builder = builder
        .menu(|handle| {
            let menu = tauri::menu::Menu::default(handle)?;
            if let Some(tauri::menu::MenuItemKind::Submenu(app_menu)) =
                menu.items()?.into_iter().next()
            {
                let items = app_menu.items()?;
                for (idx, item) in items.iter().enumerate() {
                    if let tauri::menu::MenuItemKind::Predefined(predefined) = item {
                        if predefined.text()?.starts_with("Quit") {
                            app_menu.remove_at(idx)?;
                            break;
                        }
                    }
                }
                let quit_item = tauri::menu::MenuItem::with_id(
                    handle,
                    "request-quit",
                    format!("Quit {}", handle.package_info().name),
                    true,
                    Some("CmdOrCtrl+Q"),
                )?;
                app_menu.append(&quit_item)?;
            }
            Ok(menu)
        })
        .on_menu_event(|app_handle, event| {
            if event.id().as_ref() == "request-quit" {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("quit-requested", ());
                }
            }
        });

    builder
        .setup(|app| {
            let handle_for_output = app.handle().clone();
            let handle_for_exit = app.handle().clone();
            let handle_for_status = app.handle().clone();
            let handle_for_hook = app.handle().clone();
            let handle_for_subagents = app.handle().clone();
            let handle_for_worktree_cwd = app.handle().clone();

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
                    // Ship raw PTY bytes as a single base64 string rather than
                    // a JSON array of integers. The array form bloated each
                    // chunk ~3-4x and forced a slow number[]-parse on the JS
                    // side; base64 is one compact string with a fast decode.
                    use base64::Engine as _;
                    let event_name = format!("session-output-{}", id);
                    let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
                    let _ = handle_for_output.emit(&event_name, encoded);
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

            let on_worktree_cwd: pty_manager::WorktreeCwdCallback =
                Box::new(move |id, cwd| {
                    let event_name = format!("session-worktree-cwd-{}", id);
                    let _ = handle_for_worktree_cwd.emit(
                        &event_name,
                        serde_json::json!({ "worktreeCwd": cwd }),
                    );
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
            let on_worktree_cwd_arc: Arc<pty_manager::WorktreeCwdCallback> = Arc::new(on_worktree_cwd);
            let (status_server, status_port) =
                status_server::StatusServer::start(status_trackers.clone(), on_status_for_server, on_subagents_arc, on_worktree_cwd_arc);

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
            commands::get_session_worktree_cwd,
            commands::save_sessions,
            commands::save_single_session,
            commands::list_persisted_sessions,
            commands::get_session_scrollback,
            commands::delete_persisted_session,
            commands::save_dropped_image,
            commands::remove_worktree,
            commands::quit_app,
        ])
        .on_window_event(|_window, _event| {
            // Shutdown moved to RunEvent::Exit to allow frontend save-on-close
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            match event {
                tauri::RunEvent::ExitRequested { api, code, .. } => {
                    // A confirmed quit goes through quit_app -> exit(0), which
                    // sets `code` — let it through, or the app could never
                    // exit. Only window-close-driven requests (code: None)
                    // get redirected to the confirmation dialog.
                    if code.is_none() {
                        api.prevent_exit();
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.emit("quit-requested", ());
                        }
                    }
                }
                tauri::RunEvent::Exit => {
                    if let Some(state) = app_handle.try_state::<AppState>() {
                        state.pty.shutdown();
                        state.status_server.stop();
                    }
                }
                _ => {}
            }
        });
}
