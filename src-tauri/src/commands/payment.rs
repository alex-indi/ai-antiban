//! Payment BIN checker and account warm-up timer commands.

use super::AppContext;
use crate::bin_checker;

#[tauri::command]
pub async fn check_payment_bin(
    bin: String,
    vpn_country: Option<String>,
    checker: tauri::State<'_, bin_checker::BinChecker>,
) -> Result<bin_checker::BinAuditResult, String> {
    checker.check_bin(&bin, vpn_country.as_deref()).await
}

#[tauri::command]
pub async fn set_warmup_timer(
    timestamp: Option<u64>,
    state: tauri::State<'_, AppContext>,
) -> Result<Option<u64>, String> {
    state
        .set_warmup_started_at(timestamp)
        .await
        .map_err(|e| e.to_string())?;
    Ok(timestamp)
}
