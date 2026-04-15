pub mod commands;
pub mod pty_manager;
pub mod state;

use state::AppState;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle_for_output = app.handle().clone();
            let handle_for_exit = app.handle().clone();

            let on_output: pty_manager::OutputCallback =
                Box::new(move |id, data| {
                    let event_name = format!("session-output-{}", id);
                    let _ = handle_for_output.emit(&event_name, data);
                });

            let on_exit: pty_manager::ExitCallback =
                Box::new(move |id, code| {
                    let event_name = format!("session-exit-{}", id);
                    let _ = handle_for_exit.emit(&event_name, code);
                });

            let pty_handle = pty_manager::start(on_output, on_exit);
            app.manage(AppState { pty: pty_handle });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_session,
            commands::close_session,
            commands::write_to_session,
            commands::resize_session,
            commands::rename_session,
            commands::list_sessions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
