//! Process-exit safety net and cleanup entry points: panic hook, OS signal watcher,
//! forced removal of every proxy trace, and legacy Windows `HKCU\Environment` purge.

use super::{disable_all_sysproxy, disable_cli_protection};
#[cfg(target_os = "windows")]
use super::windows::{broadcast_windows_env_change, delete_windows_user_env_var};
use std::sync::Once;

static CLEAN_HOOK_INIT: Once = Once::new();

/// Registers panic hook and cleanup guard once per process.
pub fn register_clean_exit_guard() {
    CLEAN_HOOK_INIT.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            eprintln!("Panic detected! Executing emergency system PAC proxy cleanup...");
            disable_all_sysproxy();
            default_hook(panic_info);
        }));
    });
}

/// Spawns an asynchronous OS signal watcher (`SIGINT` / `SIGTERM` / `SIGHUP`) that guarantees
/// systemic PAC proxy settings are disabled before the process terminates.
pub fn spawn_signal_cleanup_watcher() {
    tauri::async_runtime::spawn(async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = signal(SignalKind::terminate()).ok();
            let mut sighup = signal(SignalKind::hangup()).ok();

            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = async {
                    match &mut sigterm {
                        Some(s) => { let _ = s.recv().await; }
                        None => std::future::pending::<()>().await,
                    }
                } => {},
                _ = async {
                    match &mut sighup {
                        Some(s) => { let _ = s.recv().await; }
                        None => std::future::pending::<()>().await,
                    }
                } => {},
            }
        }

        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }

        eprintln!("Termination signal received! Cleaning up system PAC proxy...");
        disable_all_sysproxy();
        std::process::exit(0);
    });
}

/// Unconditionally deletes any legacy proxy, timezone (`TZ`), or locale (`LANG`, `LC_TIME`, `LC_ALL`)
/// variables from `HKCU\Environment` on Windows and broadcasts `WM_SETTINGCHANGE` ("Environment")
/// to `HWND_BROADCAST` so `explorer.exe` and desktop apps (e.g. Telegram Desktop) never inherit
/// an IANA `TZ=Europe/...` string that Windows CRT (`_tzset`) misparses as UTC+0 (-3h from MSK).
pub fn cleanup_legacy_windows_user_env() {
    #[cfg(target_os = "windows")]
    {
        for var_name in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "NO_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
            "no_proxy",
            "TZ",
            "LANG",
            "LC_TIME",
            "LC_ALL",
        ] {
            let _ = delete_windows_user_env_var(var_name);
        }
        broadcast_windows_env_change();
    }
}

/// Forcefully and completely cleans all OS, terminal, IDE, and registry proxy traces,
/// returning a human-readable list of cleaned components.
pub fn force_clean_all_os_proxy_traces() -> Vec<String> {
    disable_all_sysproxy();
    disable_cli_protection();
    cleanup_legacy_windows_user_env();
    vec![
        "Системный PAC-прокси".to_string(),
        "Конфигурации VS Code / Cursor / Claude".to_string(),
        "Профили терминала и переменные среды ОС".to_string(),
    ]
}

/// Forcefully cleans all OS proxy settings, CLI profile blocks, IDE configs, and uninstaller persistent traces.
pub fn cleanup_all_persistent_settings() {
    let _ = force_clean_all_os_proxy_traces();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sysproxy::{is_sysproxy_active, SysProxy};

    #[test]
    fn test_graceful_proxy_cleanup() {
        register_clean_exit_guard();
        SysProxy::disable();
        assert!(!SysProxy::is_active());
    }

    #[test]
    fn test_graceful_proxy_cleanup_hook() {
        register_clean_exit_guard();
        // Disabling proxy should execute safely without crashing
        disable_all_sysproxy();
        assert!(!is_sysproxy_active());
    }

    #[test]
    fn test_force_clean_all_os_proxy_traces() {
        let cleaned = force_clean_all_os_proxy_traces();
        assert_eq!(cleaned.len(), 3);
        assert!(cleaned.iter().any(|c| c.contains("PAC")));
    }
}
