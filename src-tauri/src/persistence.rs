use crate::pty_manager::SessionType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSession {
    pub id: String,
    pub name: String,
    pub cwd: String,
    pub session_type: SessionType,
    pub is_git_repo: bool,
    pub created_at_epoch_ms: u64,
    pub status_at_close: String,
}

/// Ensure persistence directories exist.
pub fn ensure_dirs(persistence_dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(persistence_dir.join("scrollback"))
}

/// Load all persisted sessions. Returns empty vec if file is missing or unreadable.
pub fn load_sessions(persistence_dir: &Path) -> Vec<PersistedSession> {
    let path = persistence_dir.join("sessions.json");
    match fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Load scrollback text for a single session. Returns None if file is missing.
pub fn load_scrollback(persistence_dir: &Path, session_id: &str) -> Option<String> {
    let path = scrollback_path(persistence_dir, session_id);
    fs::read_to_string(path).ok()
}

/// Atomically save sessions and their scrollback.
/// Merges with any already-persisted sessions (from save_single_session calls)
/// to avoid overwriting them.
pub fn save_sessions(
    persistence_dir: &Path,
    sessions: &[PersistedSession],
    scrollbacks: &HashMap<String, String>,
) -> std::io::Result<()> {
    ensure_dirs(persistence_dir)?;

    // Load existing persisted sessions and merge
    let mut existing = load_sessions(persistence_dir);
    let new_ids: std::collections::HashSet<&str> =
        sessions.iter().map(|s| s.id.as_str()).collect();
    // Keep existing sessions that are NOT being overwritten by this save
    existing.retain(|s| !new_ids.contains(s.id.as_str()));
    existing.extend(sessions.iter().cloned());

    let json = serde_json::to_string_pretty(&existing)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    atomic_write(&persistence_dir.join("sessions.json"), json.as_bytes())?;

    for (id, text) in scrollbacks {
        atomic_write(&scrollback_path(persistence_dir, id), text.as_bytes())?;
    }

    Ok(())
}

/// Append/update a single session in the persisted list and save its scrollback.
/// Caller must hold the persistence lock.
pub fn save_single_session(
    persistence_dir: &Path,
    session: &PersistedSession,
    scrollback: &str,
) -> std::io::Result<()> {
    ensure_dirs(persistence_dir)?;

    let mut sessions = load_sessions(persistence_dir);
    sessions.retain(|s| s.id != session.id);
    sessions.push(session.clone());

    let json = serde_json::to_string_pretty(&sessions)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    atomic_write(&persistence_dir.join("sessions.json"), json.as_bytes())?;

    atomic_write(
        &scrollback_path(persistence_dir, &session.id),
        scrollback.as_bytes(),
    )
}

/// Delete a persisted session and its scrollback file.
/// Caller must hold the persistence lock.
pub fn delete_session(persistence_dir: &Path, session_id: &str) -> std::io::Result<()> {
    let mut sessions = load_sessions(persistence_dir);
    sessions.retain(|s| s.id != session_id);

    let json = serde_json::to_string_pretty(&sessions)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    atomic_write(&persistence_dir.join("sessions.json"), json.as_bytes())?;

    let _ = fs::remove_file(scrollback_path(persistence_dir, session_id));
    Ok(())
}

fn scrollback_path(persistence_dir: &Path, session_id: &str) -> PathBuf {
    persistence_dir
        .join("scrollback")
        .join(format!("{}.txt", session_id))
}

fn atomic_write(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp_path = path.with_extension("tmp");
    let mut file = fs::File::create(&tmp_path)?;
    file.write_all(data)?;
    file.sync_all()?;
    fs::rename(&tmp_path, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(id: &str, name: &str) -> PersistedSession {
        PersistedSession {
            id: id.to_string(),
            name: name.to_string(),
            cwd: "/tmp/test".to_string(),
            session_type: SessionType::Claude,
            is_git_repo: true,
            created_at_epoch_ms: 1715000000000,
            status_at_close: "working".to_string(),
        }
    }

    #[test]
    fn test_save_and_load_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let pd = dir.path().join("persistence");

        let sessions = vec![make_session("aaa", "s1"), make_session("bbb", "s2")];
        let mut scrollbacks = HashMap::new();
        scrollbacks.insert("aaa".to_string(), "line1\nline2\n".to_string());
        scrollbacks.insert("bbb".to_string(), "output\n".to_string());

        save_sessions(&pd, &sessions, &scrollbacks).unwrap();

        let loaded = load_sessions(&pd);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "aaa");
        assert_eq!(loaded[1].id, "bbb");
        assert_eq!(
            load_scrollback(&pd, "aaa"),
            Some("line1\nline2\n".to_string())
        );
        assert_eq!(load_scrollback(&pd, "ccc"), None);
    }

    #[test]
    fn test_save_sessions_merges_with_existing() {
        let dir = tempfile::tempdir().unwrap();
        let pd = dir.path().join("persistence");

        // First, persist session "aaa" via save_single_session
        save_single_session(&pd, &make_session("aaa", "first"), "scroll-a").unwrap();

        // Then do a bulk save with session "bbb" only
        let sessions = vec![make_session("bbb", "second")];
        let mut scrollbacks = HashMap::new();
        scrollbacks.insert("bbb".to_string(), "scroll-b".to_string());
        save_sessions(&pd, &sessions, &scrollbacks).unwrap();

        // Both should exist
        let loaded = load_sessions(&pd);
        assert_eq!(loaded.len(), 2);
        let ids: Vec<&str> = loaded.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"aaa"));
        assert!(ids.contains(&"bbb"));
    }

    #[test]
    fn test_save_single_session() {
        let dir = tempfile::tempdir().unwrap();
        let pd = dir.path().join("persistence");

        save_single_session(&pd, &make_session("aaa", "first"), "scroll-1").unwrap();
        save_single_session(&pd, &make_session("bbb", "second"), "scroll-2").unwrap();

        let loaded = load_sessions(&pd);
        assert_eq!(loaded.len(), 2);
    }

    #[test]
    fn test_save_single_session_replaces_existing() {
        let dir = tempfile::tempdir().unwrap();
        let pd = dir.path().join("persistence");

        save_single_session(&pd, &make_session("aaa", "original"), "scroll-1").unwrap();

        let updated = PersistedSession {
            name: "updated".to_string(),
            status_at_close: "finished".to_string(),
            ..make_session("aaa", "")
        };
        save_single_session(&pd, &updated, "scroll-new").unwrap();

        let loaded = load_sessions(&pd);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "updated");
        assert_eq!(
            load_scrollback(&pd, "aaa"),
            Some("scroll-new".to_string())
        );
    }

    #[test]
    fn test_delete_session() {
        let dir = tempfile::tempdir().unwrap();
        let pd = dir.path().join("persistence");

        save_single_session(&pd, &make_session("aaa", "first"), "scroll-1").unwrap();
        save_single_session(&pd, &make_session("bbb", "second"), "scroll-2").unwrap();

        delete_session(&pd, "aaa").unwrap();

        let loaded = load_sessions(&pd);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "bbb");
        assert_eq!(load_scrollback(&pd, "aaa"), None);
        assert!(load_scrollback(&pd, "bbb").is_some());
    }

    #[test]
    fn test_load_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let pd = dir.path().join("nonexistent");
        assert!(load_sessions(&pd).is_empty());
        assert_eq!(load_scrollback(&pd, "xyz"), None);
    }
}
