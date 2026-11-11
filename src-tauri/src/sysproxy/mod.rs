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
pub use ide::{
    apply_ide_settings_file, apply_ide_settings_file_with_tz, candidate_ide_settings_paths,
    cleanup_ide_settings_file, sanitize_jsonc,
};
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
static CHAINED_PROXY: Mutex<Option<String>> = Mutex::new(None);

const ZASTAVA_RC_START: &str = "# >>> Zastava AI CLI & IDE Guard >>>";
const ZASTAVA_RC_END: &str = "# <<< Zastava AI CLI & IDE Guard <<<";
const NO_PROXY_VALUE: &str = "localhost,127.0.0.1,::1,.local";

/// Returns the currently discovered/configured upstream chained proxy (e.g. "127.0.0.1:10808"), if any.
pub fn get_chained_proxy() -> Option<String> {
    CHAINED_PROXY.lock().ok().and_then(|g| g.clone())
}

/// Sets or clears the upstream chained proxy with loop protection against Zastava's gateway.
pub fn set_chained_proxy(proxy: Option<String>) {
    if let Some(ref p) = proxy {
        let current_gp = LAST_PAC_URL
            .lock()
            .ok()
            .and_then(|g| g.as_deref().and_then(parse_gateway_port_from_pac_url));
        if is_gateway_host_port(p, current_gp) {
            return;
        }
    }
    if let Ok(mut g) = CHAINED_PROXY.lock() {
        *g = proxy;
    }
}

/// Extracts gateway port from a PAC URL (e.g. "http://127.0.0.1:8899/proxy.pac" -> 8899).
pub fn parse_gateway_port_from_pac_url(pac_url: &str) -> Option<u16> {
    let without_scheme = pac_url
        .strip_prefix("http://")
        .or_else(|| pac_url.strip_prefix("https://"))
        .unwrap_or(pac_url);
    let host_port = without_scheme.split('/').next().unwrap_or("");
    host_port.rsplit_once(':').and_then(|(_, p)| p.parse::<u16>().ok())
}

/// Checks whether `server:port` points to Zastava's own gateway port or default loopback ports,
/// preventing recursive proxy loops.
pub fn is_gateway_target(server: &str, port: u16, gateway_port: Option<u16>) -> bool {
    if port == 0 {
        return true;
    }
    if gateway_port.is_some_and(|gp| gp > 0 && port == gp) {
        return true;
    }
    let s = server.trim().trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
    let is_loopback = s == "127.0.0.1" || s == "localhost" || s == "::1";
    if is_loopback && (port == 8899 || port == 8900) {
        return true;
    }
    false
}

/// Helper that checks `host:port` string against gateway loop.
pub fn is_gateway_host_port(host_port: &str, gateway_port: Option<u16>) -> bool {
    let trimmed = host_port.trim();
    if let Some((host, port_str)) = trimmed.rsplit_once(':') {
        match port_str.parse::<u16>() {
            Ok(port) => is_gateway_target(host, port, gateway_port),
            Err(_) => true,
        }
    } else {
        true
    }
}

/// Parses the output of macOS `networksetup -getwebproxy <service>` or `-getsecurewebproxy <service>`.
/// Returns `Some((server, port))` only when enabled is "Yes", server is non-empty, and port > 0.
pub fn parse_networksetup_webproxy_output(output: &str) -> Option<(String, u16)> {
    let mut enabled = false;
    let mut server = String::new();
    let mut port: u16 = 0;

    for line in output.lines() {
        let trimmed = line.trim();
        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim();
            match key.as_str() {
                "enabled" => {
                    enabled = val.eq_ignore_ascii_case("yes");
                }
                "server" => {
                    server = val.to_string();
                }
                "port" => {
                    port = val.parse::<u16>().unwrap_or(0);
                }
                _ => {}
            }
        }
    }

    if enabled && !server.is_empty() && port > 0 {
        Some((server, port))
    } else {
        None
    }
}

/// Parses Windows registry ProxyServer value into a `host:port` string.
/// Supports both plain format "127.0.0.1:10808" and protocol format "http=127.0.0.1:10808;https=127.0.0.1:10808".
/// Returns `None` if the proxy points to Zastava's gateway (loop prevention) or is invalid.
pub fn parse_windows_proxy_server(raw: &str, gateway_port: Option<u16>) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    let candidate = if raw.contains('=') {
        let parts: Vec<&str> = raw.split(';').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        let find_proto = |proto: &str| -> Option<&str> {
            parts.iter().find_map(|p| {
                let (k, v) = p.split_once('=')?;
                if k.trim().eq_ignore_ascii_case(proto) {
                    Some(v.trim())
                } else {
                    None
                }
            })
        };

        find_proto("https")
            .or_else(|| find_proto("http"))
            .or_else(|| find_proto("socks"))
            .or_else(|| {
                parts.first().and_then(|p| {
                    p.split_once('=').map(|(_, v)| v.trim()).or(Some(p))
                })
            })
    } else {
        Some(raw)
    };

    let target = candidate?.trim();
    if target.is_empty() {
        return None;
    }

    let clean_target = target
        .strip_prefix("http://")
        .or_else(|| target.strip_prefix("https://"))
        .or_else(|| target.strip_prefix("socks://"))
        .unwrap_or(target);

    let (host, port_str) = clean_target.rsplit_once(':')?;
    let port = port_str.parse::<u16>().ok()?;
    if port == 0 {
        return None;
    }

    let clean_host = host.trim().trim_start_matches('[').trim_end_matches(']').to_string();
    if clean_host.is_empty() {
        return None;
    }

    if is_gateway_target(&clean_host, port, gateway_port) {
        return None;
    }

    Some(format!("{clean_host}:{port}"))
}

/// Parses a REG_DWORD integer value from Windows `reg query` output.
pub fn parse_reg_dword(output: &str, value_name: &str) -> Option<u32> {
    for line in output.lines() {
        let trimmed = line.trim();
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 3
            && parts[0].eq_ignore_ascii_case(value_name)
            && parts[1].eq_ignore_ascii_case("REG_DWORD")
        {
            let v = parts[2].trim_start_matches("0x").trim_start_matches("0X");
            return u32::from_str_radix(v, 16).or_else(|_| parts[2].parse::<u32>()).ok();
        }
    }
    None
}

/// Parses a REG_SZ string value from Windows `reg query` output.
pub fn parse_reg_sz(output: &str, value_name: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 2
            && parts[0].eq_ignore_ascii_case(value_name)
            && parts[1].eq_ignore_ascii_case("REG_SZ")
        {
            let idx = trimmed.find("REG_SZ")?;
            let rest = trimmed[idx + 6..].trim();
            return Some(rest.to_string());
        }
    }
    None
}

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
    set_chained_proxy(None);
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

    #[test]
    fn test_parse_networksetup_webproxy_output() {
        let active_output = "Enabled: Yes\nServer: 127.0.0.1\nPort: 10808\nAuthenticated Proxy Enabled: 0\n";
        let parsed = parse_networksetup_webproxy_output(active_output);
        assert_eq!(parsed, Some(("127.0.0.1".to_string(), 10808)));

        // Test loop prevention on Zastava's gateway
        assert!(!is_gateway_target("127.0.0.1", 10808, Some(8899)));
        assert!(is_gateway_target("127.0.0.1", 8899, Some(8899)));
        assert!(is_gateway_target("localhost", 8899, None));
        assert!(is_gateway_target("127.0.0.1", 8900, None));
        assert!(is_gateway_target("127.0.0.1", 0, None));

        let disabled_output = "Enabled: No\nServer: amrika-panel.nomadnocode.com\nPort: 11443\nAuthenticated Proxy Enabled: 0\n";
        assert_eq!(parse_networksetup_webproxy_output(disabled_output), None);

        let empty_output = "";
        assert_eq!(parse_networksetup_webproxy_output(empty_output), None);
    }

    #[test]
    fn test_parse_windows_proxy_server_and_loop_prevention() {
        // Plain format
        assert_eq!(
            parse_windows_proxy_server("127.0.0.1:10808", Some(8899)),
            Some("127.0.0.1:10808".to_string())
        );

        // Protocol formats
        assert_eq!(
            parse_windows_proxy_server("http=127.0.0.1:10808;https=127.0.0.1:10808", Some(8899)),
            Some("127.0.0.1:10808".to_string())
        );
        assert_eq!(
            parse_windows_proxy_server("http=127.0.0.1:10808;https=127.0.0.1:2080", Some(8899)),
            Some("127.0.0.1:2080".to_string())
        );
        assert_eq!(
            parse_windows_proxy_server("socks=127.0.0.1:10808", Some(8899)),
            Some("127.0.0.1:10808".to_string())
        );

        // Loop prevention: Zastava's active gateway port or default ports
        assert_eq!(parse_windows_proxy_server("127.0.0.1:8899", Some(8899)), None);
        assert_eq!(
            parse_windows_proxy_server("http=127.0.0.1:8899;https=127.0.0.1:8899", Some(8899)),
            None
        );
        assert_eq!(parse_windows_proxy_server("localhost:8900", None), None);
        assert_eq!(parse_windows_proxy_server("", Some(8899)), None);
    }

    #[test]
    fn test_parse_reg_dword_and_sz() {
        let dword_sample = "HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings\n    ProxyEnable    REG_DWORD    0x1\n";
        assert_eq!(parse_reg_dword(dword_sample, "ProxyEnable"), Some(1));

        let sz_sample = "HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings\n    ProxyServer    REG_SZ    http=127.0.0.1:10808;https=127.0.0.1:10808\n";
        assert_eq!(
            parse_reg_sz(sz_sample, "ProxyServer"),
            Some("http=127.0.0.1:10808;https=127.0.0.1:10808".to_string())
        );
    }

    #[test]
    fn test_get_set_chained_proxy_loop_guard() {
        set_chained_proxy(Some("127.0.0.1:10808".to_string()));
        assert_eq!(get_chained_proxy(), Some("127.0.0.1:10808".to_string()));

        // Setting Zastava default port should be rejected by loop guard
        set_chained_proxy(Some("127.0.0.1:8899".to_string()));
        assert_ne!(get_chained_proxy(), Some("127.0.0.1:8899".to_string()));

        set_chained_proxy(None);
        assert_eq!(get_chained_proxy(), None);
    }

    #[test]
    fn test_parse_gateway_port_from_pac_url() {
        assert_eq!(
            parse_gateway_port_from_pac_url("http://127.0.0.1:8899/proxy.pac"),
            Some(8899)
        );
        assert_eq!(
            parse_gateway_port_from_pac_url("http://127.0.0.1:8900/pac.js"),
            Some(8900)
        );
        assert_eq!(parse_gateway_port_from_pac_url("127.0.0.1:8899"), Some(8899));
    }
}
