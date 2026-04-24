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
        -H "Content-Type: application/json" -d @- 2>/dev/null || true
fi
"#;

const IDLE_THRESHOLD_MS: u64 = 500;

/// Check if hooks are installed. If not, install them.
/// Returns the result of the operation.
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

    // Step 3: Merge hook config into settings.json
    if let Err(e) = merge_hook_settings(&settings_path) {
        return HookInstallResult::Failed(format!("Failed to update settings.json: {e}"));
    }

    // Step 4: Set idle threshold in ~/.claude.json
    if let Err(e) = set_idle_threshold(&profile_path) {
        return HookInstallResult::Failed(format!("Failed to update .claude.json: {e}"));
    }

    HookInstallResult::Installed
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

    // Check settings.json contains our Notification and Stop hooks
    if !settings_has_our_hook(settings_path) {
        return false;
    }
    if !settings_has_our_stop_hook(settings_path) {
        return false;
    }
    if !settings_has_our_subagent_stop_hook(settings_path) {
        return false;
    }
    if !settings_has_our_subagent_start_hook(settings_path) {
        return false;
    }

    // Check .claude.json has idle threshold
    if !profile_has_idle_threshold(profile_path) {
        return false;
    }

    true
}

fn settings_has_our_hook(settings_path: &Path) -> bool {
    let content = match fs::read_to_string(settings_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let val: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };
    notification_array_has_our_hook(&val)
}

fn settings_has_our_stop_hook(settings_path: &Path) -> bool {
    let content = match fs::read_to_string(settings_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let val: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };
    hook_array_has_our_script(&val, "Stop")
}

fn settings_has_our_subagent_stop_hook(settings_path: &Path) -> bool {
    let content = match fs::read_to_string(settings_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let val: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };
    hook_array_has_our_script(&val, "SubagentStop")
}

fn settings_has_our_subagent_start_hook(settings_path: &Path) -> bool {
    let content = match fs::read_to_string(settings_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let val: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };
    hook_array_has_our_script(&val, "SubagentStart")
}

fn hook_array_has_our_script(val: &serde_json::Value, hook_type: &str) -> bool {
    let pointer = format!("/hooks/{}", hook_type);
    let entries = match val.pointer(&pointer) {
        Some(serde_json::Value::Array(arr)) => arr,
        _ => return false,
    };
    let needle = "agent-orchestrator-notify";
    for entry in entries {
        let entry_str = entry.to_string();
        if entry_str.contains(needle) {
            return true;
        }
    }
    false
}

fn notification_array_has_our_hook(val: &serde_json::Value) -> bool {
    hook_array_has_our_script(val, "Notification")
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

fn merge_hook_settings(settings_path: &Path) -> Result<(), String> {
    let mut val: serde_json::Value = if settings_path.exists() {
        let content = fs::read_to_string(settings_path)
            .map_err(|e| format!("read error: {e}"))?;
        match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => {
                // Back up malformed file and start fresh
                let bak = settings_path.with_extension("json.bak");
                fs::copy(settings_path, &bak)
                    .map_err(|e| format!("backup error: {e}"))?;
                serde_json::Value::Object(serde_json::Map::new())
            }
        }
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    // Check what's missing before taking mutable borrows
    let need_notification = !notification_array_has_our_hook(&val);
    let need_stop = !hook_array_has_our_script(&val, "Stop");
    let need_subagent_stop = !hook_array_has_our_script(&val, "SubagentStop");
    let need_subagent_start = !hook_array_has_our_script(&val, "SubagentStart");

    if !need_notification && !need_stop && !need_subagent_stop && !need_subagent_start {
        return Ok(());
    }

    let our_hook_entry = serde_json::json!({
        "matcher": "",
        "hooks": [
            {
                "type": "command",
                "command": "${HOME}/.claude/agent-orchestrator-notify.sh"
            }
        ]
    });

    // Ensure hooks object exists
    let hooks = val
        .as_object_mut()
        .ok_or("settings.json root is not an object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

    let hooks_obj = hooks
        .as_object_mut()
        .ok_or("hooks is not an object")?;

    // Add Notification hook if missing
    if need_notification {
        let notifications = hooks_obj
            .entry("Notification")
            .or_insert_with(|| serde_json::Value::Array(vec![]));
        notifications
            .as_array_mut()
            .ok_or("Notification is not an array")?
            .push(our_hook_entry.clone());
    }

    // Add Stop hook if missing — fires immediately when Claude finishes,
    // unlike idle_prompt which has a hardcoded 60-second delay.
    if need_stop {
        let stop_hooks = hooks_obj
            .entry("Stop")
            .or_insert_with(|| serde_json::Value::Array(vec![]));
        stop_hooks
            .as_array_mut()
            .ok_or("Stop is not an array")?
            .push(our_hook_entry.clone());
    }

    // Add SubagentStop hook if missing — fires when a subagent finishes,
    // enabling status tracking for nested subagent sessions.
    if need_subagent_stop {
        let subagent_stop_hooks = hooks_obj
            .entry("SubagentStop")
            .or_insert_with(|| serde_json::Value::Array(vec![]));
        subagent_stop_hooks
            .as_array_mut()
            .ok_or("SubagentStop is not an array")?
            .push(our_hook_entry.clone());
    }

    // Add SubagentStart hook if missing — fires when a subagent spawns,
    // enabling real-time tracking of subagent lifecycle from the start.
    if need_subagent_start {
        let subagent_start_hooks = hooks_obj
            .entry("SubagentStart")
            .or_insert_with(|| serde_json::Value::Array(vec![]));
        subagent_start_hooks
            .as_array_mut()
            .ok_or("SubagentStart is not an array")?
            .push(our_hook_entry.clone());
    }

    let serialized = serde_json::to_string_pretty(&val)
        .map_err(|e| format!("serialization error: {e}"))?;
    fs::write(settings_path, serialized)
        .map_err(|e| format!("write error: {e}"))?;

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

        // settings.json has both Notification and Stop hook entries
        assert!(settings_has_our_hook(&settings_path(&home)));
        assert!(settings_has_our_stop_hook(&settings_path(&home)));

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
    fn test_merges_with_existing_settings() {
        let home = temp_home();
        fs::create_dir_all(claude_dir(&home)).unwrap();

        // Pre-populate settings with an existing hook entry
        let existing = serde_json::json!({
            "hooks": {
                "Notification": [
                    {
                        "hooks": [
                            { "type": "command", "command": "/some/other/hook.sh" }
                        ]
                    }
                ]
            },
            "otherSetting": true
        });
        fs::write(
            settings_path(&home),
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        ensure_hooks_installed_in(home.path());

        let content = fs::read_to_string(settings_path(&home)).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Our Notification and Stop hooks are present
        assert!(notification_array_has_our_hook(&val));
        assert!(hook_array_has_our_script(&val, "Stop"));

        // Existing hook is preserved
        let notifications = val.pointer("/hooks/Notification").unwrap().as_array().unwrap();
        assert_eq!(notifications.len(), 2);

        // Stop hook added
        let stop_hooks = val.pointer("/hooks/Stop").unwrap().as_array().unwrap();
        assert_eq!(stop_hooks.len(), 1);

        // Other settings preserved
        assert_eq!(val["otherSetting"], serde_json::json!(true));
    }

    #[test]
    fn test_handles_missing_settings_json() {
        let home = temp_home();
        fs::create_dir_all(claude_dir(&home)).unwrap();
        // Write script and profile manually, skip settings to test missing case
        write_hook_script(&script_path(&home)).unwrap();
        set_idle_threshold(&profile_path(&home)).unwrap();

        // settings.json does not exist — installation should succeed
        let result = ensure_hooks_installed_in(home.path());
        assert_eq!(result, HookInstallResult::Installed);
        assert!(settings_path(&home).exists());
        assert!(settings_has_our_hook(&settings_path(&home)));
        assert!(settings_has_our_stop_hook(&settings_path(&home)));
    }

    #[test]
    fn test_handles_malformed_settings_json() {
        let home = temp_home();
        fs::create_dir_all(claude_dir(&home)).unwrap();
        // Write malformed JSON
        fs::write(settings_path(&home), b"{ this is not valid json !!!").unwrap();

        let result = ensure_hooks_installed_in(home.path());
        assert_eq!(result, HookInstallResult::Installed);

        // Backup created
        let bak = home.path().join(".claude").join("settings.json.bak");
        assert!(bak.exists());

        // New settings.json is valid and has both hooks
        assert!(settings_has_our_hook(&settings_path(&home)));
        assert!(settings_has_our_stop_hook(&settings_path(&home)));
    }

    #[test]
    fn test_subagent_stop_hook_installed() {
        let home = temp_home();
        ensure_hooks_installed_in(home.path());

        let content = fs::read_to_string(settings_path(&home)).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(hook_array_has_our_script(&val, "SubagentStop"));
    }

    #[test]
    fn test_adds_subagent_stop_when_only_notification_and_stop_exist() {
        let home = temp_home();
        fs::create_dir_all(claude_dir(&home)).unwrap();

        let existing = serde_json::json!({
            "hooks": {
                "Notification": [{
                    "matcher": "",
                    "hooks": [{ "type": "command", "command": "${HOME}/.claude/agent-orchestrator-notify.sh" }]
                }],
                "Stop": [{
                    "matcher": "",
                    "hooks": [{ "type": "command", "command": "${HOME}/.claude/agent-orchestrator-notify.sh" }]
                }]
            }
        });
        fs::write(settings_path(&home), serde_json::to_string_pretty(&existing).unwrap()).unwrap();
        write_hook_script(&script_path(&home)).unwrap();
        set_idle_threshold(&profile_path(&home)).unwrap();

        let result = ensure_hooks_installed_in(home.path());
        assert_eq!(result, HookInstallResult::Installed);
        assert!(hook_array_has_our_script(
            &serde_json::from_str::<serde_json::Value>(&fs::read_to_string(settings_path(&home)).unwrap()).unwrap(),
            "SubagentStop"
        ));
    }

    #[test]
    fn test_adds_stop_hook_when_only_notification_exists() {
        let home = temp_home();
        fs::create_dir_all(claude_dir(&home)).unwrap();

        // Pre-populate with only the Notification hook (simulates pre-fix installation)
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

        // Should detect missing Stop hook and install it
        let result = ensure_hooks_installed_in(home.path());
        assert_eq!(result, HookInstallResult::Installed);

        assert!(settings_has_our_stop_hook(&settings_path(&home)));
        // Notification hook still present
        assert!(settings_has_our_hook(&settings_path(&home)));
    }
}
