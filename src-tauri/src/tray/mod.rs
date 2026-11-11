//! System tray: managed menu-item handles, icon/title/tooltip updates.
//! `menu` builds the tray and its menu, `events` handles menu clicks and tray icon clicks.

mod events;
mod menu;

pub use menu::build_tray;

use crate::{auditor, state};
use tauri::{
    image::Image,
    menu::{IconMenuItem, MenuItem},
    Manager,
};

pub const MAIN_TRAY_ID: &str = "main-tray";

#[derive(Clone)]
pub struct TrayStatsMenuItem(pub MenuItem<tauri::Wry>);

#[derive(Clone)]
pub struct TrayKillswitchMenuItem(pub IconMenuItem<tauri::Wry>);

#[derive(Clone)]
pub struct TrayStatusMenuItem(pub IconMenuItem<tauri::Wry>);

pub(crate) const TRAY_ICON_GREEN: &[u8] = include_bytes!("../../icons/tray-green.png");
pub(crate) const TRAY_ICON_YELLOW: &[u8] = include_bytes!("../../icons/tray-yellow.png");
pub(crate) const TRAY_ICON_RED: &[u8] = include_bytes!("../../icons/tray-red.png");

/// Updates the system tray icon, menu bar title (macOS), tooltip, and live menu stats based on current audit report.
pub fn update_tray_from_report(
    app: &tauri::AppHandle,
    report: &auditor::RiskReport,
    is_pinned: bool,
    has_blocked: bool,
) {
    if let Some(tray) = app.tray_by_id(MAIN_TRAY_ID) {
        let country = &report.telemetry.country_code;

        let (icon_bytes, status_label) = if has_blocked {
            (TRAY_ICON_RED, " СТОП СЕТИ".to_string())
        } else {
            match report.overall_status {
                auditor::OverallStatus::Green => {
                    let pin_mark = if is_pinned { " · Закреплён" } else { "" };
                    (TRAY_ICON_GREEN, format!(" Защита · {country}{pin_mark}"))
                }
                auditor::OverallStatus::Yellow => {
                    (TRAY_ICON_YELLOW, format!(" Внимание · {country}"))
                }
                auditor::OverallStatus::Red => {
                    (TRAY_ICON_RED, " СТОП СЕТИ".to_string())
                }
            }
        };

        let _ = tray.set_title(Some(&status_label));

        let stats = app
            .try_state::<state::AppState>()
            .map(|s| s.stats().snapshot_sync())
            .unwrap_or_default();
        let tooltip = format!(
            "Застава · IP: {} ({}) · Защищено запросов: {} · Отражено сбоев: {}",
            report.telemetry.ip, country, stats.protected_ai_requests, stats.intercepted_leaks
        );
        let _ = tray.set_tooltip(Some(&tooltip));

        if let Ok(img) = Image::from_bytes(icon_bytes) {
            let _ = tray.set_icon(Some(img));
        }

        if let Some(item) = app.try_state::<TrayStatsMenuItem>() {
            let _ = item.0.set_text(format!(
                "Защищено запросов: {} · Отражено сбоев: {}",
                stats.protected_ai_requests, stats.intercepted_leaks
            ));
        }

        if let Some(status_item) = app.try_state::<TrayStatusMenuItem>() {
            let (header_text, header_icon) = if has_blocked {
                ("Застава v1.3.0 — СТОП СЕТИ", TRAY_ICON_RED)
            } else {
                match report.overall_status {
                    auditor::OverallStatus::Green => ("Застава v1.3.0 — Шлюз активен", TRAY_ICON_GREEN),
                    auditor::OverallStatus::Yellow => ("Застава v1.3.0 — Требуется внимание", TRAY_ICON_YELLOW),
                    auditor::OverallStatus::Red => ("Застава v1.3.0 — СТОП СЕТИ", TRAY_ICON_RED),
                }
            };
            let _ = status_item.0.set_text(header_text);
            if let Ok(img) = Image::from_bytes(header_icon) {
                let _ = status_item.0.set_icon(Some(img));
            }
        }

        if let Some(ks_item) = app.try_state::<TrayKillswitchMenuItem>() {
            let (ks_text, ks_icon) = if has_blocked {
                ("Снять СТОП СЕТИ — Возобновить работу ИИ", TRAY_ICON_GREEN)
            } else {
                ("СТОП СЕТИ — Заморозить трафик ИИ", TRAY_ICON_RED)
            };
            let _ = ks_item.0.set_text(ks_text);
            if let Ok(img) = Image::from_bytes(ks_icon) {
                let _ = ks_item.0.set_icon(Some(img));
            }
        }
    }
}

/// Updates the system tray icon, menu bar title (macOS), and tooltip with a simple color and status title.
pub fn update_tray_status(app: &tauri::AppHandle, color: &str, title_override: Option<&str>) {
    if let Some(tray) = app.tray_by_id(MAIN_TRAY_ID) {
        let (icon_bytes, default_title) = match color {
            "red" => (TRAY_ICON_RED, " СТОП СЕТИ"),
            "yellow" => (TRAY_ICON_YELLOW, " Внимание"),
            _ => (TRAY_ICON_GREEN, " Защита"),
        };
        let title = title_override.unwrap_or(default_title);
        let _ = tray.set_title(Some(title));
        let _ = tray.set_tooltip(Some(&format!("Застава · {title}")));
        if let Ok(img) = Image::from_bytes(icon_bytes) {
            let _ = tray.set_icon(Some(img));
        }
    }
}
