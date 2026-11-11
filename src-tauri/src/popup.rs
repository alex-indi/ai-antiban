//! Alert popup window positioning.

use crate::state;
use tauri::Manager;

/// Positions the alert-popup window directly under the notch or top-right of the screen
/// without stealing keyboard focus.
pub fn show_and_position_alert_popup(app: &tauri::AppHandle) {
    if let Some(popup) = app.get_webview_window("alert-popup") {
        let _ = popup.set_shadow(false);
        let _ = popup.set_decorations(false);
        let pos_setting = app
            .try_state::<state::AppState>()
            .map(|s| s.get_popup_position_sync())
            .unwrap_or(state::PopupPosition::TopRight);

        if let Ok(Some(monitor)) = popup.primary_monitor() {
            let scale = monitor.scale_factor();
            let mon_size = monitor.size();
            let win_width = 440.0;
            let mon_width = mon_size.width as f64 / scale;

            let (x, y) = match pos_setting {
                state::PopupPosition::TopRight => {
                    let right_x = (mon_width - win_width - 24.0).max(0.0);
                    (right_x, 24.0)
                }
                state::PopupPosition::TopCenter => {
                    let center_x = ((mon_width - win_width) / 2.0).max(0.0);
                    (center_x, 0.0)
                }
            };
            let _ = popup.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)));
        }
        let _ = popup.show();
        let _ = popup.unminimize();
        let _ = popup.set_always_on_top(true);
        // Note: intentionally do NOT call popup.set_focus() so user's active editor/terminal does not lose focus
    } else if let Some(main_win) = app.get_webview_window("main") {
        let _ = main_win.show();
        let _ = main_win.set_focus();
    }
}
