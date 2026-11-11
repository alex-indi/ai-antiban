//! Интеграция ИИ-клиентов и профилей; общие HKCU/launchd переменные не меняются.
#[cfg(unix)]
use super::shell_rc::build_shell_rc_guard_block_with_tz;
use super::shell_rc::normalize_proxy_url_and_port;
use super::{
    CLI_PROTECTION_ACTIVE, CliProtectionReport, NO_PROXY_VALUE, candidate_ide_settings_paths,
    user_home_dir,
};
use std::sync::atomic::Ordering;

pub fn enable_cli_and_ide_protection(proxy: &str) -> CliProtectionReport {
    enable_cli_and_ide_protection_with_tz(proxy, None)
}

pub fn enable_cli_and_ide_protection_with_tz(proxy: &str, tz: Option<&str>) -> CliProtectionReport {
    let (proxy_url, _port) = normalize_proxy_url_and_port(proxy);
    let mut configured_targets = vec![];
    let mut errors = vec![];
    let mut record = |label: &str, result: Result<bool, String>| match result {
        Ok(_) => configured_targets.push(label.to_string()),
        Err(_) => errors.push(format!("{label}: безопасная настройка не завершена")),
    };
    if let Some(home) = user_home_dir() {
        record(
            "Claude Code",
            super::apply_claude_code_settings_file_with_tz(
                &home.join(".claude/settings.json"),
                &proxy_url,
                tz,
            ),
        );
        if home.join(".gemini").is_dir() {
            record(
                "Gemini CLI",
                super::apply_gemini_cli_settings_file(
                    &home.join(".gemini/settings.json"),
                    &proxy_url,
                    tz,
                ),
            );
        }
        #[cfg(unix)]
        for name in [".zshenv", ".zshrc", ".bash_profile"] {
            let path = home.join(name);
            if name != ".bash_profile" || path.exists() {
                record(
                    name,
                    super::config_settings::apply_shell(
                        &path,
                        build_shell_rc_guard_block_with_tz(&proxy_url, _port, tz),
                    ),
                );
            }
        }
        #[cfg(windows)]
        for path in super::get_windows_powershell_profile_paths() {
            record(
                "PowerShell",
                super::config_settings::apply_shell(
                    &path,
                    super::build_powershell_guard_block_with_tz(&proxy_url, tz),
                ),
            );
        }
    }
    for (label, path) in candidate_ide_settings_paths() {
        record(
            label,
            super::apply_ide_settings_file_with_tz(&path, &proxy_url, tz),
        );
    }
    let enabled = !configured_targets.is_empty();
    CLI_PROTECTION_ACTIVE.store(enabled, Ordering::SeqCst);
    let mut shell_command = format!(
        "export HTTP_PROXY=\"{proxy_url}\" HTTPS_PROXY=\"{proxy_url}\" ALL_PROXY=\"{proxy_url}\" NO_PROXY=\"{NO_PROXY_VALUE}\""
    );
    if let Some(timezone) = tz.and_then(super::sanitize_timezone) {
        shell_command.push_str(&format!(
            " TZ=\"{timezone}\" LANG=\"en_US.UTF-8\" LC_ALL=\"en_US.UTF-8\""
        ));
    }
    CliProtectionReport {
        enabled,
        proxy_url,
        configured_targets,
        shell_command,
        errors,
    }
}

pub fn disable_cli_and_ide_runtime_settings() -> Vec<super::owned_settings::RecoveryReport> {
    super::recover_client_settings()
}

pub fn disable_cli_protection() -> Vec<super::owned_settings::RecoveryReport> {
    let mut reports = disable_cli_and_ide_runtime_settings();
    let owned = super::owned_settings::default_path()
        .and_then(super::owned_settings::Journal::open)
        .map(|j| j.resources(false))
        .unwrap_or_default();
    // Только точно маркированные блоки старой версии. Не пытаемся угадать владельца по порту.
    let mut paths = vec![];
    if let Some(home) = user_home_dir() {
        for name in [".zshenv", ".zshrc", ".bash_profile"] {
            paths.push(home.join(name));
        }
    }
    #[cfg(windows)]
    paths.extend(super::get_windows_powershell_profile_paths());
    for path in paths {
        if owned.contains(&super::owned_settings::Resource::Shell(path.clone())) {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(Some(range)) = super::config_settings::shell_range(&text) {
                if text[range.clone()].contains("Remove-Item Env:") {
                    let mut safe = text;
                    safe.replace_range(range, "");
                    if super::owned_settings::atomic_write(&path, safe.as_bytes()).is_err() {
                        reports.push(super::owned_settings::RecoveryReport {
                            item: reports.len(),
                            outcome: super::owned_settings::Outcome::Error,
                        });
                    }
                }
            }
        }
    }
    if !reports.iter().any(|r| {
        matches!(
            r.outcome,
            super::owned_settings::Outcome::Error | super::owned_settings::Outcome::Unknown
        )
    }) {
        CLI_PROTECTION_ACTIVE.store(false, Ordering::SeqCst);
    }
    reports
}

pub fn is_cli_protection_enabled() -> bool {
    CLI_PROTECTION_ACTIVE.load(Ordering::SeqCst)
}
