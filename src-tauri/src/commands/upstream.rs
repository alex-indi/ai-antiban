//! Legacy (v1.3.x) single upstream proxy commands, implemented on top of routes:
//! the key becomes the legacy route and the default route for unbound AI providers.

use super::AppContext;
use crate::{routing, state};

#[tauri::command]
pub async fn set_upstream_proxy(
    key: String,
    state: tauri::State<'_, AppContext>,
) -> Result<state::UpstreamProxyConfig, String> {
    let route = routing::parse_route_key(&key, crate::auditor::network::current_timestamp())?;
    let cfg = route
        .to_legacy(true)
        .ok_or_else(|| "Key has no entry nodes".to_string())?;
    let id = route.id.clone();
    // Only the explicit legacy slot is replaced; a route that merely is the default stays.
    let previous = state.get_data().await.legacy_route_id;
    state.upsert_route(route).await.map_err(|e| e.to_string())?;
    {
        let mut w = state.write().await;
        w.legacy_route_id = Some(id.clone());
        w.default_route_id = Some(id.clone());
        if let Some(r) = w.routes.iter_mut().find(|r| r.id == id) {
            r.enabled = true;
        }
        w.sync_legacy_mirror();
    }
    // A replaced legacy key that nothing else uses is dropped, as the single slot used to be.
    if let Some(prev) = previous.filter(|p| *p != id) {
        let data = state.get_data().await;
        let referenced = data.provider_routes.values().any(|v| *v == prev)
            || data.profile_route_id.as_deref() == Some(prev.as_str());
        if !referenced {
            let _ = state.remove_route(&prev).await;
        }
    }
    state.save_async().await.map_err(|e| e.to_string())?;
    Ok(cfg)
}

#[tauri::command]
pub async fn toggle_upstream_proxy(
    enabled: bool,
    state: tauri::State<'_, AppContext>,
) -> Result<Option<state::UpstreamProxyConfig>, String> {
    state
        .toggle_legacy_upstream(enabled)
        .await
        .map_err(|e| e.to_string())
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
