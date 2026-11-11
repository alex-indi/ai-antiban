//! System proxy and CLI/IDE protection.
//!
//! Layout: `macos` / `windows` (platform PAC plumbing), `shell_rc` (zsh/bash/PowerShell blocks),
//! `ide` (editor settings.json), `ai_cli` (Claude Code / Gemini CLI settings),
//! `cli_protection` (enable/disable/status orchestration), `cleanup` (exit guard, signals, force clean).
//! Every previously public name is re-exported here, so external paths are unchanged.

mod ai_cli;
mod cleanup;
mod cli_protection;
mod ide;
#[cfg(target_os = "macos")]
mod macos;
mod shell_rc;
#[cfg(target_os = "windows")]
mod windows;

pub use ai_cli::{
    apply_claude_code_settings_file, apply_claude_code_settings_file_with_tz,
    apply_gemini_cli_settings_file, cleanup_claude_code_settings_file,
    cleanup_gemini_cli_settings_file,
};
pub use cleanup::{
    cleanup_all_persistent_settings, cleanup_legacy_windows_user_env, force_clean_all_os_proxy_traces,
    register_clean_exit_guard, spawn_signal_cleanup_watcher,
};
pub use cli_protection::{
    disable_cli_and_ide_runtime_settings, disable_cli_protection, enable_cli_and_ide_protection,
    enable_cli_and_ide_protection_with_tz, is_cli_protection_enabled,
};
pub use ide::{apply_ide_settings_file, apply_ide_settings_file_with_tz, cleanup_ide_settings_file, sanitize_jsonc};
pub use shell_rc::{
    build_powershell_guard_block_with_tz, build_shell_rc_guard_block,
    build_shell_rc_guard_block_with_tz, get_windows_powershell_profile_paths, remove_shell_rc_block,
    sanitize_timezone, upsert_powershell_profile_content_with_tz, upsert_shell_rc_content,
    upsert_shell_rc_content_with_tz,
};
#[cfg(target_os = "windows")]
pub use self::windows::{broadcast_windows_env_change, delete_windows_user_env_var, set_windows_user_env_var};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static SYS_PROXY_ACTIVE: AtomicBool = AtomicBool::new(false);
static CLI_PROTECTION_ACTIVE: AtomicBool = AtomicBool::new(false);
static LAST_PAC_URL: Mutex<Option<String>> = Mutex::new(None);

const ZASTAVA_RC_START: &str = "# >>> Zastava AI CLI & IDE Guard >>>";
const ZASTAVA_RC_END: &str = "# <<< Zastava AI CLI & IDE Guard <<<";
const NO_PROXY_VALUE: &str = "localhost,127.0.0.1,::1,.local";

/// Spawns a Command configured on Windows with `CREATE_NO_WINDOW` (0x08000000)
/// to completely prevent cmd.exe / PowerShell / Terminal console windows flashing.
pub fn silent_command<S: AsRef<std::ffi::OsStr>>(program: S) -> std::process::Command {
    #[allow(unused_mut)]
    let mut cmd = std::process::Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SysproxyStatus {
    pub enabled: bool,
    pub pac_url: Option<String>,
    pub services_configured: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliProtectionReport {
    pub enabled: bool,
    pub proxy_url: String,
    pub configured_targets: Vec<String>,
    pub shell_command: String,
}

/// Disables systemic PAC proxy on all network services across macOS and Windows,
/// and thoroughly cleans up CLI, IDE, and OS environment proxy traces.
pub fn disable_all_sysproxy() {
    #[cfg(target_os = "macos")]
    {
        macos::disable_system_proxy();
    }

    #[cfg(target_os = "windows")]
    {
        windows::disable_system_proxy();
    }

    // Always clean up CLI, shell, PowerShell, IDE settings, and OS environment
    disable_cli_protection();
    cleanup_legacy_windows_user_env();

    SYS_PROXY_ACTIVE.store(false, Ordering::SeqCst);
    if let Ok(mut g) = LAST_PAC_URL.lock() {
        *g = None;
    }
}

/// Enables systemic PAC proxy with the given PAC URL (e.g. "http://127.0.0.1:8899/proxy.pac").
pub fn enable_sysproxy(pac_url: &str) -> Result<SysproxyStatus, String> {
    register_clean_exit_guard();
    let mut configured = Vec::new();


    #[cfg(target_os = "macos")]
    {
        macos::enable_system_proxy(pac_url, &mut configured)?;
    }

    #[cfg(target_os = "windows")]
    {
        windows::enable_system_proxy(pac_url, &mut configured);
    }

    SYS_PROXY_ACTIVE.store(true, Ordering::SeqCst);
    if let Ok(mut g) = LAST_PAC_URL.lock() {
        *g = Some(pac_url.to_string());
    }

    Ok(SysproxyStatus {
        enabled: true,
        pac_url: Some(pac_url.to_string()),
        services_configured: configured,
    })
}

/// Returns whether systemic PAC proxy is currently active.
pub fn is_sysproxy_active() -> bool {
    SYS_PROXY_ACTIVE.load(Ordering::SeqCst)
}

/// Re-applies the PAC configuration when a new network service appeared (e.g. a VPN client
/// added its own service) or when an external network tool / OS sleep-wake cycle disabled
/// or overwrote the PAC URL on the primary interface (Self-Healing).
/// Returns `true` when the configuration was re-applied.
pub fn refresh_if_services_changed() -> bool {
    if !is_sysproxy_active() {
        return false;
    }


    #[cfg(target_os = "macos")]
    {
        macos::refresh_if_changed()
    }

    #[cfg(target_os = "windows")]
    {
        windows::refresh_if_changed()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

pub struct SysProxy;

impl SysProxy {
    pub fn enable(pac_url: &str) -> Result<SysproxyStatus, String> {
        enable_sysproxy(pac_url)
    }

    pub fn disable() {
        disable_all_sysproxy();
    }

    pub fn is_active() -> bool {
        is_sysproxy_active()
    }
}

fn is_zastava_local_proxy_val(val: &str) -> bool {
    let v = val.trim().to_ascii_lowercase();
    v.contains("127.0.0.1:8899")
        || v.contains("127.0.0.1:8900")
        || v.contains("localhost:8899")
        || v.contains("localhost:8900")
}

pub(super) fn user_home_dir() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
}

/// Spawns the native OS date and time settings panel.
pub fn open_os_datetime_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        silent_command("open")
            .arg("x-apple.systempreferences:com.apple.preference.datetime")
            .spawn()
            .map_err(|e| format!("Failed to open macOS date & time settings: {e}"))?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        silent_command("cmd")
            .args(["/C", "start", "ms-settings:dateandtime"])
            .spawn()
            .map_err(|e| format!("Failed to open Windows date & time settings: {e}"))?;
        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_ide_and_claude_settings_apply_and_cleanup() {
        let tmp_dir = std::env::temp_dir().join(format!("zastava_ide_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&tmp_dir);
        let vscode_settings = tmp_dir.join("settings.json");

        // Write JSONC with comments and trailing comma
        let initial_jsonc = r#"{
            // User custom theme
            "workbench.colorTheme": "Default Dark+",
            "editor.fontSize": 14,
        }"#;
        fs::write(&vscode_settings, initial_jsonc).unwrap();

        assert!(apply_ide_settings_file(&vscode_settings, "http://127.0.0.1:8899").unwrap());
        let content = fs::read_to_string(&vscode_settings).unwrap();
        assert!(content.contains("\"http.proxy\": \"http://127.0.0.1:8899\""));
        assert!(content.contains("\"HTTPS_PROXY\": \"http://127.0.0.1:8899\""));
        assert!(content.contains("\"workbench.colorTheme\": \"Default Dark+\""));

        // Cleanup removes Zastava proxy keys while preserving user settings
        assert!(cleanup_ide_settings_file(&vscode_settings).unwrap());
        let cleaned = fs::read_to_string(&vscode_settings).unwrap();
        assert!(!cleaned.contains("127.0.0.1:8899"));
        assert!(cleaned.contains("\"workbench.colorTheme\": \"Default Dark+\""));

        // Test Claude Code ~/.claude/settings.json apply and cleanup
        let claude_settings = tmp_dir.join(".claude").join("settings.json");
        assert!(apply_claude_code_settings_file(&claude_settings, "http://127.0.0.1:8899").unwrap());
        let claude_content = fs::read_to_string(&claude_settings).unwrap();
        assert!(claude_content.contains("\"HTTPS_PROXY\": \"http://127.0.0.1:8899\""));

        assert!(cleanup_claude_code_settings_file(&claude_settings).unwrap());
        let claude_cleaned = fs::read_to_string(&claude_settings).unwrap();
        assert!(!claude_cleaned.contains("127.0.0.1:8899"));

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_ide_and_claude_settings_with_tz() {
        let tmp_dir = std::env::temp_dir().join(format!("zastava_tz_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&tmp_dir);
        let vscode_settings = tmp_dir.join("settings.json");

        fs::write(&vscode_settings, "{}").unwrap();
        assert!(apply_ide_settings_file_with_tz(
            &vscode_settings,
            "http://127.0.0.1:8899",
            Some("Europe/Amsterdam")
        )
        .unwrap());

        let content = fs::read_to_string(&vscode_settings).unwrap();
        assert!(content.contains("\"TZ\": \"Europe/Amsterdam\""));
        assert!(content.contains("\"LANG\": \"en_US.UTF-8\""));

        // Cleanup removes TZ and LANG
        assert!(cleanup_ide_settings_file(&vscode_settings).unwrap());
        let cleaned = fs::read_to_string(&vscode_settings).unwrap();
        assert!(!cleaned.contains("Europe/Amsterdam"));
        assert!(!cleaned.contains("en_US.UTF-8"));

        // Claude settings
        let claude_settings = tmp_dir.join(".claude").join("settings.json");
        assert!(apply_claude_code_settings_file_with_tz(
            &claude_settings,
            "http://127.0.0.1:8899",
            Some("Europe/Helsinki")
        )
        .unwrap());
        let claude_content = fs::read_to_string(&claude_settings).unwrap();
        assert!(claude_content.contains("\"TZ\": \"Europe/Helsinki\""));
        assert!(claude_content.contains("\"LANG\": \"en_US.UTF-8\""));

        assert!(cleanup_claude_code_settings_file(&claude_settings).unwrap());
        let claude_cleaned = fs::read_to_string(&claude_settings).unwrap();
        assert!(!claude_cleaned.contains("Europe/Helsinki"));

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
