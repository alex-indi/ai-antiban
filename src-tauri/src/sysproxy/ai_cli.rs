//! Claude Code (`~/.claude/settings.json`) and Gemini CLI (`~/.gemini/settings.json`) settings.

use super::ide::sanitize_jsonc;
use super::{is_zastava_local_proxy_val, NO_PROXY_VALUE};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

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
