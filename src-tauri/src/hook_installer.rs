use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Result of the hook installation check/install process.
#[derive(Debug, PartialEq)]
pub enum HookInstallResult {
    AlreadyInstalled,
    Installed,
    Failed(String),
}

const HOOK_SCRIPT: &str = r#"#!/bin/bash
# Forward Claude Code notifications to Agent Orchestrator.
# No-ops silently when Agent Orchestrator is not running.
if [ -n "$AO_STATUS_PORT" ] && [ -n "$AO_SESSION_ID" ]; then
    curl -s -X POST "http://127.0.0.1:${AO_STATUS_PORT}/status/${AO_SESSION_ID}" \
        -H "Content-Type: application/json" \
        -H "X-Cwd: $(pwd)" \
        -d @- 2>/dev/null || true
fi
"#;

const IDLE_THRESHOLD_MS: u64 = 500;

/// Hook events forwarded to the status server. Notification drives
/// Working/Idle/NeedsAttention transitions, Stop fires immediately on task
/// completion, SubagentStart/SubagentStop track subagent lifecycles, and
/// PreToolUse exists solely for early worktree CWD discovery via the
/// script's X-Cwd header.
const HOOK_EVENTS: [&str; 5] = [
    "Notification",
    "Stop",
    "SubagentStop",
    "SubagentStart",
    "PreToolUse",
];

const HOOK_SCRIPT_NEEDLE: &str = "agent-orchestrator-notify";

/// Settings JSON passed to AO-spawned Claude sessions via `--settings`.
/// Injecting the hooks per-session keeps them out of the user's global
/// ~/.claude/settings.json, where they would fire in every Claude Code
/// session on the machine — AO-launched or not.
pub fn session_hook_settings() -> String {
    let entry = serde_json::json!({
        "matcher": "",
        "hooks": [
            {
                "type": "command",
                "command": "${HOME}/.claude/agent-orchestrator-notify.sh"
            }
        ]
    });
    let mut hooks = serde_json::Map::new();
    for event in HOOK_EVENTS {
        hooks.insert(event.to_string(), serde_json::Value::Array(vec![entry.clone()]));
    }
    serde_json::json!({ "hooks": hooks }).to_string()
}

/// Ensure the notify script and idle threshold are in place, and remove
/// hooks that older versions merged into the global settings.json (hooks
/// are injected per-session via `--settings` now — see
/// `session_hook_settings`). Returns the result of the operation.
pub fn ensure_hooks_installed() -> HookInstallResult {
    let home = match std::env::var("HOME") {
        Ok(h) => PathBuf::from(h),
        Err(_) => return HookInstallResult::Failed("HOME environment variable not set".to_string()),
    };
    ensure_hooks_installed_in(&home)
}

/// Internal implementation accepting a base path for testability.
pub fn ensure_hooks_installed_in(home: &Path) -> HookInstallResult {
    let claude_dir = home.join(".claude");
    let script_path = claude_dir.join("agent-orchestrator-notify.sh");
    let settings_path = claude_dir.join("settings.json");
    let profile_path = home.join(".claude.json");

    if is_already_installed(&script_path, &settings_path, &profile_path) {
        return HookInstallResult::AlreadyInstalled;
    }

    // Step 1: Ensure ~/.claude/ directory exists
    if let Err(e) = fs::create_dir_all(&claude_dir) {
        return HookInstallResult::Failed(format!("Failed to create ~/.claude directory: {e}"));
    }

    // Step 2: Write hook script
    if let Err(e) = write_hook_script(&script_path) {
        return HookInstallResult::Failed(format!("Failed to write hook script: {e}"));
    }

    // Step 3: Remove hooks that older versions merged into the global
    // settings.json — they are injected per-session via --settings now.
    if let Err(e) = remove_hook_settings(&settings_path) {
        return HookInstallResult::Failed(format!("Failed to update settings.json: {e}"));
    }

    // Step 4: Set idle threshold in ~/.claude.json
    if let Err(e) = set_idle_threshold(&profile_path) {
        return HookInstallResult::Failed(format!("Failed to update .claude.json: {e}"));
    }

    HookInstallResult::Installed
}

fn script_has_current_content(path: &Path) -> bool {
    match fs::read_to_string(path) {
        Ok(content) => content == HOOK_SCRIPT,
        Err(_) => false,
    }
}

fn is_already_installed(script_path: &Path, settings_path: &Path, profile_path: &Path) -> bool {
    // Check script exists and is executable
    if !script_path.exists() {
        return false;
    }
    if let Ok(meta) = fs::metadata(script_path) {
        if meta.permissions().mode() & 0o111 == 0 {
            return false;
        }
    } else {
        return false;
    }

    // Check script content is up-to-date
    if !script_has_current_content(script_path) {
        return false;
    }

    // Check settings.json is free of the hooks older versions installed
    // globally — if any remain, the migration in remove_hook_settings
    // still needs to run.
    if settings_has_any_of_our_hooks(settings_path) {
        return false;
    }

    // Check .claude.json has idle threshold
    if !profile_has_idle_threshold(profile_path) {
        return false;
    }

    true
}

fn settings_has_any_of_our_hooks(settings_path: &Path) -> bool {
    let content = match fs::read_to_string(settings_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let val: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };
    HOOK_EVENTS
        .iter()
        .any(|event| hook_array_has_our_script(&val, event))
}

fn hook_array_has_our_script(val: &serde_json::Value, hook_type: &str) -> bool {
    let pointer = format!("/hooks/{}", hook_type);
    let entries = match val.pointer(&pointer) {
        Some(serde_json::Value::Array(arr)) => arr,
        _ => return false,
    };
    for entry in entries {
        let entry_str = entry.to_string();
        if entry_str.contains(HOOK_SCRIPT_NEEDLE) {
            return true;
        }
    }
    false
}

fn profile_has_idle_threshold(profile_path: &Path) -> bool {
    let content = match fs::read_to_string(profile_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let val: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };
    val.get("messageIdleNotifThresholdMs").is_some()
}

fn write_hook_script(script_path: &Path) -> std::io::Result<()> {
    fs::write(script_path, HOOK_SCRIPT)?;
    let mut perms = fs::metadata(script_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(script_path, perms)?;
    Ok(())
}

/// Remove hook entries that older versions merged into the user's global
/// settings.json. Only entries referencing our notify script are touched;
/// everything else in the file is preserved verbatim.
fn remove_hook_settings(settings_path: &Path) -> Result<(), String> {
    if !settings_path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(settings_path)
        .map_err(|e| format!("read error: {e}"))?;
    let mut val: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        // A malformed settings.json has nothing of ours to remove; leave
        // it for the user rather than clobbering it.
        Err(_) => return Ok(()),
    };

    let mut changed = false;
    let mut hooks_now_empty = false;
    if let Some(hooks_obj) = val.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        for event in HOOK_EVENTS {
            let emptied = match hooks_obj.get_mut(event).and_then(|e| e.as_array_mut()) {
                Some(entries) => {
                    let before = entries.len();
                    entries.retain(|entry| !entry.to_string().contains(HOOK_SCRIPT_NEEDLE));
                    changed |= entries.len() != before;
                    before > 0 && entries.is_empty()
                }
                None => false,
            };
            if emptied {
                hooks_obj.remove(event);
            }
        }
        hooks_now_empty = changed && hooks_obj.is_empty();
    }
    if hooks_now_empty {
        if let Some(root) = val.as_object_mut() {
            root.remove("hooks");
        }
    }

    if changed {
        let serialized = serde_json::to_string_pretty(&val)
            .map_err(|e| format!("serialization error: {e}"))?;
        fs::write(settings_path, serialized)
            .map_err(|e| format!("write error: {e}"))?;
    }

    Ok(())
}

fn set_idle_threshold(profile_path: &Path) -> Result<(), String> {
    let mut val: serde_json::Value = if profile_path.exists() {
        let content = fs::read_to_string(profile_path)
            .map_err(|e| format!("read error: {e}"))?;
        serde_json::from_str(&content)
            .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()))
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    // Only set if not already present
    if val.get("messageIdleNotifThresholdMs").is_none() {
        val.as_object_mut()
            .ok_or("~/.claude.json root is not an object")?
            .insert(
                "messageIdleNotifThresholdMs".to_string(),
                serde_json::Value::Number(IDLE_THRESHOLD_MS.into()),
            );

        let serialized = serde_json::to_string_pretty(&val)
            .map_err(|e| format!("serialization error: {e}"))?;
        fs::write(profile_path, serialized)
            .map_err(|e| format!("write error: {e}"))?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn temp_home() -> TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    fn claude_dir(home: &TempDir) -> PathBuf {
        home.path().join(".claude")
    }

    fn script_path(home: &TempDir) -> PathBuf {
        claude_dir(home).join("agent-orchestrator-notify.sh")
    }

    fn settings_path(home: &TempDir) -> PathBuf {
        claude_dir(home).join("settings.json")
    }

    fn profile_path(home: &TempDir) -> PathBuf {
        home.path().join(".claude.json")
    }

    /// Builds the settings.json shape older versions wrote: all five hook
    /// events pointing at the notify script.
    fn legacy_hooks_json() -> serde_json::Value {
        let entry = serde_json::json!({
            "matcher": "",
            "hooks": [
                { "type": "command", "command": "${HOME}/.claude/agent-orchestrator-notify.sh" }
            ]
        });
        let mut hooks = serde_json::Map::new();
        for event in HOOK_EVENTS {
            hooks.insert(event.to_string(), serde_json::Value::Array(vec![entry.clone()]));
        }
        serde_json::Value::Object(hooks)
    }

    #[test]
    fn test_fresh_install() {
        let home = temp_home();
        let result = ensure_hooks_installed_in(home.path());
        assert_eq!(result, HookInstallResult::Installed);

        // Script written and executable
        let sp = script_path(&home);
        assert!(sp.exists());
        let mode = fs::metadata(&sp).unwrap().permissions().mode();
        assert!(mode & 0o100 != 0, "script should be executable");

        // Global settings.json is NOT created — hooks are per-session now
        assert!(!settings_path(&home).exists());

        // .claude.json has idle threshold
        assert!(profile_has_idle_threshold(&profile_path(&home)));
    }

    #[test]
    fn test_already_installed() {
        let home = temp_home();
        // First install
        assert_eq!(
            ensure_hooks_installed_in(home.path()),
            HookInstallResult::Installed
        );
        // Second call
        assert_eq!(
            ensure_hooks_installed_in(home.path()),
            HookInstallResult::AlreadyInstalled
        );
    }

    #[test]
    fn test_idempotent_reinstall() {
        let home = temp_home();
        ensure_hooks_installed_in(home.path());
        // Run a third time for good measure
        ensure_hooks_installed_in(home.path());
        assert_eq!(
            ensure_hooks_installed_in(home.path()),
            HookInstallResult::AlreadyInstalled
        );
    }

    #[test]
    fn test_creates_claude_dir_if_missing() {
        let home = temp_home();
        assert!(!claude_dir(&home).exists());
        let result = ensure_hooks_installed_in(home.path());
        assert_eq!(result, HookInstallResult::Installed);
        assert!(claude_dir(&home).exists());
    }

    #[test]
    fn test_migration_removes_our_hooks_preserves_others() {
        let home = temp_home();
        fs::create_dir_all(claude_dir(&home)).unwrap();

        // Legacy install shape plus a foreign hook and an unrelated setting
        let mut hooks = legacy_hooks_json();
        hooks.as_object_mut().unwrap()["Notification"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "hooks": [{ "type": "command", "command": "/some/other/hook.sh" }]
            }));
        let existing = serde_json::json!({ "hooks": hooks, "otherSetting": true });
        fs::write(
            settings_path(&home),
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        let result = ensure_hooks_installed_in(home.path());
        assert_eq!(result, HookInstallResult::Installed);

        let content = fs::read_to_string(settings_path(&home)).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();

        // None of our hooks remain in any event
        for event in HOOK_EVENTS {
            assert!(
                !hook_array_has_our_script(&val, event),
                "{event} should no longer reference the notify script"
            );
        }

        // Foreign hook survives; events we emptied are dropped entirely
        let notifications = val.pointer("/hooks/Notification").unwrap().as_array().unwrap();
        assert_eq!(notifications.len(), 1);
        assert!(val.pointer("/hooks/Stop").is_none());
        assert!(val.pointer("/hooks/PreToolUse").is_none());

        // Other settings preserved
        assert_eq!(val["otherSetting"], serde_json::json!(true));

        // Second run has nothing left to do
        assert_eq!(
            ensure_hooks_installed_in(home.path()),
            HookInstallResult::AlreadyInstalled
        );
    }

    #[test]
    fn test_migration_drops_hooks_object_when_emptied() {
        let home = temp_home();
        fs::create_dir_all(claude_dir(&home)).unwrap();

        let existing = serde_json::json!({ "hooks": legacy_hooks_json(), "model": "opus" });
        fs::write(
            settings_path(&home),
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        ensure_hooks_installed_in(home.path());

        let content = fs::read_to_string(settings_path(&home)).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(val.get("hooks").is_none(), "emptied hooks object should be removed");
        assert_eq!(val["model"], serde_json::json!("opus"));
    }

    #[test]
    fn test_handles_missing_settings_json() {
        let home = temp_home();
        fs::create_dir_all(claude_dir(&home)).unwrap();
        // Write script and profile manually, skip settings to test missing case
        write_hook_script(&script_path(&home)).unwrap();
        set_idle_threshold(&profile_path(&home)).unwrap();

        // settings.json does not exist — nothing to migrate, nothing to write
        let result = ensure_hooks_installed_in(home.path());
        assert_eq!(result, HookInstallResult::AlreadyInstalled);
        assert!(!settings_path(&home).exists());
    }

    #[test]
    fn test_handles_malformed_settings_json() {
        let home = temp_home();
        fs::create_dir_all(claude_dir(&home)).unwrap();
        // Write malformed JSON
        let malformed = b"{ this is not valid json !!!";
        fs::write(settings_path(&home), malformed).unwrap();

        let result = ensure_hooks_installed_in(home.path());
        assert_eq!(result, HookInstallResult::Installed);

        // Malformed file is left untouched — we only remove, never rewrite
        let content = fs::read(settings_path(&home)).unwrap();
        assert_eq!(content, malformed);
        let bak = home.path().join(".claude").join("settings.json.bak");
        assert!(!bak.exists());
    }

    #[test]
    fn test_session_hook_settings_covers_all_events() {
        let val: serde_json::Value =
            serde_json::from_str(&session_hook_settings()).expect("valid JSON");

        let root = val.as_object().unwrap();
        assert_eq!(root.len(), 1, "only the hooks key should be present");

        for event in HOOK_EVENTS {
            let entries = val
                .pointer(&format!("/hooks/{event}"))
                .and_then(|e| e.as_array())
                .unwrap_or_else(|| panic!("{event} missing from session settings"));
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0]["matcher"], serde_json::json!(""));
            let command = entries[0]["hooks"][0]["command"].as_str().unwrap();
            assert!(command.contains(HOOK_SCRIPT_NEEDLE));
        }
    }

    #[test]
    fn test_hook_script_contains_x_cwd_header() {
        assert!(
            HOOK_SCRIPT.contains("X-Cwd"),
            "hook script should include X-Cwd header"
        );
    }

    #[test]
    fn test_outdated_script_content_triggers_reinstall() {
        let home = temp_home();
        fs::create_dir_all(claude_dir(&home)).unwrap();

        // Write an old-format script (no X-Cwd header)
        let old_script = r#"#!/bin/bash
if [ -n "$AO_STATUS_PORT" ] && [ -n "$AO_SESSION_ID" ]; then
    curl -s -X POST "http://127.0.0.1:${AO_STATUS_PORT}/status/${AO_SESSION_ID}" \
        -H "Content-Type: application/json" -d @- 2>/dev/null || true
fi
"#;
        fs::write(script_path(&home), old_script).unwrap();
        let mut perms = fs::metadata(script_path(&home)).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(script_path(&home), perms).unwrap();

        // Install profile so that check passes
        set_idle_threshold(&profile_path(&home)).unwrap();

        // Should detect outdated script and reinstall
        let result = ensure_hooks_installed_in(home.path());
        assert_eq!(result, HookInstallResult::Installed);

        // Verify new script content
        let content = fs::read_to_string(script_path(&home)).unwrap();
        assert!(content.contains("X-Cwd"), "updated script should contain X-Cwd header");
    }

    #[test]
    fn test_migration_handles_partial_legacy_install() {
        let home = temp_home();
        fs::create_dir_all(claude_dir(&home)).unwrap();

        // Pre-populate with only the Notification hook (oldest install shape)
        let existing = serde_json::json!({
            "hooks": {
                "Notification": [
                    {
                        "matcher": "",
                        "hooks": [
                            { "type": "command", "command": "${HOME}/.claude/agent-orchestrator-notify.sh" }
                        ]
                    }
                ]
            }
        });
        fs::write(
            settings_path(&home),
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();
        // Also write script and profile so those checks pass
        write_hook_script(&script_path(&home)).unwrap();
        set_idle_threshold(&profile_path(&home)).unwrap();

        // Should detect the leftover hook and remove it
        let result = ensure_hooks_installed_in(home.path());
        assert_eq!(result, HookInstallResult::Installed);

        assert!(!settings_has_any_of_our_hooks(&settings_path(&home)));
    }
}
