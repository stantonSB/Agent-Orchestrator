// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // macOS aborts forked children in multi-threaded processes when the
    // Objective-C runtime detects +initialize methods were in flight during
    // fork(). portable-pty's PTY spawn uses fork+exec (forced by pre_exec),
    // so every session creation is a potential crash. Setting this env var
    // before any Obj-C code runs suppresses that abort and lets the child
    // reach exec() safely.
    //
    // Safety: called before any threads are spawned, so set_var is safe.
    #[cfg(target_os = "macos")]
    unsafe {
        std::env::set_var("OBJC_DISABLE_INITIALIZE_FORK_SAFETY", "YES");
    }

    agent_orchestrator_lib::run()
}
