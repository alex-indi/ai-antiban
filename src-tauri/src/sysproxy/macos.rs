//! macOS-specific system proxy plumbing via `networksetup` (Auto Proxy / PAC per network service).
//! Compiled only on `target_os = "macos"` (see `sysproxy/mod.rs`).

use super::{enable_sysproxy, silent_command, LAST_PAC_URL};
use std::sync::Mutex;

static KNOWN_SERVICES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Turns Auto Proxy off on every network service.
pub(super) fn disable_system_proxy() {
    if let Ok(services) = get_macos_network_services() {
        for service in services {
            let _ = silent_command("networksetup")
                .args(["-setautoproxystate", &service, "off"])
                .output();
        }
    }
}

/// Sets the PAC URL on every network service and records the ones that succeeded in `configured`.
pub(super) fn enable_system_proxy(pac_url: &str, configured: &mut Vec<String>) -> Result<(), String> {
    let services = get_macos_network_services().map_err(|e| e.to_string())?;
    if let Ok(mut known) = KNOWN_SERVICES.lock() {
        *known = services.clone();
    }
    for service in services {
        // Set autoproxy URL
        let output_url = silent_command("networksetup")
            .args(["-setautoproxyurl", &service, pac_url])
            .output();

        // Enable autoproxy state
        let output_state = silent_command("networksetup")
            .args(["-setautoproxystate", &service, "on"])
            .output();

        if output_url.is_ok() && output_state.is_ok() {
            configured.push(service);
        }
    }
    Ok(())
}

/// macOS branch of `refresh_if_services_changed`: re-applies the PAC when services changed or the PAC drifted.
pub(super) fn refresh_if_changed() -> bool {
    let pac = LAST_PAC_URL.lock().ok().and_then(|g| g.clone());
    if let (Some(pac), Ok(mut services)) = (pac, get_macos_network_services()) {
        let mut known = KNOWN_SERVICES.lock().map(|g| g.clone()).unwrap_or_default();
        services.sort();
        known.sort();
        if services != known {
            return enable_sysproxy(&pac).is_ok();
        }

        // Also verify that the primary service (e.g. "Wi-Fi" or first active service)
        // still has Auto Proxy enabled and pointing to our PAC URL.
        let check_target = services
            .iter()
            .find(|s| s.eq_ignore_ascii_case("Wi-Fi") || s.contains("Ethernet"))
            .or_else(|| services.first());
        if let Some(service) = check_target {
            if let Ok(output) = silent_command("networksetup")
                .args(["-getautoproxyurl", service])
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let url_matches = stdout.contains(&pac);
                let enabled_yes = stdout.lines().any(|l| {
                    let t = l.trim().to_ascii_lowercase();
                    t.starts_with("enabled:") && t.contains("yes")
                });
                if !url_matches || !enabled_yes {
                    return enable_sysproxy(&pac).is_ok();
                }
            }
        }
    }
    false
}

pub(super) fn get_macos_network_services() -> Result<Vec<String>, std::io::Error> {
    let output = silent_command("networksetup")
        .arg("-listallnetworkservices")
        .output()?;

    let text = String::from_utf8_lossy(&output.stdout);
    let mut services = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('*')
            || trimmed.contains("denotes that a network service is disabled")
        {
            continue;
        }
        services.push(trimmed.to_string());
    }

    Ok(services)
}
