//! Coverage snapshot inspection command.

use crate::proxy::{CoverageSnapshot, ProxyManager};
use crate::sysproxy;

/// Returns current network protection coverage across installed applications and active socket traffic.
#[tauri::command]
pub async fn get_coverage_snapshot(
    proxy_manager: tauri::State<'_, ProxyManager>,
) -> Result<CoverageSnapshot, String> {
    let sysproxy_enabled = sysproxy::is_sysproxy_active();
    let cli_protection_enabled = sysproxy::is_cli_protection_enabled();
    let chained_proxy = sysproxy::get_chained_proxy();
    Ok(proxy_manager.coverage.get_snapshot(
        sysproxy_enabled,
        cli_protection_enabled,
        chained_proxy,
    ))
}
