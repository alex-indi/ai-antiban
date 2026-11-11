//! VS Code / Cursor / Windsurf / VSCodium / Antigravity `settings.json` handling (JSONC-safe).

use super::config_settings;
#[cfg(not(target_os = "windows"))]
use super::user_home_dir;
use serde_json::json;
use std::path::{Path, PathBuf};

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

/// Меняет отдельные ключи с сохранением исходных значений и текста JSONC.
pub fn apply_ide_settings_file(path: &Path, proxy: &str) -> Result<bool, String> {
    apply_ide_settings_file_with_tz(path, proxy, None)
}
pub fn apply_ide_settings_file_with_tz(
    path: &Path,
    proxy: &str,
    tz: Option<&str>,
) -> Result<bool, String> {
    let mut changes = super::ai_cli::env_changes(ide_env_key(), proxy, tz);
    changes.push((vec!["http.proxy".into()], json!(proxy)));
    changes.push((vec!["http.proxySupport".into()], json!("override")));
    config_settings::apply_json(path, changes)
}
pub fn cleanup_ide_settings_file(path: &Path) -> Result<bool, String> {
    config_settings::cleanup_json(path)
}

pub fn candidate_ide_settings_paths() -> Vec<(&'static str, PathBuf)> {
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
