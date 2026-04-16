pub mod commands;
pub mod pty_manager;
pub mod state;
pub mod status_parser;

#[cfg(test)]
mod status_parser_tests;

use state::AppState;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle_for_output = app.handle().clone();
            let handle_for_exit = app.handle().clone();
            let handle_for_status = app.handle().clone();

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

            let pty_handle = pty_manager::start(on_output, on_exit, on_status);
            app.manage(AppState { pty: pty_handle });

            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::create_session,
            commands::close_session,
            commands::write_to_session,
            commands::resize_session,
            commands::rename_session,
            commands::list_sessions,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if let Some(state) = window.try_state::<AppState>() {
                    state.pty.shutdown();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
