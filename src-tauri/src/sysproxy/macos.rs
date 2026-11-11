//! macOS-specific system proxy plumbing via `networksetup` (Auto Proxy / PAC per network service).
//! Compiled only on `target_os = "macos"` (see `sysproxy/mod.rs`).

use super::{
    enable_sysproxy, get_chained_proxy, is_gateway_target, parse_gateway_port_from_pac_url,
    parse_networksetup_webproxy_output, set_chained_proxy, silent_command, LAST_PAC_URL,
};
use std::sync::Mutex;

static KNOWN_SERVICES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Inspects webproxy and securewebproxy on the specified network service.
/// If an active third-party proxy is found (not Zastava's gateway), saves it to `CHAINED_PROXY`
/// and disables the explicit web proxy so that browser traffic resolves via our PAC script.
pub(super) fn detect_and_save_macos_chained_proxy(
    service: &str,
    gateway_port: Option<u16>,
) -> Option<String> {
    for flag in &["-getwebproxy", "-getsecurewebproxy"] {
        if let Ok(output) = silent_command("networksetup")
            .args([flag, service])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some((server, port)) = parse_networksetup_webproxy_output(&text) {
                if !is_gateway_target(&server, port, gateway_port) {
                    let chained = format!("{server}:{port}");
                    set_chained_proxy(Some(chained.clone()));

                    // Disable explicit web proxy on this service so PAC is authoritative
                    let _ = silent_command("networksetup")
                        .args(["-setwebproxystate", service, "off"])
                        .output();
                    let _ = silent_command("networksetup")
                        .args(["-setsecurewebproxystate", service, "off"])
                        .output();

                    return Some(chained);
                }
            }
        }
    }
    None
}

/// Turns Auto Proxy off on every network service, and restores third-party chained proxy if present.
pub(super) fn disable_system_proxy() {
    let chained = get_chained_proxy();
    if let Ok(services) = get_macos_network_services() {
        for service in &services {
            let _ = silent_command("networksetup")
                .args(["-setautoproxystate", service, "off"])
                .output();
        }

        // If a third-party upstream proxy was chained, restore it on all network services
        if let Some(ref chained_target) = chained {
            if let Some((host, port_str)) = chained_target.rsplit_once(':') {
                if let Ok(port) = port_str.parse::<u16>() {
                    for service in &services {
                        let _ = silent_command("networksetup")
                            .args(["-setwebproxy", service, host, &port.to_string()])
                            .output();
                        let _ = silent_command("networksetup")
                            .args(["-setsecurewebproxy", service, host, &port.to_string()])
                            .output();
                        let _ = silent_command("networksetup")
                            .args(["-setwebproxystate", service, "on"])
                            .output();
                        let _ = silent_command("networksetup")
                            .args(["-setsecurewebproxystate", service, "on"])
                            .output();
                    }
                }
            }
        }
    }
}

/// Sets the PAC URL on every network service and records the ones that succeeded in `configured`.
pub(super) fn enable_system_proxy(pac_url: &str, configured: &mut Vec<String>) -> Result<(), String> {
    let services = get_macos_network_services().map_err(|e| e.to_string())?;
    let gateway_port = parse_gateway_port_from_pac_url(pac_url);

    if let Ok(mut known) = KNOWN_SERVICES.lock() {
        *known = services.clone();
    }

    for service in &services {
        // 1. Detect third-party proxy before overriding, chain it, and turn off explicit proxy
        detect_and_save_macos_chained_proxy(service, gateway_port);

        // 2. Set autoproxy URL
        let output_url = silent_command("networksetup")
            .args(["-setautoproxyurl", service, pac_url])
            .output();

        // 3. Enable autoproxy state
        let output_state = silent_command("networksetup")
            .args(["-setautoproxystate", service, "on"])
            .output();

        if output_url.is_ok() && output_state.is_ok() {
            configured.push(service.clone());
        }
    }
    Ok(())
}

/// macOS branch of `refresh_if_services_changed`: re-applies the PAC when services changed,
/// explicit proxy was enabled on top of PAC, or the PAC drifted.
pub(super) fn refresh_if_changed() -> bool {
    let pac = LAST_PAC_URL.lock().ok().and_then(|g| g.clone());
    if let (Some(pac), Ok(mut services)) = (pac, get_macos_network_services()) {
        let gateway_port = parse_gateway_port_from_pac_url(&pac);
        let mut known = KNOWN_SERVICES.lock().map(|g| g.clone()).unwrap_or_default();
        services.sort();
        known.sort();
        if services != known {
            return enable_sysproxy(&pac).is_ok();
        }

        // Check if any service had an explicit proxy enabled by a third-party VPN client on top of PAC
        let mut explicit_proxy_detected = false;
        for service in &services {
            if detect_and_save_macos_chained_proxy(service, gateway_port).is_some() {
                explicit_proxy_detected = true;
            }
        }
        if explicit_proxy_detected {
            // Re-apply autoproxy so PAC script stays authoritative
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
