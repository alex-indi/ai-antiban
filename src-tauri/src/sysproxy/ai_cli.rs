//! Настройки только ИИ-клиентов: отдельные ключи env, без глобального TZ.
use super::{NO_PROXY_VALUE, config_settings};
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn env_changes(
    parent: &str,
    proxy_url: &str,
    tz: Option<&str>,
) -> Vec<(Vec<String>, Value)> {
    let mut changes: Vec<_> = ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"]
        .into_iter()
        .map(|key| (vec![parent.into(), key.into()], json!(proxy_url)))
        .collect();
    changes.push((
        vec![parent.into(), "NO_PROXY".into()],
        json!(NO_PROXY_VALUE),
    ));
    if let Some(timezone) = tz.and_then(super::sanitize_timezone) {
        changes.push((vec![parent.into(), "TZ".into()], json!(timezone)));
        for key in ["LANG", "LC_TIME", "LC_ALL"] {
            changes.push((vec![parent.into(), key.into()], json!("en_US.UTF-8")));
        }
    }
    changes
}

pub fn apply_claude_code_settings_file(path: &Path, proxy: &str) -> Result<bool, String> {
    apply_claude_code_settings_file_with_tz(path, proxy, None)
}
pub fn apply_claude_code_settings_file_with_tz(
    path: &Path,
    proxy: &str,
    tz: Option<&str>,
) -> Result<bool, String> {
    config_settings::apply_json(path, env_changes("env", proxy, tz))
}
pub fn apply_gemini_cli_settings_file(
    path: &Path,
    proxy: &str,
    tz: Option<&str>,
) -> Result<bool, String> {
    apply_claude_code_settings_file_with_tz(path, proxy, tz)
}
pub fn cleanup_claude_code_settings_file(path: &Path) -> Result<bool, String> {
    config_settings::cleanup_json(path)
}
pub fn cleanup_gemini_cli_settings_file(path: &Path) -> Result<bool, String> {
    config_settings::cleanup_json(path)
}
