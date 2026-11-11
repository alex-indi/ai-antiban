//! Upstream (egress) proxy configuration commands.

use super::AppContext;
use crate::{proxy, state};

#[tauri::command]
pub async fn set_upstream_proxy(
    key: String,
    state: tauri::State<'_, AppContext>,
) -> Result<state::UpstreamProxyConfig, String> {
    let cfg = proxy::upstream::parse_upstream_key(&key)?;
    state
        .set_upstream_proxy(Some(cfg.clone()))
        .await
        .map_err(|e| e.to_string())?;
    Ok(cfg)
}

#[tauri::command]
pub async fn toggle_upstream_proxy(
    enabled: bool,
    state: tauri::State<'_, AppContext>,
) -> Result<Option<state::UpstreamProxyConfig>, String> {
    let mut current = state.get_upstream_proxy().await;
    if let Some(ref mut cfg) = current {
        cfg.enabled = enabled;
        state
            .set_upstream_proxy(Some(cfg.clone()))
            .await
            .map_err(|e| e.to_string())?;
        Ok(Some(cfg.clone()))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn clear_upstream_proxy(
    state: tauri::State<'_, AppContext>,
) -> Result<bool, String> {
    state
        .set_upstream_proxy(None)
        .await
        .map_err(|e| e.to_string())?;
    Ok(true)
}
