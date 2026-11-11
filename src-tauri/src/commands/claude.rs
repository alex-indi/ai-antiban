//! Claude-specific commands: app cache cleaning, local data scan/export, multi-account profiles.

use crate::{auditor, proxy, state};

#[tauri::command]
pub async fn clean_app_cache(
    provider: String,
) -> Result<auditor::cache_cleaner::CleanResult, String> {
    let provider_id = auditor::ProviderId::from_id(&provider)
        .ok_or_else(|| format!("Неизвестный провайдер: {provider}"))?;
    Ok(auditor::clean_ai_app_cache(provider_id))
}

#[tauri::command]
pub fn scan_claude_data_cmd() -> Result<auditor::claude_exporter::ClaudeLocalDataSummary, String> {
    Ok(auditor::claude_exporter::scan_claude_local_data())
}

#[tauri::command]
pub async fn export_claude_data_cmd() -> Result<auditor::claude_exporter::ClaudeExportResult, String> {
    tokio::task::spawn_blocking(auditor::claude_exporter::export_claude_local_data)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn list_claude_profiles_cmd(
    state: tauri::State<'_, state::AppState>,
) -> Result<auditor::profile_switcher::ClaudeProfileListResponse, String> {
    let pinned_ip = state.get_pinned_ip().await;
    auditor::profile_switcher::list_claude_profiles(pinned_ip.as_deref())
}

#[tauri::command]
pub async fn create_claude_profile_cmd(
    name: String,
    bind_ip: Option<String>,
    bind_country: Option<String>,
    bind_upstream_key: Option<String>,
) -> Result<auditor::profile_switcher::ClaudeProfileListResponse, String> {
    auditor::profile_switcher::create_claude_profile(&name, bind_ip, bind_country, bind_upstream_key)
}

#[tauri::command]
pub async fn switch_claude_profile_cmd(
    target_id: String,
    state: tauri::State<'_, state::AppState>,
) -> Result<auditor::profile_switcher::ProfileSwitchResult, String> {
    let res = auditor::profile_switcher::switch_claude_profile(&target_id)?;
    if let Some(ref ip) = res.active_profile.bound_ip {
        let _ = state.set_pinned_ip(Some(ip.clone())).await;
    }
    if let Some(ref key) = res.bound_upstream_key_updated {
        if let Ok(parsed) = proxy::upstream::parse_upstream_key(key) {
            let _ = state.set_upstream_proxy(Some(parsed)).await;
        }
    }
    Ok(res)
}

#[tauri::command]
pub async fn delete_claude_profile_cmd(
    profile_id: String,
) -> Result<auditor::profile_switcher::ClaudeProfileListResponse, String> {
    auditor::profile_switcher::delete_claude_profile(&profile_id)
}

#[tauri::command]
pub async fn bind_ip_to_claude_profile_cmd(
    profile_id: String,
    bind_ip: Option<String>,
    bind_country: Option<String>,
) -> Result<auditor::profile_switcher::ClaudeProfileListResponse, String> {
    auditor::profile_switcher::update_profile_bound_ip(&profile_id, bind_ip, bind_country)
}

#[tauri::command]
pub async fn bind_upstream_to_claude_profile_cmd(
    profile_id: String,
    upstream_key: Option<String>,
) -> Result<auditor::profile_switcher::ClaudeProfileListResponse, String> {
    auditor::profile_switcher::update_profile_bound_upstream(&profile_id, upstream_key)
}
