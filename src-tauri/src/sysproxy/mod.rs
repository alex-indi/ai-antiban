use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};

static SYS_PROXY_ACTIVE: AtomicBool = AtomicBool::new(false);
static CLI_PROTECTION_ACTIVE: AtomicBool = AtomicBool::new(false);
static CLEAN_HOOK_INIT: Once = Once::new();
static LAST_PAC_URL: Mutex<Option<String>> = Mutex::new(None);
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
static KNOWN_SERVICES: Mutex<Vec<String>> = Mutex::new(Vec::new());

const ZASTAVA_RC_START: &str = "# >>> Zastava AI CLI & IDE Guard >>>";
const ZASTAVA_RC_END: &str = "# <<< Zastava AI CLI & IDE Guard <<<";
const NO_PROXY_VALUE: &str = "localhost,127.0.0.1,::1,.local";

/// Spawns a Command configured on Windows with `CREATE_NO_WINDOW` (0x08000000)
/// to completely prevent cmd.exe / PowerShell / Terminal console windows flashing.
pub fn silent_command<S: AsRef<std::ffi::OsStr>>(program: S) -> std::process::Command {
    #[allow(unused_mut)]
    let mut cmd = std::process::Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SysproxyStatus {
    pub enabled: bool,
    pub pac_url: Option<String>,
    pub services_configured: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliProtectionReport {
    pub enabled: bool,
    pub proxy_url: String,
    pub configured_targets: Vec<String>,
    pub shell_command: String,
}

/// Registers panic hook and cleanup guard once per process.
pub fn register_clean_exit_guard() {
    CLEAN_HOOK_INIT.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            eprintln!("Panic detected! Executing emergency system PAC proxy cleanup...");
            disable_all_sysproxy();
            default_hook(panic_info);
        }));
    });
}

/// Spawns an asynchronous OS signal watcher (`SIGINT` / `SIGTERM` / `SIGHUP`) that guarantees
/// systemic PAC proxy settings are disabled before the process terminates.
pub fn spawn_signal_cleanup_watcher() {
    tauri::async_runtime::spawn(async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = signal(SignalKind::terminate()).ok();
            let mut sighup = signal(SignalKind::hangup()).ok();

            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = async {
                    match &mut sigterm {
                        Some(s) => { let _ = s.recv().await; }
                        None => std::future::pending::<()>().await,
                    }
                } => {},
                _ = async {
                    match &mut sighup {
                        Some(s) => { let _ = s.recv().await; }
                        None => std::future::pending::<()>().await,
                    }
                } => {},
            }
        }

        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }

        eprintln!("Termination signal received! Cleaning up system PAC proxy...");
        disable_all_sysproxy();
        std::process::exit(0);
    });
}

/// Disables systemic PAC proxy on all network services across macOS and Windows,
/// and thoroughly cleans up CLI, IDE, and OS environment proxy traces.
pub fn disable_all_sysproxy() {
    #[cfg(target_os = "macos")]
    {
        if let Ok(services) = get_macos_network_services() {
            for service in services {
                let _ = silent_command("networksetup")
                    .args(["-setautoproxystate", &service, "off"])
                    .output();
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows remove HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings\AutoConfigURL
        let _ = silent_command("reg")
            .args([
                "delete",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
                "/v",
                "AutoConfigURL",
                "/f",
            ])
            .output();
        let _ = silent_command("reg")
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
                "/v",
                "ProxyEnable",
                "/t",
                "REG_DWORD",
                "/d",
                "0",
                "/f",
            ])
            .output();
        notify_windows_inet_refresh();
    }

    // Always clean up CLI, shell, PowerShell, IDE settings, and OS environment
    disable_cli_protection();

    #[cfg(target_os = "windows")]
    {
        for var_name in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "NO_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
            "no_proxy",
        ] {
            let _ = delete_windows_user_env_var(var_name);
        }
        broadcast_windows_env_change();
    }

    SYS_PROXY_ACTIVE.store(false, Ordering::SeqCst);
    if let Ok(mut g) = LAST_PAC_URL.lock() {
        *g = None;
    }
}

/// Enables systemic PAC proxy with the given PAC URL (e.g. "http://127.0.0.1:8899/proxy.pac").
pub fn enable_sysproxy(pac_url: &str) -> Result<SysproxyStatus, String> {
    register_clean_exit_guard();
    let mut configured = Vec::new();

    #[cfg(target_os = "macos")]
    {
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
    }

    #[cfg(target_os = "windows")]
    {
        let output = silent_command("reg")
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
                "/v",
                "AutoConfigURL",
                "/t",
                "REG_SZ",
                "/d",
                pac_url,
                "/f",
            ])
            .output();

        let _ = silent_command("reg")
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
                "/v",
                "ProxyEnable",
                "/t",
                "REG_DWORD",
                "/d",
                "0",
                "/f",
            ])
            .output();

        if output.is_ok() {
            configured.push("Windows Internet Settings (WinINet)".to_string());
        }
        notify_windows_inet_refresh();
    }

    SYS_PROXY_ACTIVE.store(true, Ordering::SeqCst);
    if let Ok(mut g) = LAST_PAC_URL.lock() {
        *g = Some(pac_url.to_string());
    }

    Ok(SysproxyStatus {
        enabled: true,
        pac_url: Some(pac_url.to_string()),
        services_configured: configured,
    })
}

#[cfg(target_os = "windows")]
fn notify_windows_inet_refresh() {
    #[link(name = "wininet")]
    unsafe extern "system" {
        fn InternetSetOptionW(
            hInternet: *mut core::ffi::c_void,
            dwOption: u32,
            lpBuffer: *mut core::ffi::c_void,
            dwBufferLength: u32,
        ) -> i32;
    }
    const INTERNET_OPTION_SETTINGS_CHANGED: u32 = 39;
    const INTERNET_OPTION_REFRESH: u32 = 37;
    unsafe {
        let _ = InternetSetOptionW(
            core::ptr::null_mut(),
            INTERNET_OPTION_SETTINGS_CHANGED,
            core::ptr::null_mut(),
            0,
        );
        let _ = InternetSetOptionW(
            core::ptr::null_mut(),
            INTERNET_OPTION_REFRESH,
            core::ptr::null_mut(),
            0,
        );
    }
}

/// Returns whether systemic PAC proxy is currently active.
pub fn is_sysproxy_active() -> bool {
    SYS_PROXY_ACTIVE.load(Ordering::SeqCst)
}

/// Re-applies the PAC configuration when a new network service appeared (e.g. a VPN client
/// added its own service) or when an external network tool / OS sleep-wake cycle disabled
/// or overwrote the PAC URL on the primary interface (Self-Healing).
/// Returns `true` when the configuration was re-applied.
pub fn refresh_if_services_changed() -> bool {
    if !is_sysproxy_active() {
        return false;
    }

    #[cfg(target_os = "macos")]
    {
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
    }

    #[cfg(target_os = "windows")]
    {
        let pac = LAST_PAC_URL.lock().ok().and_then(|g| g.clone());
        if let Some(pac) = pac {
            if let Ok(output) = silent_command("reg")
                .args([
                    "query",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
                    "/v",
                    "AutoConfigURL",
                ])
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if !output.status.success() || !stdout.contains(&pac) {
                    return enable_sysproxy(&pac).is_ok();
                }
            }
        }
    }

    false
}

#[cfg(target_os = "macos")]
fn get_macos_network_services() -> Result<Vec<String>, std::io::Error> {
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

pub struct SysProxy;

impl SysProxy {
    pub fn enable(pac_url: &str) -> Result<SysproxyStatus, String> {
        enable_sysproxy(pac_url)
    }

    pub fn disable() {
        disable_all_sysproxy();
    }

    pub fn is_active() -> bool {
        is_sysproxy_active()
    }
}

fn normalize_proxy_url_and_port(proxy_addr_or_url: &str) -> (String, u16) {
    let trimmed = proxy_addr_or_url
        .trim()
        .trim_end_matches("/proxy.pac")
        .trim_end_matches('/');
    let url = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let port = trimmed
        .rsplit_once(':')
        .and_then(|(_, p)| p.parse::<u16>().ok())
        .unwrap_or(8899);
    (url, port)
}

fn is_zastava_local_proxy_val(val: &str) -> bool {
    let v = val.trim().to_ascii_lowercase();
    v.contains("127.0.0.1:8899")
        || v.contains("127.0.0.1:8900")
        || v.contains("localhost:8899")
        || v.contains("localhost:8900")
}

/// Builds the shell RC block that enables proxy variables in new terminals ONLY when
/// Zastava is actively listening on `127.0.0.1:<port>`. If Zastava is closed, `nc -z`
/// exits immediately and the terminal works directly without proxy errors.
pub fn build_shell_rc_guard_block(proxy_url: &str, port: u16) -> String {
    build_shell_rc_guard_block_with_tz(proxy_url, port, None)
}

/// Validates and sanitizes a timezone identifier string.
/// Returns Some(sanitized) if valid (non-empty, len <= 64, ASCII alphanumeric + '/' | '_' | '-' | '+').
pub fn sanitize_timezone(tz: &str) -> Option<String> {
    let trimmed = tz.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return None;
    }
    if trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '+'))
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Builds the shell RC block including optional timezone and clean UTF-8 locale sync.
pub fn build_shell_rc_guard_block_with_tz(
    proxy_url: &str,
    port: u16,
    tz: Option<&str>,
) -> String {
    let mut tz_exports = String::new();
    if let Some(timezone) = tz.and_then(sanitize_timezone) {
        tz_exports = format!(
            "\n  export TZ=\"{timezone}\"\n  export LANG=\"en_US.UTF-8\"\n  export LC_TIME=\"en_US.UTF-8\"\n  export LC_ALL=\"en_US.UTF-8\""
        );
    }
    format!(
        "{start}\nif command -v nc >/dev/null 2>&1 && nc -z 127.0.0.1 {port} >/dev/null 2>&1; then\n  export HTTP_PROXY=\"{url}\"\n  export HTTPS_PROXY=\"{url}\"\n  export ALL_PROXY=\"{url}\"\n  export http_proxy=\"{url}\"\n  export https_proxy=\"{url}\"\n  export all_proxy=\"{url}\"\n  export NO_PROXY=\"{no_proxy}\"\n  export no_proxy=\"{no_proxy}\"{tz_exports}\nfi\n{end}",
        start = ZASTAVA_RC_START,
        port = port,
        url = proxy_url,
        no_proxy = NO_PROXY_VALUE,
        tz_exports = tz_exports,
        end = ZASTAVA_RC_END
    )
}

/// Builds the PowerShell profile block with proxy settings and optional timezone/locale.
/// Wraps proxy assignments inside a zero-latency kernel TCP listener check so PowerShell never hangs
/// or fails when Zastava is closed or uninstalled, and actively removes stale proxy variables.
pub fn build_powershell_guard_block_with_tz(
    proxy_url: &str,
    tz: Option<&str>,
) -> String {
    let (url, port) = normalize_proxy_url_and_port(proxy_url);
    let mut tz_assignment = String::new();
    if let Some(timezone) = tz.and_then(sanitize_timezone) {
        tz_assignment = format!("\n  $env:TZ = \"{timezone}\"");
    }

    format!(
        "{start}\n$__zastavaUp = $false\ntry {{\n  $__zastavaUp = [bool]([System.Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners() | Where-Object {{ $_.Port -eq {port} }})\n}} catch {{}}\nif ($__zastavaUp) {{\n  $env:HTTP_PROXY = \"{url}\"\n  $env:HTTPS_PROXY = \"{url}\"\n  $env:ALL_PROXY = \"{url}\"\n  $env:http_proxy = \"{url}\"\n  $env:https_proxy = \"{url}\"\n  $env:all_proxy = \"{url}\"\n  $env:NO_PROXY = \"{no_proxy}\"\n  $env:no_proxy = \"{no_proxy}\"\n  $env:LANG = \"en_US.UTF-8\"\n  $env:LC_TIME = \"en_US.UTF-8\"\n  $env:LC_ALL = \"en_US.UTF-8\"{tz_assignment}\n}} else {{\n  Remove-Item Env:HTTP_PROXY,Env:HTTPS_PROXY,Env:ALL_PROXY,Env:http_proxy,Env:https_proxy,Env:all_proxy -ErrorAction SilentlyContinue\n}}\n{end}",
        start = ZASTAVA_RC_START,
        port = port,
        url = url,
        no_proxy = NO_PROXY_VALUE,
        tz_assignment = tz_assignment,
        end = ZASTAVA_RC_END,
    )
}

/// Idempotently inserts or updates the Zastava CLI guard block inside a PowerShell profile file content.
pub fn upsert_powershell_profile_content_with_tz(
    existing: &str,
    proxy_url: &str,
    tz: Option<&str>,
) -> String {
    let block = build_powershell_guard_block_with_tz(proxy_url, tz);
    if let (Some(start_idx), Some(end_idx)) = (
        existing.find(ZASTAVA_RC_START),
        existing.find(ZASTAVA_RC_END),
    ) {
        if start_idx <= end_idx {
            let after_end = end_idx + ZASTAVA_RC_END.len();
            let mut result = String::with_capacity(existing.len() + block.len());
            result.push_str(&existing[..start_idx]);
            result.push_str(&block);
            result.push_str(&existing[after_end..]);
            return result;
        }
    }

    let mut result = existing.to_string();
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
    if !result.is_empty() {
        result.push('\n');
    }
    result.push_str(&block);
    result.push('\n');
    result
}

pub fn get_windows_powershell_profile_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = user_home_dir() {
        let docs = home.join("Documents");
        paths.push(docs.join("PowerShell").join("Microsoft.PowerShell_profile.ps1"));
        paths.push(docs.join("WindowsPowerShell").join("Microsoft.PowerShell_profile.ps1"));
    }
    paths
}

#[cfg(target_os = "windows")]
pub fn set_windows_user_env_var(name: &str, value: &str) -> std::io::Result<()> {
    silent_command("reg")
        .args(["add", r"HKCU\Environment", "/v", name, "/t", "REG_SZ", "/d", value, "/f"])
        .output()?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn delete_windows_user_env_var(name: &str) -> std::io::Result<()> {
    let _ = silent_command("reg")
        .args(["delete", r"HKCU\Environment", "/v", name, "/f"])
        .output();
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn broadcast_windows_env_change() {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn SendMessageTimeoutW(
            hWnd: *mut core::ffi::c_void,
            Msg: u32,
            wParam: usize,
            lParam: *const u16,
            fuFlags: u32,
            uTimeout: u32,
            lpdwResult: *mut usize,
        ) -> isize;
    }
    const HWND_BROADCAST: *mut core::ffi::c_void = 0xffff as *mut core::ffi::c_void;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;
    let env_str: Vec<u16> = "Environment\0".encode_utf16().collect();
    let mut dw_result: usize = 0;
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            env_str.as_ptr(),
            SMTO_ABORTIFHUNG,
            1000,
            &mut dw_result,
        );
    }
}

/// Idempotently inserts or updates the Zastava CLI guard block inside a shell RC file content.
pub fn upsert_shell_rc_content(existing: &str, proxy_url: &str, port: u16) -> String {
    upsert_shell_rc_content_with_tz(existing, proxy_url, port, None)
}

/// Idempotently inserts or updates the Zastava CLI guard block with timezone synchronization.
pub fn upsert_shell_rc_content_with_tz(
    existing: &str,
    proxy_url: &str,
    port: u16,
    tz: Option<&str>,
) -> String {
    let block = build_shell_rc_guard_block_with_tz(proxy_url, port, tz);
    if let (Some(start_idx), Some(end_idx)) = (
        existing.find(ZASTAVA_RC_START),
        existing.find(ZASTAVA_RC_END),
    ) {
        if start_idx <= end_idx {
            let after_end = end_idx + ZASTAVA_RC_END.len();
            let mut result = String::with_capacity(existing.len() + block.len());
            result.push_str(&existing[..start_idx]);
            result.push_str(&block);
            result.push_str(&existing[after_end..]);
            return result;
        }
    }

    let mut result = existing.to_string();
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
    if !result.is_empty() {
        result.push('\n');
    }
    result.push_str(&block);
    result.push('\n');
    result
}

/// Strips `//` and `/* */` comments outside JSON string literals and removes trailing commas
/// so VS Code `settings.json` (JSONC) can be safely parsed by `serde_json`.
pub fn sanitize_jsonc(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut in_string = false;

    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                i += 1;
                out.push(chars[i]);
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }

        // Line comment //
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            i += 2;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Block comment /* ... */
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            continue;
        }

        out.push(c);
        i += 1;
    }

    // Remove trailing commas before `}` or `]` outside strings
    let chars2: Vec<char> = out.chars().collect();
    let mut final_out = String::with_capacity(out.len());
    let mut j = 0;
    let mut in_str = false;
    while j < chars2.len() {
        let c = chars2[j];
        if in_str {
            final_out.push(c);
            if c == '\\' && j + 1 < chars2.len() {
                j += 1;
                final_out.push(chars2[j]);
            } else if c == '"' {
                in_str = false;
            }
            j += 1;
            continue;
        }
        if c == '"' {
            in_str = true;
            final_out.push(c);
            j += 1;
            continue;
        }
        if c == ',' {
            let mut k = j + 1;
            while k < chars2.len() && chars2[k].is_whitespace() {
                k += 1;
            }
            if k < chars2.len() && (chars2[k] == '}' || chars2[k] == ']') {
                j += 1;
                continue;
            }
        }
        final_out.push(c);
        j += 1;
    }

    final_out
}

fn ide_env_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "terminal.integrated.env.osx"
    } else if cfg!(target_os = "windows") {
        "terminal.integrated.env.windows"
    } else {
        "terminal.integrated.env.linux"
    }
}

/// Applies Zastava proxy configuration to a VS Code / Cursor / Windsurf `settings.json` file.
pub fn apply_ide_settings_file(settings_path: &Path, proxy_url: &str) -> Result<bool, String> {
    apply_ide_settings_file_with_tz(settings_path, proxy_url, None)
}

/// Applies Zastava proxy and optional timezone/locale configuration to a VS Code / Cursor / Windsurf `settings.json` file.
pub fn apply_ide_settings_file_with_tz(
    settings_path: &Path,
    proxy_url: &str,
    tz: Option<&str>,
) -> Result<bool, String> {
    let raw = if settings_path.exists() {
        fs::read_to_string(settings_path).map_err(|e| e.to_string())?
    } else {
        "{}".to_string()
    };

    let clean = if raw.trim().is_empty() {
        "{}".to_string()
    } else {
        sanitize_jsonc(&raw)
    };

    let mut root: Value = serde_json::from_str(&clean).map_err(|e| e.to_string())?;
    let Some(obj) = root.as_object_mut() else {
        return Ok(false);
    };

    obj.insert("http.proxy".to_string(), Value::String(proxy_url.to_string()));
    obj.insert(
        "http.proxySupport".to_string(),
        Value::String("override".to_string()),
    );

    let env_key = ide_env_key();
    let env_entry = obj
        .entry(env_key.to_string())
        .or_insert_with(|| json!({}));
    if !env_entry.is_object() {
        *env_entry = json!({});
    }
    if let Some(env_map) = env_entry.as_object_mut() {
        env_map.insert("HTTP_PROXY".to_string(), Value::String(proxy_url.to_string()));
        env_map.insert("HTTPS_PROXY".to_string(), Value::String(proxy_url.to_string()));
        env_map.insert("ALL_PROXY".to_string(), Value::String(proxy_url.to_string()));
        env_map.insert(
            "NO_PROXY".to_string(),
            Value::String(NO_PROXY_VALUE.to_string()),
        );
        if let Some(timezone) = tz {
            let trimmed = timezone.trim();
            if !trimmed.is_empty() {
                env_map.insert("TZ".to_string(), Value::String(trimmed.to_string()));
                env_map.insert("LANG".to_string(), Value::String("en_US.UTF-8".to_string()));
                env_map.insert("LC_TIME".to_string(), Value::String("en_US.UTF-8".to_string()));
                env_map.insert("LC_ALL".to_string(), Value::String("en_US.UTF-8".to_string()));
            }
        }
    }

    let formatted = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    fs::write(settings_path, format!("{formatted}\n")).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Removes Zastava local proxy keys from a VS Code / Cursor / Windsurf `settings.json` file
/// when Zastava exits or when protection is turned off, preserving all other user settings.
pub fn cleanup_ide_settings_file(settings_path: &Path) -> Result<bool, String> {
    if !settings_path.exists() {
        return Ok(false);
    }
    let raw = fs::read_to_string(settings_path).map_err(|e| e.to_string())?;
    let clean = sanitize_jsonc(&raw);
    let mut root: Value = serde_json::from_str(&clean).map_err(|e| e.to_string())?;
    let Some(obj) = root.as_object_mut() else {
        return Ok(false);
    };

    let mut changed = false;
    if let Some(val) = obj.get("http.proxy").and_then(|v| v.as_str()) {
        if is_zastava_local_proxy_val(val) {
            obj.remove("http.proxy");
            obj.remove("http.proxySupport");
            changed = true;
        }
    }

    for env_key in [
        "terminal.integrated.env.osx",
        "terminal.integrated.env.windows",
        "terminal.integrated.env.linux",
    ] {
        let mut remove_entire_env_obj = false;
        if let Some(env_map) = obj.get_mut(env_key).and_then(|v| v.as_object_mut()) {
            let matches_zastava = env_map
                .get("HTTPS_PROXY")
                .or_else(|| env_map.get("HTTP_PROXY"))
                .and_then(|v| v.as_str())
                .map(is_zastava_local_proxy_val)
                .unwrap_or(false);

            if matches_zastava {
                env_map.remove("HTTP_PROXY");
                env_map.remove("HTTPS_PROXY");
                env_map.remove("ALL_PROXY");
                env_map.remove("NO_PROXY");
                env_map.remove("TZ");
                env_map.remove("LANG");
                env_map.remove("LC_TIME");
                env_map.remove("LC_ALL");
                changed = true;
            }
            if env_map.is_empty() {
                remove_entire_env_obj = true;
            }
        }
        if remove_entire_env_obj {
            obj.remove(env_key);
        }
    }

    if changed {
        let formatted = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
        fs::write(settings_path, format!("{formatted}\n")).map_err(|e| e.to_string())?;
    }
    Ok(changed)
}

/// Applies Zastava proxy environment variables to `~/.claude/settings.json` (`env` block)
/// so both the Claude Code VS Code extension and `claude` CLI route through Zastava immediately.
pub fn apply_claude_code_settings_file(settings_path: &Path, proxy_url: &str) -> Result<bool, String> {
    apply_claude_code_settings_file_with_tz(settings_path, proxy_url, None)
}

/// Applies Zastava proxy and timezone variables to `~/.claude/settings.json` (`env` block).
pub fn apply_claude_code_settings_file_with_tz(
    settings_path: &Path,
    proxy_url: &str,
    tz: Option<&str>,
) -> Result<bool, String> {
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = if settings_path.exists() {
        fs::read_to_string(settings_path).map_err(|e| e.to_string())?
    } else {
        "{}".to_string()
    };
    let clean = if raw.trim().is_empty() {
        "{}".to_string()
    } else {
        sanitize_jsonc(&raw)
    };
    let mut root: Value = serde_json::from_str(&clean).unwrap_or_else(|_| json!({}));
    if !root.is_object() {
        root = json!({});
    }
    let obj = root.as_object_mut().unwrap();
    let env_entry = obj.entry("env".to_string()).or_insert_with(|| json!({}));
    if !env_entry.is_object() {
        *env_entry = json!({});
    }
    if let Some(env_map) = env_entry.as_object_mut() {
        env_map.insert("HTTP_PROXY".to_string(), Value::String(proxy_url.to_string()));
        env_map.insert("HTTPS_PROXY".to_string(), Value::String(proxy_url.to_string()));
        env_map.insert("ALL_PROXY".to_string(), Value::String(proxy_url.to_string()));
        env_map.insert(
            "NO_PROXY".to_string(),
            Value::String(NO_PROXY_VALUE.to_string()),
        );
        if let Some(timezone) = tz {
            let trimmed = timezone.trim();
            if !trimmed.is_empty() {
                env_map.insert("TZ".to_string(), Value::String(trimmed.to_string()));
                env_map.insert("LANG".to_string(), Value::String("en_US.UTF-8".to_string()));
                env_map.insert("LC_TIME".to_string(), Value::String("en_US.UTF-8".to_string()));
                env_map.insert("LC_ALL".to_string(), Value::String("en_US.UTF-8".to_string()));
            }
        }
    }

    let formatted = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    fs::write(settings_path, format!("{formatted}\n")).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Applies Zastava proxy and timezone variables to `~/.gemini/settings.json` (`env` block).
pub fn apply_gemini_cli_settings_file(
    settings_path: &Path,
    proxy_url: &str,
    tz: Option<&str>,
) -> Result<bool, String> {
    apply_claude_code_settings_file_with_tz(settings_path, proxy_url, tz)
}

/// Removes Zastava local proxy variables from `~/.gemini/settings.json` when Zastava exits.
pub fn cleanup_gemini_cli_settings_file(settings_path: &Path) -> Result<bool, String> {
    cleanup_claude_code_settings_file(settings_path)
}

/// Removes Zastava local proxy variables from `~/.claude/settings.json` when Zastava exits.
pub fn cleanup_claude_code_settings_file(settings_path: &Path) -> Result<bool, String> {
    if !settings_path.exists() {
        return Ok(false);
    }
    let raw = fs::read_to_string(settings_path).map_err(|e| e.to_string())?;
    let clean = sanitize_jsonc(&raw);
    let mut root: Value = serde_json::from_str(&clean).map_err(|e| e.to_string())?;
    let Some(obj) = root.as_object_mut() else {
        return Ok(false);
    };

    let mut changed = false;
    let mut remove_env_obj = false;
    if let Some(env_map) = obj.get_mut("env").and_then(|v| v.as_object_mut()) {
        let matches_zastava = env_map
            .get("HTTPS_PROXY")
            .or_else(|| env_map.get("HTTP_PROXY"))
            .and_then(|v| v.as_str())
            .map(is_zastava_local_proxy_val)
            .unwrap_or(false);
        if matches_zastava {
            env_map.remove("HTTP_PROXY");
            env_map.remove("HTTPS_PROXY");
            env_map.remove("ALL_PROXY");
            env_map.remove("NO_PROXY");
            env_map.remove("TZ");
            env_map.remove("LANG");
            env_map.remove("LC_TIME");
            env_map.remove("LC_ALL");
            changed = true;
        }
        if env_map.is_empty() {
            remove_env_obj = true;
        }
    }
    if remove_env_obj {
        obj.remove("env");
    }

    if changed {
        let formatted = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
        fs::write(settings_path, format!("{formatted}\n")).map_err(|e| e.to_string())?;
    }
    Ok(changed)
}

fn user_home_dir() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
}

fn candidate_ide_settings_paths() -> Vec<(&'static str, PathBuf)> {
    let mut result = Vec::new();
    let ides = [
        ("VS Code", "Code"),
        ("Cursor", "Cursor"),
        ("Windsurf", "Windsurf"),
        ("VSCodium", "VSCodium"),
        ("Antigravity", "Antigravity"),
    ];

    #[cfg(target_os = "macos")]
    if let Some(home) = user_home_dir() {
        let base = home.join("Library").join("Application Support");
        for (label, folder) in ides {
            let user_dir = base.join(folder).join("User");
            if user_dir.is_dir() {
                result.push((label, user_dir.join("settings.json")));
            }
        }
    }

    #[cfg(target_os = "windows")]
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        for (label, folder) in ides {
            let user_dir = appdata.join(folder).join("User");
            if user_dir.is_dir() {
                result.push((label, user_dir.join("settings.json")));
            }
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    if let Some(home) = user_home_dir() {
        let base = home.join(".config");
        for (label, folder) in ides {
            let user_dir = base.join(folder).join("User");
            if user_dir.is_dir() {
                result.push((label, user_dir.join("settings.json")));
            }
        }
    }

    result
}

/// Automatically configures CLI & IDE protection across:
/// 1. `~/.claude/settings.json` and `~/.gemini/settings.json` (`env.HTTPS_PROXY`, `env.HTTP_PROXY`, optional `TZ`)
/// 2. VS Code / Cursor / Windsurf `settings.json` (`http.proxy` + `terminal.integrated.env.*`)
/// 3. Shell profiles (`~/.zshrc`, `~/.bash_profile`) with an automatic `nc -z` liveness check
/// 4. macOS `launchctl setenv` for GUI apps launched from Dock
pub fn enable_cli_and_ide_protection(proxy_addr_or_url: &str) -> CliProtectionReport {
    enable_cli_and_ide_protection_with_tz(proxy_addr_or_url, None)
}

/// Configures CLI & IDE protection with optional 1-click timezone (`TZ`) and UTF-8 locale sync.
pub fn enable_cli_and_ide_protection_with_tz(
    proxy_addr_or_url: &str,
    tz: Option<&str>,
) -> CliProtectionReport {
    let (proxy_url, _port) = normalize_proxy_url_and_port(proxy_addr_or_url);
    let mut configured_targets = Vec::new();

    #[cfg(target_os = "macos")]
    {
        let mut vars = vec![
            ("HTTP_PROXY", proxy_url.as_str()),
            ("HTTPS_PROXY", proxy_url.as_str()),
            ("ALL_PROXY", proxy_url.as_str()),
            ("NO_PROXY", NO_PROXY_VALUE),
        ];
        if let Some(timezone) = tz {
            let trimmed = timezone.trim();
            if !trimmed.is_empty() {
                vars.push(("TZ", trimmed));
            }
        }
        let mut launchd_ok = false;
        for (k, v) in vars {
            if silent_command("launchctl")
                .args(["setenv", k, v])
                .output()
                .is_ok()
            {
                launchd_ok = true;
            }
        }
        if launchd_ok {
            configured_targets.push("macOS Launchd (GUI)".to_string());
        }
    }

    if let Some(home) = user_home_dir() {
        // 1. Claude Code settings (~/.claude/settings.json)
        let claude_settings = home.join(".claude").join("settings.json");
        if apply_claude_code_settings_file_with_tz(&claude_settings, &proxy_url, tz).is_ok() {
            configured_targets.push("Claude Code".to_string());
        }

        // 2. Gemini CLI / Antigravity settings (~/.gemini/settings.json if directory exists)
        let gemini_dir = home.join(".gemini");
        if gemini_dir.is_dir() {
            let gemini_settings = gemini_dir.join("settings.json");
            if apply_gemini_cli_settings_file(&gemini_settings, &proxy_url, tz).is_ok() {
                configured_targets.push("Gemini CLI".to_string());
            }
        }

        // 3. Shell RC files on Unix (~/.zshenv, ~/.zshrc, ~/.bash_profile)
        #[cfg(unix)]
        {
            let mut shell_configured = false;
            for rc_name in [".zshenv", ".zshrc", ".bash_profile"] {
                let rc_path = home.join(rc_name);
                if rc_name == ".zshenv" || rc_name == ".zshrc" || rc_path.exists() {
                    let existing = fs::read_to_string(&rc_path).unwrap_or_default();
                    let updated = upsert_shell_rc_content_with_tz(&existing, &proxy_url, _port, tz);
                    if fs::write(&rc_path, updated).is_ok() {
                        shell_configured = true;
                    }
                }
            }
            if shell_configured {
                configured_targets.push("Терминал (~/.zshenv, ~/.zshrc)".to_string());
            }
        }

        // 3b. Windows User Environment & PowerShell Profiles
        #[cfg(target_os = "windows")]
        {
            // Self-heal: Stop writing proxy vars into HKCU\Environment.
            // Clean up any stale HTTP_PROXY / HTTPS_PROXY / ALL_PROXY from HKCU\Environment so upgrades from v1.2.0 are healed.
            for stale_var in [
                "HTTP_PROXY",
                "HTTPS_PROXY",
                "ALL_PROXY",
                "NO_PROXY",
                "http_proxy",
                "https_proxy",
                "all_proxy",
                "no_proxy",
            ] {
                let _ = delete_windows_user_env_var(stale_var);
            }

            if let Some(timezone) = tz {
                let trimmed = timezone.trim();
                if !trimmed.is_empty() {
                    let _ = set_windows_user_env_var("TZ", trimmed);
                    let _ = set_windows_user_env_var("LANG", "en_US.UTF-8");
                    let _ = set_windows_user_env_var("LC_TIME", "en_US.UTF-8");
                    configured_targets.push("Windows Реестр (HKCU\\Environment: TZ/Locale)".to_string());
                }
            }
            broadcast_windows_env_change();

            let mut ps_configured = false;
            for ps_path in get_windows_powershell_profile_paths() {
                if let Some(parent) = ps_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let existing = fs::read_to_string(&ps_path).unwrap_or_default();
                let updated = upsert_powershell_profile_content_with_tz(&existing, &proxy_url, tz);
                if fs::write(&ps_path, updated).is_ok() {
                    ps_configured = true;
                }
            }
            if ps_configured {
                configured_targets.push("PowerShell ($PROFILE)".to_string());
            }
        }
    }

    // 4. Installed IDEs (VS Code, Cursor, Windsurf, VSCodium, Antigravity)
    for (label, settings_path) in candidate_ide_settings_paths() {
        if apply_ide_settings_file_with_tz(&settings_path, &proxy_url, tz).unwrap_or(false) {
            configured_targets.push(label.to_string());
        }
    }

    CLI_PROTECTION_ACTIVE.store(true, Ordering::SeqCst);

    let mut shell_command = format!(
        "export HTTP_PROXY=\"{proxy_url}\" HTTPS_PROXY=\"{proxy_url}\" ALL_PROXY=\"{proxy_url}\" NO_PROXY=\"{NO_PROXY_VALUE}\""
    );
    if let Some(timezone) = tz {
        let trimmed = timezone.trim();
        if !trimmed.is_empty() {
            shell_command.push_str(&format!(
                " TZ=\"{trimmed}\" LANG=\"en_US.UTF-8\" LC_ALL=\"en_US.UTF-8\""
            ));
        }
    }

    CliProtectionReport {
        enabled: true,
        proxy_url,
        configured_targets,
        shell_command,
    }
}

/// Cleans up runtime proxy settings in IDE `settings.json`, `~/.claude/settings.json`,
/// `~/.gemini/settings.json`, and macOS `launchctl` environment.
pub fn disable_cli_and_ide_runtime_settings() {
    #[cfg(target_os = "macos")]
    {
        for k in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "NO_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
            "no_proxy",
            "TZ",
        ] {
            let _ = silent_command("launchctl")
                .args(["unsetenv", k])
                .output();
        }
    }

    if let Some(home) = user_home_dir() {
        let claude_settings = home.join(".claude").join("settings.json");
        let _ = cleanup_claude_code_settings_file(&claude_settings);

        let gemini_settings = home.join(".gemini").join("settings.json");
        if gemini_settings.exists() {
            let _ = cleanup_gemini_cli_settings_file(&gemini_settings);
        }
    }

    for (_label, settings_path) in candidate_ide_settings_paths() {
        let _ = cleanup_ide_settings_file(&settings_path);
    }
}

pub fn remove_shell_rc_block(existing: &str) -> String {
    if let (Some(start_idx), Some(end_idx)) = (
        existing.find(ZASTAVA_RC_START),
        existing.find(ZASTAVA_RC_END),
    ) {
        if start_idx <= end_idx {
            let mut after_end = end_idx + ZASTAVA_RC_END.len();
            if existing[after_end..].starts_with('\n') {
                after_end += 1;
            }
            let mut result = String::with_capacity(existing.len());
            result.push_str(&existing[..start_idx]);
            result.push_str(&existing[after_end..]);
            return result;
        }
    }
    existing.to_string()
}

pub fn disable_cli_protection() {
    CLI_PROTECTION_ACTIVE.store(false, Ordering::SeqCst);
    disable_cli_and_ide_runtime_settings();
    if let Some(home) = user_home_dir() {
        for rc_name in [".zshenv", ".zshrc", ".bash_profile"] {
            let rc_path = home.join(rc_name);
            if rc_path.exists() {
                if let Ok(content) = fs::read_to_string(&rc_path) {
                    let cleaned = remove_shell_rc_block(&content);
                    if cleaned != content {
                        let _ = fs::write(&rc_path, cleaned);
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        for var_name in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "NO_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
            "no_proxy",
            "TZ",
            "LANG",
            "LC_TIME",
            "LC_ALL",
        ] {
            let _ = delete_windows_user_env_var(var_name);
        }
        broadcast_windows_env_change();

        for ps_path in get_windows_powershell_profile_paths() {
            if ps_path.exists() {
                if let Ok(content) = fs::read_to_string(&ps_path) {
                    let cleaned = remove_shell_rc_block(&content);
                    if cleaned != content {
                        let _ = fs::write(&ps_path, cleaned);
                    }
                }
            }
        }
    }
}

/// Forcefully and completely cleans all OS, terminal, IDE, and registry proxy traces,
/// returning a human-readable list of cleaned components.
pub fn force_clean_all_os_proxy_traces() -> Vec<String> {
    disable_all_sysproxy();
    disable_cli_protection();
    vec![
        "Системный PAC-прокси".to_string(),
        "Конфигурации VS Code / Cursor / Claude".to_string(),
        "Профили терминала и переменные среды ОС".to_string(),
    ]
}

/// Forcefully cleans all OS proxy settings, CLI profile blocks, IDE configs, and uninstaller persistent traces.
pub fn cleanup_all_persistent_settings() {
    let _ = force_clean_all_os_proxy_traces();
}

/// Checks whether CLI protection is currently enabled in memory, in shell RC files (~/.zshenv, ~/.zshrc, ~/.bash_profile),
/// Claude/Gemini settings, or Windows registry/PowerShell environment.
pub fn is_cli_protection_enabled() -> bool {
    if CLI_PROTECTION_ACTIVE.load(Ordering::SeqCst) {
        return true;
    }
    if let Some(home) = user_home_dir() {
        for rc_name in [".zshenv", ".zshrc", ".bash_profile"] {
            let rc_path = home.join(rc_name);
            if rc_path.exists() {
                if let Ok(content) = fs::read_to_string(&rc_path) {
                    if content.contains(ZASTAVA_RC_START)
                        || content.contains("# >>> ZASTAVA AI GUARD >>>")
                        || content.to_ascii_uppercase().contains("ZASTAVA AI")
                        || content.contains("127.0.0.1:8899")
                    {
                        return true;
                    }
                }
            }
        }
        let claude_settings = home.join(".claude").join("settings.json");
        if claude_settings.exists() {
            if let Ok(content) = fs::read_to_string(&claude_settings) {
                if is_zastava_local_proxy_val(&content) {
                    return true;
                }
            }
        }
        let gemini_settings = home.join(".gemini").join("settings.json");
        if gemini_settings.exists() {
            if let Ok(content) = fs::read_to_string(&gemini_settings) {
                if is_zastava_local_proxy_val(&content) {
                    return true;
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = silent_command("reg")
            .args(["query", r"HKCU\Environment", "/v", "HTTP_PROXY"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if is_zastava_local_proxy_val(&stdout) {
                return true;
            }
        }
        for ps_path in get_windows_powershell_profile_paths() {
            if ps_path.exists() {
                if let Ok(content) = fs::read_to_string(&ps_path) {
                    if content.contains(ZASTAVA_RC_START) || is_zastava_local_proxy_val(&content) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Spawns the native OS date and time settings panel.
pub fn open_os_datetime_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        silent_command("open")
            .arg("x-apple.systempreferences:com.apple.preference.datetime")
            .spawn()
            .map_err(|e| format!("Failed to open macOS date & time settings: {e}"))?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        silent_command("cmd")
            .args(["/C", "start", "ms-settings:dateandtime"])
            .spawn()
            .map_err(|e| format!("Failed to open Windows date & time settings: {e}"))?;
        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graceful_proxy_cleanup() {
        register_clean_exit_guard();
        SysProxy::disable();
        assert!(!SysProxy::is_active());
    }

    #[test]
    fn test_graceful_proxy_cleanup_hook() {
        register_clean_exit_guard();
        // Disabling proxy should execute safely without crashing
        disable_all_sysproxy();
        assert!(!is_sysproxy_active());
    }

    #[test]
    fn test_upsert_shell_rc_idempotent() {
        let initial = "export PATH=\"$HOME/.local/bin:$PATH\"\n";
        let updated1 = upsert_shell_rc_content(initial, "http://127.0.0.1:8899", 8899);
        assert!(updated1.contains("nc -z 127.0.0.1 8899"));
        assert!(updated1.contains("export HTTPS_PROXY=\"http://127.0.0.1:8899\""));

        // Second upsert with fallback port 8900 replaces the block without duplicating it
        let updated2 = upsert_shell_rc_content(&updated1, "http://127.0.0.1:8900", 8900);
        assert_eq!(updated2.matches(ZASTAVA_RC_START).count(), 1);
        assert!(updated2.contains("nc -z 127.0.0.1 8900"));
        assert!(!updated2.contains("127.0.0.1:8899"));

        let removed = remove_shell_rc_block(&updated2);
        assert!(!removed.contains(ZASTAVA_RC_START));
        assert!(!removed.contains(ZASTAVA_RC_END));
        assert!(removed.contains("export PATH="));
    }

    #[test]
    fn test_ide_and_claude_settings_apply_and_cleanup() {
        let tmp_dir = std::env::temp_dir().join(format!("zastava_ide_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&tmp_dir);
        let vscode_settings = tmp_dir.join("settings.json");

        // Write JSONC with comments and trailing comma
        let initial_jsonc = r#"{
            // User custom theme
            "workbench.colorTheme": "Default Dark+",
            "editor.fontSize": 14,
        }"#;
        fs::write(&vscode_settings, initial_jsonc).unwrap();

        assert!(apply_ide_settings_file(&vscode_settings, "http://127.0.0.1:8899").unwrap());
        let content = fs::read_to_string(&vscode_settings).unwrap();
        assert!(content.contains("\"http.proxy\": \"http://127.0.0.1:8899\""));
        assert!(content.contains("\"HTTPS_PROXY\": \"http://127.0.0.1:8899\""));
        assert!(content.contains("\"workbench.colorTheme\": \"Default Dark+\""));

        // Cleanup removes Zastava proxy keys while preserving user settings
        assert!(cleanup_ide_settings_file(&vscode_settings).unwrap());
        let cleaned = fs::read_to_string(&vscode_settings).unwrap();
        assert!(!cleaned.contains("127.0.0.1:8899"));
        assert!(cleaned.contains("\"workbench.colorTheme\": \"Default Dark+\""));

        // Test Claude Code ~/.claude/settings.json apply and cleanup
        let claude_settings = tmp_dir.join(".claude").join("settings.json");
        assert!(apply_claude_code_settings_file(&claude_settings, "http://127.0.0.1:8899").unwrap());
        let claude_content = fs::read_to_string(&claude_settings).unwrap();
        assert!(claude_content.contains("\"HTTPS_PROXY\": \"http://127.0.0.1:8899\""));

        assert!(cleanup_claude_code_settings_file(&claude_settings).unwrap());
        let claude_cleaned = fs::read_to_string(&claude_settings).unwrap();
        assert!(!claude_cleaned.contains("127.0.0.1:8899"));

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_shell_rc_with_timezone() {
        let initial = "export PATH=\"$HOME/bin:$PATH\"\n";
        let updated = upsert_shell_rc_content_with_tz(
            initial,
            "http://127.0.0.1:8899",
            8899,
            Some("Europe/London"),
        );
        assert!(updated.contains("export TZ=\"Europe/London\""));
        assert!(updated.contains("export LANG=\"en_US.UTF-8\""));
        assert!(updated.contains("export LC_TIME=\"en_US.UTF-8\""));
        assert!(updated.contains("export LC_ALL=\"en_US.UTF-8\""));
        assert!(updated.contains("export HTTPS_PROXY=\"http://127.0.0.1:8899\""));
    }

    #[test]
    fn test_sanitize_timezone() {
        assert_eq!(sanitize_timezone("Europe/London"), Some("Europe/London".to_string()));
        assert_eq!(sanitize_timezone(" America/New_York "), Some("America/New_York".to_string()));
        assert_eq!(sanitize_timezone("Etc/GMT+3"), Some("Etc/GMT+3".to_string()));
        assert_eq!(sanitize_timezone(""), None);
        assert_eq!(sanitize_timezone("   "), None);
        assert_eq!(sanitize_timezone("Europe/Berlin; rm -rf /"), None);
        assert_eq!(sanitize_timezone("$(whoami)"), None);
    }

    #[test]
    fn test_powershell_profile_with_timezone() {
        let initial = "# Custom powershell user settings\nWrite-Host 'Welcome'\n";
        let updated = upsert_powershell_profile_content_with_tz(
            initial,
            "http://127.0.0.1:8899",
            Some("Europe/Warsaw"),
        );
        assert!(updated.contains("$__zastavaUp"));
        assert!(updated.contains("GetActiveTcpListeners()"));
        assert!(updated.contains("Port -eq 8899"));
        assert!(updated.contains("$env:HTTP_PROXY = \"http://127.0.0.1:8899\""));
        assert!(updated.contains("$env:TZ = \"Europe/Warsaw\""));
        assert!(updated.contains("$env:LANG = \"en_US.UTF-8\""));
        assert!(updated.contains("$env:LC_TIME = \"en_US.UTF-8\""));
        assert!(updated.contains("Remove-Item Env:HTTP_PROXY"));
        assert!(updated.contains(ZASTAVA_RC_START));
        assert!(updated.contains(ZASTAVA_RC_END));

        // Idempotent upsert replaces block
        let updated2 = upsert_powershell_profile_content_with_tz(
            &updated,
            "http://127.0.0.1:8900",
            Some("Europe/Berlin"),
        );
        assert_eq!(updated2.matches(ZASTAVA_RC_START).count(), 1);
        assert!(updated2.contains("Port -eq 8900"));
        assert!(updated2.contains("http://127.0.0.1:8900"));
        assert!(updated2.contains("Europe/Berlin"));
        assert!(!updated2.contains("Europe/Warsaw"));

        // Cleanup removes block
        let cleaned = remove_shell_rc_block(&updated2);
        assert!(!cleaned.contains(ZASTAVA_RC_START));
        assert!(!cleaned.contains("Europe/Berlin"));
        assert!(cleaned.contains("Write-Host 'Welcome'"));
    }

    #[test]
    fn test_force_clean_all_os_proxy_traces() {
        let cleaned = force_clean_all_os_proxy_traces();
        assert_eq!(cleaned.len(), 3);
        assert!(cleaned.iter().any(|c| c.contains("PAC")));
    }

    #[test]
    fn test_ide_and_claude_settings_with_tz() {
        let tmp_dir = std::env::temp_dir().join(format!("zastava_tz_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&tmp_dir);
        let vscode_settings = tmp_dir.join("settings.json");

        fs::write(&vscode_settings, "{}").unwrap();
        assert!(apply_ide_settings_file_with_tz(
            &vscode_settings,
            "http://127.0.0.1:8899",
            Some("Europe/Amsterdam")
        )
        .unwrap());

        let content = fs::read_to_string(&vscode_settings).unwrap();
        assert!(content.contains("\"TZ\": \"Europe/Amsterdam\""));
        assert!(content.contains("\"LANG\": \"en_US.UTF-8\""));

        // Cleanup removes TZ and LANG
        assert!(cleanup_ide_settings_file(&vscode_settings).unwrap());
        let cleaned = fs::read_to_string(&vscode_settings).unwrap();
        assert!(!cleaned.contains("Europe/Amsterdam"));
        assert!(!cleaned.contains("en_US.UTF-8"));

        // Claude settings
        let claude_settings = tmp_dir.join(".claude").join("settings.json");
        assert!(apply_claude_code_settings_file_with_tz(
            &claude_settings,
            "http://127.0.0.1:8899",
            Some("Europe/Helsinki")
        )
        .unwrap());
        let claude_content = fs::read_to_string(&claude_settings).unwrap();
        assert!(claude_content.contains("\"TZ\": \"Europe/Helsinki\""));
        assert!(claude_content.contains("\"LANG\": \"en_US.UTF-8\""));

        assert!(cleanup_claude_code_settings_file(&claude_settings).unwrap());
        let claude_cleaned = fs::read_to_string(&claude_settings).unwrap();
        assert!(!claude_cleaned.contains("Europe/Helsinki"));

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
