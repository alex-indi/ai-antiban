//! Application network coverage and incoming socket process auditing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Individual application status within Zastava network protection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppCoverageItem {
    pub id: String,
    pub name: String,
    pub category: String, // "browser" | "desktop" | "ide" | "cli"
    pub installed: bool,
    pub status: String, // "verified" | "configured" | "unconfigured"
    pub requests_seen: u64,
    pub detail: String,
}

/// Comprehensive snapshot of application protection coverage and proxy chaining.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageSnapshot {
    pub sysproxy_enabled: bool,
    pub cli_protection_enabled: bool,
    pub chained_proxy: Option<String>,
    pub apps: Vec<AppCoverageItem>,
}

#[derive(Debug, Clone)]
struct AppTrafficStats {
    pub requests_seen: u64,
    pub last_seen_ms: u64,
    pub detected_name: String,
    pub category: String,
}

/// Live process accounting engine tracking incoming local gateway sockets.
#[derive(Default)]
pub struct CoverageTracker {
    stats: Mutex<HashMap<String, AppTrafficStats>>,
    port_cache: Mutex<HashMap<u16, (String, Instant)>>,
}

impl CoverageTracker {
    pub fn new() -> Self {
        Self {
            stats: Mutex::new(HashMap::new()),
            port_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Records an incoming connection from `peer_port`.
    /// Performs non-blocking, cached process resolution without delaying proxy tunneling.
    pub fn record_incoming(self: &Arc<Self>, peer_port: u16, user_agent: Option<String>) {
        // 1. Check recent port cache (connections from the same port within 15 seconds)
        {
            if let Ok(cache) = self.port_cache.lock() {
                if let Some((proc_name, instant)) = cache.get(&peer_port) {
                    if instant.elapsed().as_secs() < 15 {
                        let name_clone = proc_name.clone();
                        drop(cache);
                        self.record_hit(&name_clone, user_agent.as_deref());
                        return;
                    }
                }
            }
        }

        // 2. Fast check: can we identify the client directly from User-Agent?
        if let Some(ref ua) = user_agent {
            let ua_lower = ua.to_lowercase();
            if ua_lower.starts_with("curl/") {
                self.cache_port(peer_port, "curl".to_string());
                self.record_hit("curl", Some(ua));
                return;
            }
            if ua_lower.contains("claude-code") || ua_lower.contains("claude_code") {
                self.cache_port(peer_port, "claude-code".to_string());
                self.record_hit("claude-code", Some(ua));
                return;
            }
            if ua_lower.contains("python-requests") || ua_lower.contains("aiohttp") {
                self.cache_port(peer_port, "python".to_string());
                self.record_hit("python", Some(ua));
                return;
            }
            if ua_lower.contains("node-fetch") || ua_lower.contains("undici") {
                self.cache_port(peer_port, "node".to_string());
                self.record_hit("node", Some(ua));
                return;
            }
        }

        // 3. Background asynchronous OS process lookup (never blocks the tunnel)
        let tracker = Arc::clone(self);
        tokio::spawn(async move {
            if let Some(proc_name) = lookup_process_name_by_port(peer_port).await {
                tracker.cache_port(peer_port, proc_name.clone());
                tracker.record_hit(&proc_name, user_agent.as_deref());
            } else if let Some(ua) = user_agent {
                tracker.record_hit(&ua, None);
            }
        });
    }

    fn cache_port(&self, port: u16, name: String) {
        if let Ok(mut cache) = self.port_cache.lock() {
            cache.insert(port, (name, Instant::now()));
        }
    }

    /// Records a verified request from a discovered process name or user-agent.
    pub fn record_hit(&self, raw_process_name: &str, user_agent: Option<&str>) {
        let (id, display_name, category) = resolve_app_identity(raw_process_name, user_agent);
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if let Ok(mut lock) = self.stats.lock() {
            let entry = lock.entry(id).or_insert_with(|| AppTrafficStats {
                requests_seen: 0,
                last_seen_ms: now_ms,
                detected_name: display_name,
                category,
            });
            entry.requests_seen = entry.requests_seen.saturating_add(1);
            entry.last_seen_ms = now_ms;
        }
    }

    /// Generates a complete coverage snapshot of all known catalog apps and any live detected processes.
    pub fn get_snapshot(
        &self,
        sysproxy_enabled: bool,
        cli_protection_enabled: bool,
        chained_proxy: Option<String>,
    ) -> CoverageSnapshot {
        let stats_map = self
            .stats
            .lock()
            .map(|l| l.clone())
            .unwrap_or_default();

        let known_ids = [
            "browsers",
            "claude-desktop",
            "chatgpt-desktop",
            "antigravity",
            "cursor",
            "vscode",
            "windsurf",
            "jetbrains",
            "claude-code",
            "gemini-cli",
            "node-cli",
            "python-cli",
            "curl",
        ];

        let mut apps = Vec::new();

        // 1. Process standard known apps catalog
        for app_id in known_ids {
            let info = get_known_app_info(app_id).unwrap_or(KnownAppInfo {
                id: app_id,
                name: app_id,
                category: "desktop",
            });

            let traffic = stats_map.get(app_id);
            let requests_seen = traffic.map(|t| t.requests_seen).unwrap_or(0);
            let installed = check_app_installed(app_id) || requests_seen > 0;

            let (status, detail) = if requests_seen > 0 {
                (
                    "verified".to_string(),
                    format!("Проверено трафиком ({} запр.)", requests_seen),
                )
            } else if installed {
                match info.category {
                    "browser" => {
                        if sysproxy_enabled {
                            ("configured".to_string(), "Через системный PAC".to_string())
                        } else {
                            ("unconfigured".to_string(), "Включите системный PAC".to_string())
                        }
                    }
                    "desktop" => {
                        if sysproxy_enabled {
                            ("configured".to_string(), "Через системный PAC".to_string())
                        } else {
                            ("unconfigured".to_string(), "Включите системный PAC".to_string())
                        }
                    }
                    "ide" => {
                        if app_id == "jetbrains" {
                            (
                                "unconfigured".to_string(),
                                "Требуется ручная настройка (127.0.0.1:8899)".to_string(),
                            )
                        } else if is_ide_configured(app_id) || cli_protection_enabled {
                            ("configured".to_string(), "Настроен settings.json".to_string())
                        } else if sysproxy_enabled {
                            ("configured".to_string(), "Через системный PAC".to_string())
                        } else {
                            (
                                "unconfigured".to_string(),
                                "Включите защиту IDE и CLI".to_string(),
                            )
                        }
                    }
                    "cli" => {
                        if cli_protection_enabled {
                            (
                                "configured".to_string(),
                                "Через settings.json / переменные окружения".to_string(),
                            )
                        } else {
                            (
                                "unconfigured".to_string(),
                                "Включите защиту IDE и CLI".to_string(),
                            )
                        }
                    }
                    _ => (
                        "unconfigured".to_string(),
                        "Ожидание настройки".to_string(),
                    ),
                }
            } else {
                ("unconfigured".to_string(), "Не установлено".to_string())
            };

            apps.push(AppCoverageItem {
                id: info.id.to_string(),
                name: info.name.to_string(),
                category: info.category.to_string(),
                installed,
                status,
                requests_seen,
                detail,
            });
        }

        // 2. Append dynamically detected processes not present in the static catalog
        for (id, traffic) in &stats_map {
            if !known_ids.contains(&id.as_str()) {
                apps.push(AppCoverageItem {
                    id: id.clone(),
                    name: traffic.detected_name.clone(),
                    category: traffic.category.clone(),
                    installed: true,
                    status: "verified".to_string(),
                    requests_seen: traffic.requests_seen,
                    detail: format!("Проверено трафиком ({} запр.)", traffic.requests_seen),
                });
            }
        }

        CoverageSnapshot {
            sysproxy_enabled,
            cli_protection_enabled,
            chained_proxy,
            apps,
        }
    }
}

pub struct KnownAppInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
}

pub fn get_known_app_info(id: &str) -> Option<KnownAppInfo> {
    match id {
        "browsers" => Some(KnownAppInfo {
            id: "browsers",
            name: "Браузеры (Chrome, Safari, Arc, Edge...)",
            category: "browser",
        }),
        "claude-desktop" => Some(KnownAppInfo {
            id: "claude-desktop",
            name: "Claude Desktop",
            category: "desktop",
        }),
        "chatgpt-desktop" => Some(KnownAppInfo {
            id: "chatgpt-desktop",
            name: "ChatGPT Desktop",
            category: "desktop",
        }),
        "antigravity" => Some(KnownAppInfo {
            id: "antigravity",
            name: "Antigravity",
            category: "ide",
        }),
        "cursor" => Some(KnownAppInfo {
            id: "cursor",
            name: "Cursor",
            category: "ide",
        }),
        "vscode" => Some(KnownAppInfo {
            id: "vscode",
            name: "VS Code",
            category: "ide",
        }),
        "windsurf" => Some(KnownAppInfo {
            id: "windsurf",
            name: "Windsurf",
            category: "ide",
        }),
        "jetbrains" => Some(KnownAppInfo {
            id: "jetbrains",
            name: "JetBrains IDE (PyCharm, IntelliJ...)",
            category: "ide",
        }),
        "claude-code" => Some(KnownAppInfo {
            id: "claude-code",
            name: "Claude Code",
            category: "cli",
        }),
        "gemini-cli" => Some(KnownAppInfo {
            id: "gemini-cli",
            name: "Gemini CLI",
            category: "cli",
        }),
        "node-cli" => Some(KnownAppInfo {
            id: "node-cli",
            name: "Node.js / CLI",
            category: "cli",
        }),
        "python-cli" => Some(KnownAppInfo {
            id: "python-cli",
            name: "Python / CLI",
            category: "cli",
        }),
        "curl" => Some(KnownAppInfo {
            id: "curl",
            name: "curl",
            category: "cli",
        }),
        _ => None,
    }
}

/// Normalizes raw OS process names to canonical Zastava app identifiers.
pub fn classify_process_name(name: &str) -> &'static str {
    let lower_buf = name.to_lowercase();
    let lower = lower_buf.trim().trim_end_matches(".exe");

    if lower.contains("claude-code") || lower.contains("claude_code") {
        return "claude-code";
    }
    if lower.starts_with("claude") || lower.contains("claude helper") {
        return "claude-desktop";
    }
    if lower.contains("chatgpt") {
        return "chatgpt-desktop";
    }
    if lower.contains("antigravity") {
        return "antigravity";
    }
    if lower.contains("cursor") {
        return "cursor";
    }
    if lower == "code"
        || lower.starts_with("code ")
        || lower.contains("code helper")
        || lower.contains("vscode")
        || lower.contains("vscodium")
    {
        return "vscode";
    }
    if lower.contains("windsurf") {
        return "windsurf";
    }
    if lower.contains("chrome")
        || lower.contains("safari")
        || lower == "arc"
        || lower.starts_with("arc ")
        || lower.contains("arc helper")
        || lower.contains("edge")
        || lower.contains("msedge")
        || lower.contains("yandex")
        || lower.contains("яндекс")
        || lower.contains("firefox")
        || lower.contains("brave")
    {
        return "browsers";
    }
    if lower.contains("idea")
        || lower.contains("pycharm")
        || lower.contains("webstorm")
        || lower.contains("datagrip")
        || lower.contains("clion")
        || lower.contains("rider")
        || lower.contains("goland")
        || lower.contains("rubymine")
        || lower.contains("phpstorm")
        || lower.contains("fleet")
        || lower.contains("studio")
    {
        return "jetbrains";
    }
    if lower == "gemini" || lower.contains("gemini-cli") {
        return "gemini-cli";
    }
    if lower == "node" || lower.starts_with("node ") {
        return "node-cli";
    }
    if lower.starts_with("python") {
        return "python-cli";
    }
    if lower == "curl" {
        return "curl";
    }

    "other"
}

/// Resolves an app identifier, human-readable name, and category from process info.
pub fn resolve_app_identity(
    raw_process_name: &str,
    user_agent: Option<&str>,
) -> (String, String, String) {
    if let Some(ua) = user_agent {
        let ua_lower = ua.to_lowercase();
        if ua_lower.starts_with("curl/") {
            return ("curl".to_string(), "curl".to_string(), "cli".to_string());
        }
        if ua_lower.contains("claude-code") {
            return (
                "claude-code".to_string(),
                "Claude Code".to_string(),
                "cli".to_string(),
            );
        }
    }

    let classified = classify_process_name(raw_process_name);
    if classified != "other" {
        if let Some(info) = get_known_app_info(classified) {
            return (
                info.id.to_string(),
                info.name.to_string(),
                info.category.to_string(),
            );
        }
    }

    let clean_id = raw_process_name
        .to_lowercase()
        .replace([' ', '_', '/'], "-");
    (clean_id, raw_process_name.to_string(), "cli".to_string())
}

/// Checks whether an application is installed or its config/settings directory exists on disk.
pub fn check_app_installed(id: &str) -> bool {
    match id {
        "browsers" | "curl" => true,
        "claude-desktop" => {
            #[cfg(target_os = "macos")]
            {
                Path::new("/Applications/Claude.app").exists()
                    || dirs_home()
                        .map(|h| {
                            h.join("Applications/Claude.app").exists()
                                || h.join("Library/Application Support/Claude").exists()
                        })
                        .unwrap_or(false)
            }
            #[cfg(target_os = "windows")]
            {
                std::env::var_os("APPDATA")
                    .map(|a| PathBuf::from(a).join("Claude").exists())
                    .unwrap_or(false)
                    || std::env::var_os("LOCALAPPDATA")
                        .map(|l| PathBuf::from(l).join("Programs/Claude").exists())
                        .unwrap_or(false)
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                dirs_home()
                    .map(|h| h.join(".config/Claude").exists())
                    .unwrap_or(false)
            }
        }
        "chatgpt-desktop" => {
            #[cfg(target_os = "macos")]
            {
                Path::new("/Applications/ChatGPT.app").exists()
                    || dirs_home()
                        .map(|h| {
                            h.join("Applications/ChatGPT.app").exists()
                                || h.join("Library/Application Support/ChatGPT").exists()
                        })
                        .unwrap_or(false)
            }
            #[cfg(target_os = "windows")]
            {
                std::env::var_os("APPDATA")
                    .map(|a| PathBuf::from(a).join("ChatGPT").exists())
                    .unwrap_or(false)
                    || std::env::var_os("LOCALAPPDATA")
                        .map(|l| PathBuf::from(l).join("Programs/ChatGPT").exists())
                        .unwrap_or(false)
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                dirs_home()
                    .map(|h| h.join(".config/ChatGPT").exists())
                    .unwrap_or(false)
            }
        }
        "antigravity" => {
            has_ide_folder("Antigravity")
                || Path::new("/Applications/Antigravity.app").exists()
                || dirs_home()
                    .map(|h| h.join(".gemini/antigravity-cli").exists())
                    .unwrap_or(false)
        }
        "cursor" => {
            has_ide_folder("Cursor") || Path::new("/Applications/Cursor.app").exists()
        }
        "vscode" => {
            has_ide_folder("Code")
                || has_ide_folder("VSCodium")
                || Path::new("/Applications/Visual Studio Code.app").exists()
        }
        "windsurf" => {
            has_ide_folder("Windsurf") || Path::new("/Applications/Windsurf.app").exists()
        }
        "jetbrains" => {
            #[cfg(target_os = "macos")]
            {
                dirs_home()
                    .map(|h| h.join("Library/Application Support/JetBrains").exists())
                    .unwrap_or(false)
                    || Path::new("/Applications/IntelliJ IDEA.app").exists()
                    || Path::new("/Applications/PyCharm.app").exists()
                    || Path::new("/Applications/WebStorm.app").exists()
            }
            #[cfg(target_os = "windows")]
            {
                std::env::var_os("APPDATA")
                    .map(|a| PathBuf::from(a).join("JetBrains").exists())
                    .unwrap_or(false)
                    || std::env::var_os("LOCALAPPDATA")
                        .map(|l| PathBuf::from(l).join("JetBrains").exists())
                        .unwrap_or(false)
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                dirs_home()
                    .map(|h| h.join(".config/JetBrains").exists())
                    .unwrap_or(false)
            }
        }
        "claude-code" => dirs_home()
            .map(|h| h.join(".claude").exists())
            .unwrap_or(false),
        "gemini-cli" => dirs_home()
            .map(|h| h.join(".gemini").exists())
            .unwrap_or(false),
        "node-cli" => {
            dirs_home()
                .map(|h| h.join(".nvm").exists() || h.join(".fnm").exists())
                .unwrap_or(false)
                || Path::new("/usr/local/bin/node").exists()
                || Path::new("/opt/homebrew/bin/node").exists()
                || std::env::var_os("ProgramFiles")
                    .map(|p| PathBuf::from(p).join("nodejs").exists())
                    .unwrap_or(false)
        }
        "python-cli" => {
            Path::new("/usr/bin/python3").exists()
                || Path::new("/usr/local/bin/python3").exists()
                || Path::new("/opt/homebrew/bin/python3").exists()
                || std::env::var_os("LOCALAPPDATA")
                    .map(|l| PathBuf::from(l).join("Programs/Python").exists())
                    .unwrap_or(false)
        }
        _ => false,
    }
}

fn dirs_home() -> Option<PathBuf> {
    crate::sysproxy::user_home_dir()
}

fn has_ide_folder(folder_name: &str) -> bool {
    for (label, path) in crate::sysproxy::candidate_ide_settings_paths() {
        if label.eq_ignore_ascii_case(folder_name)
            || path.to_string_lossy().contains(folder_name)
        {
            if path.parent().map(|p| p.is_dir()).unwrap_or(false) {
                return true;
            }
        }
    }
    false
}

fn is_ide_configured(id: &str) -> bool {
    let folder_name = match id {
        "antigravity" => "Antigravity",
        "cursor" => "Cursor",
        "vscode" => "Code",
        "windsurf" => "Windsurf",
        _ => return false,
    };
    for (label, path) in crate::sysproxy::candidate_ide_settings_paths() {
        if label.eq_ignore_ascii_case(folder_name)
            || path.to_string_lossy().contains(folder_name)
        {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if content.contains("http.proxy")
                        && (content.contains("127.0.0.1") || content.contains("8899"))
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Asynchronously resolves process name owning `peer_port`.
pub async fn lookup_process_name_by_port(port: u16) -> Option<String> {
    tokio::task::spawn_blocking(move || lookup_process_name_by_port_sync(port))
        .await
        .ok()
        .flatten()
}

/// Synchronous OS-level process resolution for an established TCP port.
pub fn lookup_process_name_by_port_sync(port: u16) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        lookup_process_macos(port)
    }
    #[cfg(target_os = "windows")]
    {
        lookup_process_windows(port)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        lookup_process_linux(port)
    }
}

#[cfg(target_os = "macos")]
fn lookup_process_macos(port: u16) -> Option<String> {
    let output = crate::sysproxy::silent_command("lsof")
        .args(["-nP", "-Fc", "-Fn", &format!("-iTCP:{port}")])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let my_pid = std::process::id();
    let mut current_pid = None;
    let mut current_cmd = None;

    for line in text.lines() {
        if line.starts_with('p') {
            current_pid = line[1..].parse::<u32>().ok();
            current_cmd = None;
        } else if line.starts_with('c') {
            current_cmd = Some(line[1..].trim().to_string());
        } else if line.starts_with('n') {
            if let Some(cmd) = &current_cmd {
                if current_pid != Some(my_pid)
                    && cmd != "zastava"
                    && cmd != "zastava_lib"
                {
                    return Some(cmd.clone());
                }
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn lookup_process_windows(port: u16) -> Option<String> {
    let output = crate::sysproxy::silent_command("netstat")
        .args(["-ano", "-p", "tcp"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let port_str = format!(":{port}");
    let mut target_pid = None;
    let my_pid = std::process::id();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("TCP") && trimmed.contains(&port_str) {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if let Some(pid_str) = parts.last() {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    if pid != my_pid && pid != 0 {
                        target_pid = Some(pid);
                        break;
                    }
                }
            }
        }
    }

    let pid = target_pid?;
    let task_output = crate::sysproxy::silent_command("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .ok()?;
    if !task_output.status.success() {
        return None;
    }
    let task_text = String::from_utf8_lossy(&task_output.stdout);
    for line in task_text.lines() {
        let clean = line.trim();
        if clean.starts_with('"') {
            let name = clean.trim_start_matches('"').split('"').next()?;
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

#[cfg(all(unix, not(target_os = "macos")))]
fn lookup_process_linux(port: u16) -> Option<String> {
    let output = crate::sysproxy::silent_command("ss")
        .args(["-t", "-p", "-H"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let port_str = format!(":{port}");
    for line in text.lines() {
        if line.contains(&port_str) && line.contains("users:((") {
            if let Some(after) = line.split("users:((\"").nth(1) {
                if let Some(name) = after.split('"').next() {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_process_name_all_categories() {
        // AI Desktop apps
        assert_eq!(classify_process_name("Claude"), "claude-desktop");
        assert_eq!(classify_process_name("Claude Helper"), "claude-desktop");
        assert_eq!(classify_process_name("claude.exe"), "claude-desktop");
        assert_eq!(classify_process_name("ChatGPT"), "chatgpt-desktop");
        assert_eq!(classify_process_name("chatgpt.exe"), "chatgpt-desktop");

        // IDEs & AI Agents
        assert_eq!(classify_process_name("Antigravity"), "antigravity");
        assert_eq!(classify_process_name("Cursor"), "cursor");
        assert_eq!(classify_process_name("Cursor Helper"), "cursor");
        assert_eq!(classify_process_name("Code"), "vscode");
        assert_eq!(classify_process_name("Code Helper"), "vscode");
        assert_eq!(classify_process_name("vscode.exe"), "vscode");
        assert_eq!(classify_process_name("Windsurf"), "windsurf");
        assert_eq!(classify_process_name("windsurf.exe"), "windsurf");

        // JetBrains family
        assert_eq!(classify_process_name("idea"), "jetbrains");
        assert_eq!(classify_process_name("PyCharm"), "jetbrains");
        assert_eq!(classify_process_name("pycharm64.exe"), "jetbrains");
        assert_eq!(classify_process_name("WebStorm"), "jetbrains");

        // Browsers
        assert_eq!(classify_process_name("Google Chrome Helper"), "browsers");
        assert_eq!(classify_process_name("Chrome"), "browsers");
        assert_eq!(classify_process_name("Safari"), "browsers");
        assert_eq!(classify_process_name("Arc"), "browsers");
        assert_eq!(classify_process_name("msedge.exe"), "browsers");
        assert_eq!(classify_process_name("Яндекс Браузер"), "browsers");
        assert_eq!(classify_process_name("Firefox"), "browsers");
        assert_eq!(classify_process_name("Brave"), "browsers");

        // CLI
        assert_eq!(classify_process_name("claude-code"), "claude-code");
        assert_eq!(classify_process_name("gemini"), "gemini-cli");
        assert_eq!(classify_process_name("node"), "node-cli");
        assert_eq!(classify_process_name("python3"), "python-cli");
        assert_eq!(classify_process_name("curl"), "curl");

        // Unclassified fallback
        assert_eq!(classify_process_name("unknown-binary"), "other");
    }

    #[test]
    fn test_coverage_tracker_recording_and_snapshot() {
        let tracker = Arc::new(CoverageTracker::new());
        tracker.record_hit("Claude", None);
        tracker.record_hit("Google Chrome Helper", None);
        tracker.record_hit("curl", Some("curl/8.7.1"));

        let snap = tracker.get_snapshot(true, true, Some("127.0.0.1:1080".to_string()));
        assert!(snap.sysproxy_enabled);
        assert!(snap.cli_protection_enabled);
        assert_eq!(snap.chained_proxy.as_deref(), Some("127.0.0.1:1080"));

        let claude = snap.apps.iter().find(|a| a.id == "claude-desktop").unwrap();
        assert_eq!(claude.status, "verified");
        assert_eq!(claude.requests_seen, 1);
        assert!(claude.detail.contains("1 запр."));

        let browsers = snap.apps.iter().find(|a| a.id == "browsers").unwrap();
        assert_eq!(browsers.status, "verified");
        assert_eq!(browsers.requests_seen, 1);

        let curl = snap.apps.iter().find(|a| a.id == "curl").unwrap();
        assert_eq!(curl.status, "verified");
        assert_eq!(curl.requests_seen, 1);
    }
}
