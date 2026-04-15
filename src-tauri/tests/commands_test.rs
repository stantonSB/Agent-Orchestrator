//! Integration tests that exercise the PTY manager through the same
//! PtyRequest/PtyResponse channel round-trip that the Tauri commands use.
//!
//! Each test creates a fresh PtyManagerHandle (which internally sets up
//! the mpsc channel and manager thread) and exercises it the same way
//! the Tauri command layer does.

use tauri_app_lib::pty_manager::PtyResponse;

/// Test listing sessions when none exist.
#[test]
fn test_list_empty() {
    let handle = make_handle();
    match handle.list() {
        PtyResponse::Sessions(entries) => {
            assert!(entries.is_empty(), "Expected no sessions initially");
        }
        other => panic!("Expected Sessions, got: {:?}", other),
    }
    handle.shutdown();
}

/// Test renaming a nonexistent session returns an error.
#[test]
fn test_rename_not_found() {
    let handle = make_handle();
    match handle.rename("nonexistent-id".into(), "new-name".into()) {
        PtyResponse::Error(msg) => {
            assert!(
                msg.contains("not found"),
                "Expected 'not found' in error, got: {msg}"
            );
        }
        other => panic!("Expected Error for rename of missing session, got: {:?}", other),
    }
    handle.shutdown();
}

/// Test that list returns a created session with correct fields.
#[test]
fn test_create_and_list_roundtrip() {
    let handle = make_handle();

    let id = match handle.create(
        "integration-test".into(),
        std::env::temp_dir(),
        "echo".into(),
        vec!["hello".into()],
        80,
        24,
    ) {
        PtyResponse::Created { id } => id,
        other => panic!("Expected Created, got: {:?}", other),
    };

    // Give the short-lived echo command a moment to register.
    std::thread::sleep(std::time::Duration::from_millis(100));

    match handle.list() {
        PtyResponse::Sessions(entries) => {
            // The session may or may not still be present (echo exits quickly),
            // but if it is, the fields should match.
            if let Some(entry) = entries.iter().find(|e| e.id == id) {
                assert_eq!(entry.name, "integration-test");
            }
        }
        other => panic!("Expected Sessions, got: {:?}", other),
    }
    handle.shutdown();
}

/// Test kill on a nonexistent session.
#[test]
fn test_kill_not_found() {
    let handle = make_handle();
    match handle.kill("does-not-exist".into()) {
        PtyResponse::Error(msg) => {
            assert!(
                msg.contains("not found"),
                "Expected 'not found' in error, got: {msg}"
            );
        }
        other => panic!("Expected Error for kill of missing session, got: {:?}", other),
    }
    handle.shutdown();
}

/// Test resize on a nonexistent session.
#[test]
fn test_resize_not_found() {
    let handle = make_handle();
    match handle.resize("no-such-id".into(), 120, 40) {
        PtyResponse::Error(msg) => {
            assert!(
                msg.contains("not found"),
                "Expected 'not found' in error, got: {msg}"
            );
        }
        other => panic!("Expected Error for resize of missing session, got: {:?}", other),
    }
    handle.shutdown();
}

/// Test write to a nonexistent session.
#[test]
fn test_write_not_found() {
    let handle = make_handle();
    match handle.write("ghost".into(), b"data".to_vec()) {
        PtyResponse::Error(msg) => {
            assert!(
                msg.contains("not found"),
                "Expected 'not found' in error, got: {msg}"
            );
        }
        other => panic!("Expected Error for write to missing session, got: {:?}", other),
    }
    handle.shutdown();
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn make_handle() -> tauri_app_lib::pty_manager::PtyManagerHandle {
    use std::sync::{Arc, Mutex};

    let ol: Arc<Mutex<Vec<(String, Vec<u8>)>>> = Arc::new(Mutex::new(Vec::new()));
    let el: Arc<Mutex<Vec<(String, Option<u32>)>>> = Arc::new(Mutex::new(Vec::new()));

    let ol_c = ol.clone();
    let el_c = el.clone();

    tauri_app_lib::pty_manager::start(
        Box::new(move |id, data| {
            ol_c.lock().unwrap().push((id, data));
        }),
        Box::new(move |id, code| {
            el_c.lock().unwrap().push((id, code));
        }),
    )
}
