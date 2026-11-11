//! Безопасное восстановление при выходе и следующем запуске после аварии.

use super::{disable_all_sysproxy, disable_cli_protection};
/// Совместимая точка входа. Очистка из panic hook может захватить уже занятый
/// mutex; аварийное восстановление вместо этого выполняет журнал на следующем запуске.
pub fn register_clean_exit_guard() {}

/// Обработчик SIGINT/SIGTERM/SIGHUP пытается восстановить собственные настройки.
pub fn spawn_signal_cleanup_watcher() {
    tauri::async_runtime::spawn(async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
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

        let _ = cleanup_all_persistent_settings();
        std::process::exit(0);
    });
}

/// У старых версий нет достоверного журнала HKCU Environment.
/// Совпадение TZ или localhost-порта не доказывает владение: ничего не удаляем.
pub fn cleanup_legacy_windows_user_env() {}

/// Восстановление всех областей для выхода, обновления, аварийного перезапуска и uninstall.
pub fn cleanup_all_persistent_settings() -> bool {
    let mut reports = disable_all_sysproxy();
    reports.extend(disable_cli_protection());
    reports.iter().all(|r| {
        matches!(
            r.outcome,
            super::owned_settings::Outcome::Restored
                | super::owned_settings::Outcome::NotOwned
                | super::owned_settings::Outcome::Conflict
        )
    })
}

/// Старый принудительный сброс теперь возвращает фактический результат безопасного восстановления.
pub fn force_clean_all_os_proxy_traces() -> Vec<String> {
    let mut reports = disable_all_sysproxy();
    reports.extend(disable_cli_protection());
    reports
        .into_iter()
        .map(|r| {
            let state = match r.outcome {
                super::owned_settings::Outcome::Restored => "восстановлено",
                super::owned_settings::Outcome::NotOwned => "нет владения",
                super::owned_settings::Outcome::Conflict => "внешнее изменение сохранено",
                super::owned_settings::Outcome::Error => "ошибка; журнал сохранён",
                super::owned_settings::Outcome::Unknown => {
                    "владение неизвестно; настройки сохранены"
                }
            };
            format!("{}: {state}", r.item)
        })
        .collect()
}
