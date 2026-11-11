//! Routes: per-provider egress through the entry nodes of an imported key.
//!
//! A [`Route`] is the parsed form of a key (`zastava://…`, `socks5://…`, `https://…`). Providers are
//! bound to routes in [`crate::state::AppStateData`]; the gateway resolves the effective route of a
//! provider with [`select_route_id`]:
//!
//! 1. `anthropic` only: the route of the active Claude profile (`profile_route_id`, mirrored from
//!    the profile's `bound_upstream_key` on switch / bind);
//! 2. explicit `provider_routes[provider]`;
//! 3. `default_route_id`;
//! 4. otherwise direct (through whatever network / VPN the user has).
//!
//! Once a route is selected it is fail-closed: if it is disabled, expired, missing or unreachable
//! the connection is refused and never falls back to a direct connection.

pub mod key;

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::state::{AppStateData, UpstreamProxyConfig, PROVIDER_ANTHROPIC};

pub use key::parse_route_key;

/// Maximum number of entry nodes in one key.
pub const MAX_ENTRIES: usize = 8;

fn default_true() -> bool {
    true
}

/// Transport used to reach an entry node.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum EntryProto {
    /// HTTP CONNECT proxy wrapped in TLS (certificate is always verified).
    Https,
    /// SOCKS5 (domain names are resolved by the node).
    Socks5,
    /// Plain-text HTTP CONNECT proxy.
    Http,
}

impl EntryProto {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntryProto::Https => "https",
            EntryProto::Socks5 => "socks5",
            EntryProto::Http => "http",
        }
    }

    pub fn from_scheme(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "https" => Some(EntryProto::Https),
            "socks5" | "socks5h" => Some(EntryProto::Socks5),
            "http" => Some(EntryProto::Http),
            _ => None,
        }
    }

    pub fn default_port(&self) -> u16 {
        match self {
            EntryProto::Https => 443,
            EntryProto::Socks5 => 1080,
            EntryProto::Http => 80,
        }
    }
}

/// One entry node of a route.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub proto: EntryProto,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    /// TLS server name for `https` entries (defaults to `host`).
    #[serde(default)]
    pub sni: Option<String>,
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry")
            .field("proto", &self.proto)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username.as_deref().map(mask_secret))
            .field("password", &self.password.as_ref().map(|_| "***"))
            .field("sni", &self.sni)
            .finish()
    }
}

impl Entry {
    /// `host:port` with IPv6 literals bracketed (no credentials).
    pub fn endpoint(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    pub fn tls_server_name(&self) -> &str {
        self.sni.as_deref().filter(|s| !s.is_empty()).unwrap_or(&self.host)
    }
}

/// An imported key: where the traffic of the bound providers leaves to the Internet.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Route {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub country: Option<String>,
    /// Exit IP promised by the key (checked against the observed one, see [`RouteStatus::ExitIpMismatch`]).
    #[serde(default)]
    pub exit_ip: Option<String>,
    /// Unix seconds; the route is fail-closed once this moment has passed.
    #[serde(default)]
    pub expires_at: Option<u64>,
    /// Entry nodes tried in order.
    pub entries: Vec<Entry>,
    pub raw_key: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub added_at: u64,
}

impl std::fmt::Debug for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Route")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("country", &self.country)
            .field("exit_ip", &self.exit_ip)
            .field("expires_at", &self.expires_at)
            .field("entries", &self.entries)
            .field("raw_key", &"***")
            .field("enabled", &self.enabled)
            .field("added_at", &self.added_at)
            .finish()
    }
}

/// Why a selected route cannot carry traffic right now.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteDownReason {
    Disabled,
    Expired,
    NoEntries,
    /// The binding points to a route id that no longer exists.
    Missing,
    /// Every entry node failed.
    Unreachable,
}

impl RouteDownReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            RouteDownReason::Disabled => "disabled",
            RouteDownReason::Expired => "expired",
            RouteDownReason::NoEntries => "no_entries",
            RouteDownReason::Missing => "missing",
            RouteDownReason::Unreachable => "unreachable",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            RouteDownReason::Disabled => "route is disabled",
            RouteDownReason::Expired => "route key has expired",
            RouteDownReason::NoEntries => "route has no entry nodes",
            RouteDownReason::Missing => "bound route no longer exists",
            RouteDownReason::Unreachable => "all entry nodes of the route are unreachable",
        }
    }
}

impl Route {
    pub fn is_expired_at(&self, now: u64) -> bool {
        self.expires_at.map(|e| e <= now).unwrap_or(false)
    }

    /// Static checks (no network): enabled, not expired, has entries.
    pub fn check_usable(&self, now: u64) -> Result<(), RouteDownReason> {
        if !self.enabled {
            return Err(RouteDownReason::Disabled);
        }
        if self.is_expired_at(now) {
            return Err(RouteDownReason::Expired);
        }
        if self.entries.is_empty() {
            return Err(RouteDownReason::NoEntries);
        }
        Ok(())
    }

    /// Converts a v1.3.x single upstream config. No validation: the user already accepted it.
    pub fn from_legacy(cfg: &UpstreamProxyConfig, now: u64) -> Option<Route> {
        let proto = EntryProto::from_scheme(&cfg.protocol)?;
        let host = cfg.host.trim().trim_matches(|c| c == '[' || c == ']').to_string();
        if host.is_empty() || cfg.port == 0 {
            return None;
        }
        Some(Route {
            id: route_id_for_raw_key(&cfg.raw_key),
            label: cfg
                .label
                .clone()
                .filter(|l| !l.trim().is_empty())
                .unwrap_or_else(|| host.clone()),
            country: None,
            exit_ip: None,
            expires_at: None,
            entries: vec![Entry {
                proto,
                host,
                port: cfg.port,
                username: cfg.username.clone().filter(|u| !u.is_empty()),
                password: cfg.password.clone().filter(|p| !p.is_empty()),
                sni: None,
            }],
            raw_key: cfg.raw_key.clone(),
            enabled: cfg.enabled,
            added_at: now,
        })
    }

    /// Legacy single-proxy view (first entry) for the v1.3.x commands and `get_state`.
    pub fn to_legacy(&self, enabled: bool) -> Option<UpstreamProxyConfig> {
        let e = self.entries.first()?;
        Some(UpstreamProxyConfig {
            enabled,
            raw_key: self.raw_key.clone(),
            protocol: e.proto.as_str().to_string(),
            host: e.host.clone(),
            port: e.port,
            username: e.username.clone(),
            password: e.password.clone(),
            label: Some(self.label.clone()),
        })
    }
}

/// Stable id for keys that do not carry their own: `r-` + 24 hex chars of SHA-256(raw key).
pub fn route_id_for_raw_key(raw_key: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, raw_key.trim().as_bytes());
    let hex: String = digest.as_ref()[..12].iter().map(|b| format!("{b:02x}")).collect();
    format!("r-{hex}")
}

/// Masks a credential for display: `ab***`.
pub fn mask_secret(s: &str) -> String {
    let prefix: String = s.chars().take(2).collect();
    if s.chars().count() <= 2 {
        "***".to_string()
    } else {
        format!("{prefix}***")
    }
}

/// The route id that should carry `provider` traffic, `None` = direct.
pub fn select_route_id(data: &AppStateData, provider: &str) -> Option<String> {
    if provider == PROVIDER_ANTHROPIC {
        if let Some(id) = data.profile_route_id.as_ref() {
            return Some(id.clone());
        }
    }
    if let Some(id) = data.provider_routes.get(provider) {
        return Some(id.clone());
    }
    data.default_route_id.clone()
}

/// Effective routing decision for a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteSelection {
    Direct,
    Route(Route),
    /// Bound to an id that does not exist (treated as down, never as direct).
    Missing(String),
}

pub fn select_route(data: &AppStateData, provider: &str) -> RouteSelection {
    match select_route_id(data, provider) {
        None => RouteSelection::Direct,
        Some(id) => match data.routes.iter().find(|r| r.id == id) {
            Some(r) => RouteSelection::Route(r.clone()),
            None => RouteSelection::Missing(id),
        },
    }
}

// --- Runtime health (not persisted) ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RouteHealth {
    /// `None` until the route was used or tested.
    pub reachable: Option<bool>,
    pub last_error: Option<String>,
    pub observed_exit_ip: Option<String>,
    pub observed_country: Option<String>,
    /// `None` when the key does not declare `exit_ip` or no exit was observed yet.
    pub exit_ip_matches: Option<bool>,
    pub latency_ms: Option<u64>,
    pub checked_at: u64,
}

#[derive(Debug, Default)]
pub struct RouteHealthRegistry {
    inner: RwLock<HashMap<String, RouteHealth>>,
}

pub fn ip_equal(a: &str, b: &str) -> bool {
    match (a.trim().parse::<IpAddr>(), b.trim().parse::<IpAddr>()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a.trim().eq_ignore_ascii_case(b.trim()),
    }
}

impl RouteHealthRegistry {
    pub fn get(&self, id: &str) -> Option<RouteHealth> {
        self.inner.read().ok().and_then(|m| m.get(id).cloned())
    }

    fn update(&self, id: &str, f: impl FnOnce(&mut RouteHealth)) {
        if let Ok(mut m) = self.inner.write() {
            let h = m.entry(id.to_string()).or_default();
            f(h);
            h.checked_at = crate::auditor::network::current_timestamp();
        }
    }

    pub fn mark_reachable(&self, id: &str) {
        self.update(id, |h| {
            h.reachable = Some(true);
            h.last_error = None;
        });
    }

    pub fn mark_unreachable(&self, id: &str, error: &str) {
        let err: String = error.chars().take(300).collect();
        self.update(id, |h| {
            h.reachable = Some(false);
            h.last_error = Some(err);
        });
    }

    pub fn record_latency(&self, id: &str, latency_ms: u64) {
        self.update(id, |h| h.latency_ms = Some(latency_ms));
    }

    /// Records the observed exit; returns whether it matches the key's declared `exit_ip`.
    pub fn record_exit(
        &self,
        id: &str,
        observed_ip: &str,
        observed_country: &str,
        expected_ip: Option<&str>,
    ) -> Option<bool> {
        let matches = expected_ip.map(|e| ip_equal(e, observed_ip));
        let ip = observed_ip.to_string();
        let cc = observed_country.to_string();
        self.update(id, |h| {
            h.reachable = Some(true);
            h.last_error = None;
            h.observed_exit_ip = Some(ip);
            h.observed_country = Some(cc);
            h.exit_ip_matches = matches;
        });
        matches
    }

    pub fn remove(&self, id: &str) {
        if let Ok(mut m) = self.inner.write() {
            m.remove(id);
        }
    }
}

// --- UI views (no secrets) ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteStatus {
    Active,
    Disabled,
    Expired,
    Unreachable,
    ExitIpMismatch,
}

pub fn compute_status(route: &Route, health: Option<&RouteHealth>, now: u64) -> RouteStatus {
    if !route.enabled {
        return RouteStatus::Disabled;
    }
    if route.is_expired_at(now) {
        return RouteStatus::Expired;
    }
    if route.entries.is_empty() {
        return RouteStatus::Unreachable;
    }
    if let Some(h) = health {
        if h.reachable == Some(false) {
            return RouteStatus::Unreachable;
        }
        if h.exit_ip_matches == Some(false) {
            return RouteStatus::ExitIpMismatch;
        }
    }
    RouteStatus::Active
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntryView {
    pub proto: EntryProto,
    pub host: String,
    pub port: u16,
    /// Masked (`ab***`), never the real value.
    pub username: Option<String>,
    pub has_password: bool,
    pub sni: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteView {
    pub id: String,
    pub label: String,
    pub country: Option<String>,
    pub exit_ip: Option<String>,
    pub expires_at: Option<u64>,
    pub enabled: bool,
    pub added_at: u64,
    pub entries: Vec<EntryView>,
    pub status: RouteStatus,
    pub is_default: bool,
    /// Providers explicitly bound via `bind_provider_route`.
    pub bound_providers: Vec<String>,
    /// Route of the active Claude profile (overrides the `anthropic` binding).
    pub is_profile_route: bool,
    pub health: Option<RouteHealth>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutesSnapshot {
    pub routes: Vec<RouteView>,
    pub provider_routes: HashMap<String, String>,
    pub default_route_id: Option<String>,
    pub profile_route_id: Option<String>,
    /// Effective route id per known provider after applying priorities (`null` = direct).
    pub effective: HashMap<String, Option<String>>,
}

pub fn route_view(
    route: &Route,
    data: &AppStateData,
    health: Option<RouteHealth>,
    now: u64,
) -> RouteView {
    let mut bound: Vec<String> = data
        .provider_routes
        .iter()
        .filter(|(_, rid)| **rid == route.id)
        .map(|(p, _)| p.clone())
        .collect();
    bound.sort();
    RouteView {
        id: route.id.clone(),
        label: route.label.clone(),
        country: route.country.clone(),
        exit_ip: route.exit_ip.clone(),
        expires_at: route.expires_at,
        enabled: route.enabled,
        added_at: route.added_at,
        entries: route
            .entries
            .iter()
            .map(|e| EntryView {
                proto: e.proto,
                host: e.host.clone(),
                port: e.port,
                username: e.username.as_deref().map(mask_secret),
                has_password: e.password.as_deref().map(|p| !p.is_empty()).unwrap_or(false),
                sni: e.sni.clone(),
            })
            .collect(),
        status: compute_status(route, health.as_ref(), now),
        is_default: data.default_route_id.as_deref() == Some(route.id.as_str()),
        bound_providers: bound,
        is_profile_route: data.profile_route_id.as_deref() == Some(route.id.as_str()),
        health,
    }
}

pub fn snapshot(data: &AppStateData, health: &RouteHealthRegistry, now: u64) -> RoutesSnapshot {
    let effective = crate::state::KNOWN_PROVIDERS
        .iter()
        .map(|p| (p.to_string(), select_route_id(data, p)))
        .collect();
    RoutesSnapshot {
        routes: data
            .routes
            .iter()
            .map(|r| route_view(r, data, health.get(&r.id), now))
            .collect(),
        provider_routes: data.provider_routes.clone(),
        default_route_id: data.default_route_id.clone(),
        profile_route_id: data.profile_route_id.clone(),
        effective,
    }
}

/// Applies the `bound_upstream_key` of the active Claude profile: the key becomes a route and the
/// highest-priority route for `anthropic`. `None` removes the override.
pub async fn apply_profile_key(
    state: &crate::state::AppState,
    key: Option<&str>,
) -> Result<(), String> {
    match key.map(str::trim).filter(|k| !k.is_empty()) {
        Some(k) => {
            let route = parse_route_key(k, crate::auditor::network::current_timestamp())?;
            let id = state.upsert_route(route).await.map_err(|e| e.to_string())?;
            state.set_profile_route(Some(id)).await.map_err(|e| e.to_string())
        }
        None => state.set_profile_route(None).await.map_err(|e| e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(id: &str) -> Route {
        Route {
            id: id.to_string(),
            label: id.to_string(),
            country: Some("NL".into()),
            exit_ip: Some("185.10.10.1".into()),
            expires_at: None,
            entries: vec![Entry {
                proto: EntryProto::Socks5,
                host: "185.10.10.2".into(),
                port: 1080,
                username: Some("alice".into()),
                password: Some("s3cret".into()),
                sni: None,
            }],
            raw_key: "socks5://alice:s3cret@185.10.10.2:1080".into(),
            enabled: true,
            added_at: 1,
        }
    }

    #[test]
    fn test_selection_priority_profile_provider_default() {
        let mut d = AppStateData::default();
        d.routes = vec![route("a"), route("b"), route("c")];
        assert_eq!(select_route(&d, "openai"), RouteSelection::Direct);

        d.default_route_id = Some("a".into());
        assert_eq!(select_route_id(&d, "openai").as_deref(), Some("a"));

        d.provider_routes.insert("anthropic".into(), "b".into());
        assert_eq!(select_route_id(&d, "anthropic").as_deref(), Some("b"));
        assert_eq!(select_route_id(&d, "gemini").as_deref(), Some("a"));

        d.profile_route_id = Some("c".into());
        assert_eq!(select_route_id(&d, "anthropic").as_deref(), Some("c"));
        // Profile override is Claude-only.
        assert_eq!(select_route_id(&d, "openai").as_deref(), Some("a"));

        d.provider_routes.insert("gemini".into(), "zzz".into());
        assert_eq!(select_route(&d, "gemini"), RouteSelection::Missing("zzz".into()));
    }

    #[test]
    fn test_status_and_usability() {
        let mut r = route("a");
        let now = 1_000;
        assert_eq!(compute_status(&r, None, now), RouteStatus::Active);
        assert!(r.check_usable(now).is_ok());

        let h = RouteHealthRegistry::default();
        assert_eq!(h.record_exit("a", "185.10.10.9", "NL", r.exit_ip.as_deref()), Some(false));
        assert_eq!(compute_status(&r, h.get("a").as_ref(), now), RouteStatus::ExitIpMismatch);
        h.mark_unreachable("a", "refused");
        assert_eq!(compute_status(&r, h.get("a").as_ref(), now), RouteStatus::Unreachable);

        r.expires_at = Some(now);
        assert_eq!(compute_status(&r, None, now), RouteStatus::Expired);
        assert_eq!(r.check_usable(now), Err(RouteDownReason::Expired));
        r.enabled = false;
        assert_eq!(compute_status(&r, None, now), RouteStatus::Disabled);
        assert_eq!(r.check_usable(now), Err(RouteDownReason::Disabled));
    }

    #[test]
    fn test_view_and_debug_hide_secrets() {
        let mut d = AppStateData::default();
        d.routes = vec![route("a")];
        let snap = snapshot(&d, &RouteHealthRegistry::default(), 1);
        let json = serde_json::to_string(&snap).unwrap();
        assert!(!json.contains("s3cret"));
        assert!(!json.contains("alice"));
        assert!(!json.contains("raw_key"));
        assert!(json.contains("\"status\":\"active\""));
        let dbg = format!("{:?}", d.routes[0]);
        assert!(!dbg.contains("s3cret"));
    }

    #[test]
    fn test_route_id_stable() {
        let a = route_id_for_raw_key("socks5://u:p@1.2.3.4:1080");
        assert_eq!(a, route_id_for_raw_key("  socks5://u:p@1.2.3.4:1080 "));
        assert_eq!(a.len(), 26);
        assert_ne!(a, route_id_for_raw_key("socks5://u:p@1.2.3.4:1081"));
    }
}
