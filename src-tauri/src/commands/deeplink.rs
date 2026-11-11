//! Deep-link (`zastava://`) handling and pending key buffer.

use std::sync::Mutex;
use tauri::{Emitter, Manager};

/// In-memory cell holding the latest incoming deep link for cold start retrieval.
pub static PENDING_DEEP_LINK: Mutex<Option<String>> = Mutex::new(None);

/// Processes an incoming deep link URL: validates the scheme, stores it for cold start,
/// emits an event to the frontend, and focuses the main window.
pub fn handle_deep_link(app_handle: &tauri::AppHandle, raw_url: &str) {
    let clean = clean_deep_link_url(raw_url);
    if clean.is_empty() {
        return;
    }

    if let Ok(mut pending) = PENDING_DEEP_LINK.lock() {
        *pending = Some(clean.clone());
    }

    let _ = app_handle.emit("zastava://deep-link", &clean);

    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Cleans and validates that a raw URL starts with `zastava://`.
pub fn clean_deep_link_url(raw_url: &str) -> String {
    let trimmed = raw_url.trim().trim_matches('"').trim_matches('\'').trim();
    if trimmed.starts_with("zastava://") {
        trimmed.to_string()
    } else {
        String::new()
    }
}

/// Retrieves and clears the pending deep link URL, if one was captured during startup.
#[tauri::command]
pub fn take_pending_deep_link() -> Option<String> {
    PENDING_DEEP_LINK.lock().ok().and_then(|mut p| p.take())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_deep_link_url() {
        assert_eq!(
            clean_deep_link_url("zastava://eyJ2IjoxfQ=="),
            "zastava://eyJ2IjoxfQ=="
        );
        assert_eq!(
            clean_deep_link_url("  \"zastava://foo-bar\"  "),
            "zastava://foo-bar"
        );
        assert_eq!(clean_deep_link_url("https://example.com"), "");
        assert_eq!(clean_deep_link_url("invalid-schema"), "");
    }

    #[test]
    fn test_take_pending_deep_link_lifecycle() {
        {
            let mut lock = PENDING_DEEP_LINK.lock().unwrap();
            *lock = Some("zastava://test-key".to_string());
        }

        assert_eq!(
            take_pending_deep_link(),
            Some("zastava://test-key".to_string())
        );
        // Second call should yield None
        assert_eq!(take_pending_deep_link(), None);
    }
}
