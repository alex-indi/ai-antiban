//! Key parsing and strict validation.
//!
//! Accepted forms:
//! * `zastava://<base64 JSON v2>` — `{"v":2,"id":…,"label":…,"country":…,"exit_ip":…,"expires_at":…,"entry":[…]}`;
//! * `zastava://<base64 JSON v1>` — flat `{"protocol","host","port","username","password","label"}`;
//! * `zastava://<base64 URL>` or a bare URL: `socks5://`, `socks5h://`, `http://`, `https://`
//!   `[user:pass@]host[:port][#Label]`.
//!
//! Error messages never contain credentials or the raw key.

use std::net::IpAddr;

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use serde::Deserialize;
use url::Url;

use super::{route_id_for_raw_key, Entry, EntryProto, Route, MAX_ENTRIES};
use crate::proxy::server::{is_forbidden_ip, is_internal_hostname};

pub const MAX_KEY_LEN: usize = 16 * 1024;
pub const MAX_LABEL_LEN: usize = 64;
pub const MAX_ID_LEN: usize = 64;
pub const MAX_HOST_LEN: usize = 253;
pub const MAX_CRED_LEN: usize = 255;
pub const SUPPORTED_KEY_VERSION: u64 = 2;

/// Decodes base64 with standard or URL-safe engines, tolerating absent padding.
fn decode_base64_flexible(input: &str) -> Result<Vec<u8>, String> {
    let clean = input.trim().trim_matches('"');
    if clean.is_empty() {
        return Err("Empty base64 payload".to_string());
    }
    for engine in [&URL_SAFE_NO_PAD, &URL_SAFE, &STANDARD, &STANDARD_NO_PAD] {
        if let Ok(bytes) = engine.decode(clean) {
            return Ok(bytes);
        }
    }
    let rem = clean.len() % 4;
    if rem > 0 {
        let padded = format!("{}{}", clean, "=".repeat(4 - rem));
        for engine in [&STANDARD, &URL_SAFE] {
            if let Ok(bytes) = engine.decode(&padded) {
                return Ok(bytes);
            }
        }
    }
    Err("Could not decode base64 with standard or URL-safe encoding".to_string())
}

/// serde errors can quote offending values (possibly a password), so only the position is kept.
fn json_error(e: &serde_json::Error) -> String {
    let kind = match e.classify() {
        serde_json::error::Category::Io => "I/O error",
        serde_json::error::Category::Syntax => "syntax error",
        serde_json::error::Category::Data => "missing or mistyped field",
        serde_json::error::Category::Eof => "unexpected end of data",
    };
    format!("Invalid key JSON: {kind} at line {}, column {}", e.line(), e.column())
}

fn has_control(s: &str) -> bool {
    s.chars().any(|c| c.is_control())
}

fn clean_label(label: Option<String>, fallback: &str) -> Result<String, String> {
    match label.map(|l| l.trim().to_string()).filter(|l| !l.is_empty()) {
        None => Ok(fallback.chars().take(MAX_LABEL_LEN).collect()),
        Some(l) => {
            if has_control(&l) {
                return Err("Key label contains control characters".to_string());
            }
            if l.chars().count() > MAX_LABEL_LEN {
                return Err(format!("Key label is longer than {MAX_LABEL_LEN} characters"));
            }
            Ok(l)
        }
    }
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > MAX_ID_LEN {
        return Err(format!("Key id must be 1..={MAX_ID_LEN} characters"));
    }
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("Key id may contain only latin letters, digits, '-' and '_'".to_string());
    }
    Ok(())
}

fn validate_domain(h: &str) -> Result<(), String> {
    if h.len() > MAX_HOST_LEN {
        return Err("Entry host is too long".to_string());
    }
    if is_internal_hostname(h) {
        return Err("Entry host must be a public domain, not a local/internal name".to_string());
    }
    let labels: Vec<&str> = h.split('.').collect();
    if labels.len() < 2 {
        return Err("Entry host must be a fully qualified domain or a public IP".to_string());
    }
    for label in labels {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err("Entry host is not a valid domain name".to_string());
        }
    }
    Ok(())
}

/// Normalizes and validates an entry host: a public IP literal or a public domain name.
fn validate_host(raw: &str) -> Result<String, String> {
    let h = raw
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if h.is_empty() {
        return Err("Entry host cannot be empty".to_string());
    }
    if let Ok(ip) = h.parse::<IpAddr>() {
        if is_forbidden_ip(&ip) {
            return Err(
                "Entry host must be a public IP (loopback, private and reserved ranges are rejected)"
                    .to_string(),
            );
        }
        return Ok(ip.to_string());
    }
    validate_domain(&h)?;
    Ok(h)
}

fn validate_cred(value: Option<String>, what: &str, proto: EntryProto) -> Result<Option<String>, String> {
    let Some(v) = value.filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    if v.len() > MAX_CRED_LEN {
        return Err(format!("Entry {what} is longer than {MAX_CRED_LEN} bytes"));
    }
    if has_control(&v) {
        return Err(format!("Entry {what} contains control characters"));
    }
    if what == "username" && proto != EntryProto::Socks5 && v.contains(':') {
        return Err("Entry username must not contain ':' for HTTP(S) entries".to_string());
    }
    Ok(Some(v))
}

fn build_entry(
    proto: EntryProto,
    host: &str,
    port: u16,
    username: Option<String>,
    password: Option<String>,
    sni: Option<String>,
) -> Result<Entry, String> {
    let host = validate_host(host)?;
    if port == 0 {
        return Err("Entry port cannot be 0".to_string());
    }
    let username = validate_cred(username, "username", proto)?;
    let password = validate_cred(password, "password", proto)?;
    if password.is_some() && username.is_none() {
        return Err("Entry password is set without a username".to_string());
    }
    let sni = match sni.map(|s| s.trim().trim_end_matches('.').to_ascii_lowercase()).filter(|s| !s.is_empty()) {
        None => None,
        Some(s) => {
            if proto != EntryProto::Https {
                return Err("Entry 'sni' is only allowed for https entries".to_string());
            }
            if s.parse::<IpAddr>().is_ok() {
                return Err("Entry 'sni' must be a domain name".to_string());
            }
            validate_domain(&s).map_err(|_| "Entry 'sni' is not a valid public domain name".to_string())?;
            Some(s)
        }
    };
    Ok(Entry { proto, host, port, username, password, sni })
}

#[derive(Deserialize)]
struct KeyV1 {
    protocol: String,
    host: String,
    port: u16,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Deserialize)]
struct EntryV2 {
    proto: String,
    host: String,
    port: u16,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    sni: Option<String>,
}

#[derive(Deserialize)]
struct KeyV2 {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    exit_ip: Option<String>,
    #[serde(default)]
    expires_at: Option<u64>,
    entry: Vec<EntryV2>,
}

fn single_entry_route(entry: Entry, label: Option<String>, raw_key: &str, now: u64) -> Result<Route, String> {
    let label = clean_label(label, &entry.host)?;
    Ok(Route {
        id: route_id_for_raw_key(raw_key),
        label,
        country: None,
        exit_ip: None,
        expires_at: None,
        entries: vec![entry],
        raw_key: raw_key.to_string(),
        enabled: true,
        added_at: now,
    })
}

fn parse_v2(value: serde_json::Value, raw_key: &str, now: u64) -> Result<Route, String> {
    let key: KeyV2 = serde_json::from_value(value).map_err(|e| json_error(&e))?;
    if key.entry.is_empty() {
        return Err("Key v2 must contain at least one entry".to_string());
    }
    if key.entry.len() > MAX_ENTRIES {
        return Err(format!("Key v2 has more than {MAX_ENTRIES} entries"));
    }
    let mut entries = Vec::with_capacity(key.entry.len());
    for (i, e) in key.entry.into_iter().enumerate() {
        let proto = EntryProto::from_scheme(&e.proto)
            .ok_or_else(|| format!("Entry #{}: unsupported proto (expected https, socks5 or http)", i + 1))?;
        let entry = build_entry(proto, &e.host, e.port, e.username, e.password, e.sni)
            .map_err(|err| format!("Entry #{}: {err}", i + 1))?;
        entries.push(entry);
    }
    let id = match key.id.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        Some(id) => {
            validate_id(&id)?;
            id
        }
        None => route_id_for_raw_key(raw_key),
    };
    let country = match key.country.map(|c| c.trim().to_ascii_uppercase()).filter(|c| !c.is_empty()) {
        None => None,
        Some(c) if c.len() == 2 && c.chars().all(|ch| ch.is_ascii_uppercase()) => Some(c),
        Some(_) => return Err("Key country must be a 2-letter ISO code".to_string()),
    };
    let exit_ip = match key.exit_ip.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        None => None,
        Some(s) => {
            let ip: IpAddr = s.parse().map_err(|_| "Key exit_ip is not a valid IP address".to_string())?;
            if is_forbidden_ip(&ip) {
                return Err("Key exit_ip must be a public IP".to_string());
            }
            Some(ip.to_string())
        }
    };
    let label = clean_label(key.label, &entries[0].host)?;
    Ok(Route {
        id,
        label,
        country,
        exit_ip,
        expires_at: key.expires_at,
        entries,
        raw_key: raw_key.to_string(),
        enabled: true,
        added_at: now,
    })
}

fn parse_v1(value: serde_json::Value, raw_key: &str, now: u64) -> Result<Route, String> {
    let key: KeyV1 = serde_json::from_value(value).map_err(|e| json_error(&e))?;
    let proto = EntryProto::from_scheme(&key.protocol)
        .ok_or_else(|| "Unsupported protocol in key (expected https, socks5 or http)".to_string())?;
    let entry = build_entry(proto, &key.host, key.port, key.username, key.password, None)?;
    single_entry_route(entry, key.label, raw_key, now)
}

fn parse_json_key(json_str: &str, raw_key: &str, now: u64) -> Result<Route, String> {
    let value: serde_json::Value = serde_json::from_str(json_str).map_err(|e| json_error(&e))?;
    if !value.is_object() {
        return Err("Key JSON must be an object".to_string());
    }
    match value.get("v") {
        None => parse_v1(value, raw_key, now),
        Some(v) => match v.as_u64() {
            Some(SUPPORTED_KEY_VERSION) => parse_v2(value, raw_key, now),
            Some(n) => Err(format!(
                "Unsupported key version v={n}: this Zastava understands v1 and v{SUPPORTED_KEY_VERSION}, update the app"
            )),
            None => Err("Key field 'v' must be a positive integer".to_string()),
        },
    }
}

fn percent_decode_clean(input: &str) -> String {
    url::form_urlencoded::parse(format!("v={input}").as_bytes())
        .next()
        .map(|(_, v)| v.into_owned())
        .unwrap_or_else(|| input.to_string())
}

fn parse_proxy_url(url_str: &str, raw_key: &str, now: u64) -> Result<Route, String> {
    let trimmed = url_str.trim();
    // Support unescaped spaces or Cyrillic text in the fragment (#Label)
    let (url_part, raw_fragment) = match trimmed.split_once('#') {
        Some((u, f)) => (u.trim(), Some(f.trim())),
        None => (trimmed, None),
    };

    let scheme = url_part.split_once("://").map(|(s, _)| s).unwrap_or("");
    let proto = EntryProto::from_scheme(scheme).ok_or_else(|| {
        "Unsupported key format: expected zastava://, socks5://, socks5h://, http:// or https://".to_string()
    })?;

    // url::ParseError does not echo the input, so no credentials leak through it.
    let parsed = Url::parse(url_part).map_err(|e| format!("Invalid proxy URL: {e}"))?;
    if !matches!(parsed.path(), "" | "/") || parsed.query().is_some() {
        return Err("Proxy URL must not contain a path or query".to_string());
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "Proxy URL is missing a host".to_string())?;
    let port = parsed.port().unwrap_or(proto.default_port());
    let username = (!parsed.username().is_empty()).then(|| percent_decode_clean(parsed.username()));
    let password = parsed.password().map(percent_decode_clean);
    let label = raw_fragment.filter(|f| !f.is_empty()).map(percent_decode_clean);

    let entry = build_entry(proto, host, port, username, password, None)?;
    single_entry_route(entry, label, raw_key, now)
}

/// Parses and validates any supported key into a [`Route`] (enabled, `added_at = now`).
pub fn parse_route_key(input: &str, now: u64) -> Result<Route, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Key cannot be empty".to_string());
    }
    if trimmed.len() > MAX_KEY_LEN {
        return Err("Key is too long".to_string());
    }

    if let Some(rest) = trimmed.strip_prefix("zastava://") {
        let bytes = decode_base64_flexible(rest)
            .map_err(|e| format!("Invalid zastava:// base64 payload: {e}"))?;
        let decoded = String::from_utf8(bytes)
            .map_err(|_| "zastava:// payload is not valid UTF-8".to_string())?;
        let decoded = decoded.trim();
        if decoded.starts_with('{') {
            parse_json_key(decoded, trimmed, now)
        } else {
            parse_proxy_url(decoded, trimmed, now)
        }
    } else {
        parse_proxy_url(trimmed, trimmed, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zkey(json: &str) -> String {
        format!("zastava://{}", URL_SAFE_NO_PAD.encode(json.as_bytes()))
    }

    #[test]
    fn test_v2_full_key() {
        let key = zkey(
            r#"{"v":2,"id":"nl-ams-01","label":"NL · Claude","country":"nl","exit_ip":"185.10.10.1","expires_at":1767225600,
               "entry":[{"proto":"https","host":"entry1.example.net","port":443,"username":"u1","password":"p1","sni":"cdn.example.net"},
                        {"proto":"socks5","host":"185.10.10.2","port":1080,"username":"u2","password":"p2"}]}"#,
        );
        let r = parse_route_key(&key, 7).expect("v2 parse");
        assert_eq!(r.id, "nl-ams-01");
        assert_eq!(r.label, "NL · Claude");
        assert_eq!(r.country.as_deref(), Some("NL"));
        assert_eq!(r.exit_ip.as_deref(), Some("185.10.10.1"));
        assert_eq!(r.expires_at, Some(1767225600));
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].proto, EntryProto::Https);
        assert_eq!(r.entries[0].sni.as_deref(), Some("cdn.example.net"));
        assert_eq!(r.entries[1].proto, EntryProto::Socks5);
        assert_eq!(r.added_at, 7);
        assert!(r.enabled);
        assert_eq!(r.raw_key, key);
    }

    #[test]
    fn test_v2_rejections() {
        let unknown = zkey(r#"{"v":3,"entry":[]}"#);
        let err = parse_route_key(&unknown, 0).unwrap_err();
        assert!(err.contains("v=3"), "{err}");

        let empty = zkey(r#"{"v":2,"entry":[]}"#);
        assert!(parse_route_key(&empty, 0).is_err());

        let private = zkey(r#"{"v":2,"entry":[{"proto":"https","host":"10.0.0.5","port":443}]}"#);
        assert!(parse_route_key(&private, 0).is_err());

        let port0 = zkey(r#"{"v":2,"entry":[{"proto":"https","host":"a.example.net","port":0}]}"#);
        assert!(parse_route_key(&port0, 0).is_err());

        let sni_socks = zkey(r#"{"v":2,"entry":[{"proto":"socks5","host":"a.example.net","port":1080,"sni":"x.example.net"}]}"#);
        assert!(parse_route_key(&sni_socks, 0).is_err());

        let bad_id = zkey(r#"{"v":2,"id":"../etc","entry":[{"proto":"https","host":"a.example.net","port":443}]}"#);
        assert!(parse_route_key(&bad_id, 0).is_err());

        let bad_exit = zkey(r#"{"v":2,"exit_ip":"127.0.0.1","entry":[{"proto":"https","host":"a.example.net","port":443}]}"#);
        assert!(parse_route_key(&bad_exit, 0).is_err());

        let crlf = zkey(r#"{"v":2,"entry":[{"proto":"http","host":"a.example.net","port":80,"username":"a\r\nX-Evil: 1","password":"p"}]}"#);
        assert!(parse_route_key(&crlf, 0).is_err());

        let long_label = zkey(&format!(
            r#"{{"v":2,"label":"{}","entry":[{{"proto":"https","host":"a.example.net","port":443}}]}}"#,
            "x".repeat(100)
        ));
        assert!(parse_route_key(&long_label, 0).is_err());
    }

    #[test]
    fn test_errors_never_contain_password() {
        // Password given with a wrong type: serde would normally quote the value.
        let k = zkey(r#"{"v":2,"entry":[{"proto":"https","host":"a.example.net","port":"TopSecret42"}]}"#);
        let err = parse_route_key(&k, 0).unwrap_err();
        assert!(!err.contains("TopSecret42"), "{err}");

        let err = parse_route_key("socks5://user:TopSecret42@10.1.1.1:1080", 0).unwrap_err();
        assert!(!err.contains("TopSecret42"), "{err}");

        let err = parse_route_key("ftp://user:TopSecret42@1.2.3.4:21", 0).unwrap_err();
        assert!(!err.contains("TopSecret42"), "{err}");

        let err = parse_route_key("https://user:TopSecret42@[::1:443", 0).unwrap_err();
        assert!(!err.contains("TopSecret42"), "{err}");
    }

    #[test]
    fn test_https_url_form() {
        let r = parse_route_key("https://bob:p%40ss@entry.example.net:8443#Мой ключ", 0).unwrap();
        assert_eq!(r.entries[0].proto, EntryProto::Https);
        assert_eq!(r.entries[0].host, "entry.example.net");
        assert_eq!(r.entries[0].port, 8443);
        assert_eq!(r.entries[0].username.as_deref(), Some("bob"));
        assert_eq!(r.entries[0].password.as_deref(), Some("p@ss"));
        assert_eq!(r.label, "Мой ключ");
        assert!(r.id.starts_with("r-"));

        let default_port = parse_route_key("https://entry.example.net", 0).unwrap();
        assert_eq!(default_port.entries[0].port, 443);
    }

    #[test]
    fn test_v1_json_and_host_rules() {
        let k = format!(
            "zastava://{}",
            STANDARD.encode(br#"{"protocol":"socks5","host":"45.1.2.3","port":1080,"username":"u1","password":"p1","label":"NL-Fast"}"#)
        );
        let r = parse_route_key(&k, 0).unwrap();
        assert_eq!(r.entries[0].proto, EntryProto::Socks5);
        assert_eq!(r.label, "NL-Fast");

        assert!(parse_route_key("socks5://localhost:1080", 0).is_err());
        assert!(parse_route_key("socks5://127.0.0.1:1080", 0).is_err());
        assert!(parse_route_key("http://router.local:8080", 0).is_err());
        assert!(parse_route_key("http://intranet:8080", 0).is_err());
        assert!(parse_route_key("socks5://[2a01:4f8::1]:1080", 0).is_ok());
        assert!(parse_route_key("socks5://[fd00::1]:1080", 0).is_err());
    }
}
