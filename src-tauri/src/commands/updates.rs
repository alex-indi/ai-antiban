//! Application update commands.

use crate::{state, updater};

#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle) -> Result<updater::UpdateCheckResult, String> {
    updater::check_for_updates(Some(&app)).await
}

#[tauri::command]
pub async fn install_update(
    download_url: String,
    asset_name: String,
    app: tauri::AppHandle,
) -> Result<updater::InstallUpdateResult, String> {
    updater::install_update(&app, &download_url, &asset_name).await
}

#[tauri::command]
pub async fn toggle_auto_update(
    enabled: bool,
    state: tauri::State<'_, state::AppState>,
) -> Result<(), String> {
    state
        .set_auto_update_enabled(enabled)
        .await
        .map_err(|e| e.to_string())
}
