use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use crate::auditor::geo_matrix::CheckResult;
use crate::state::UpstreamProxyConfig;

pub const CACHE_TTL_SECONDS: u64 = 15;
pub const REQUEST_TIMEOUT_MS: u64 = 3500;

pub const SUSPICIOUS_ANONYMIZER_MARKERS: &[&str] = &[
    "tor-exit",
    "anonymizer",
    "openproxy",
    "public-vpn",
    "socks-proxy",
    "vpn-exit",
    "exit-node",
    "torproject",
    "tor-node",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkTelemetry {
    pub ip: String,
    pub country_code: String,
    #[serde(default)]
    pub hostname: Option<String>,
    pub asn: Option<u32>,
    pub org: Option<String>,
    pub isp: Option<String>,
    pub timezone: Option<String>,
    pub resolved_at: u64,
}

impl NetworkTelemetry {
    pub fn is_fresh(&self, ttl_seconds: u64) -> bool {
        let now = current_timestamp();
        now.saturating_sub(self.resolved_at) < ttl_seconds
    }

    pub fn has_provider_metadata(&self) -> bool {
        self.asn.is_some() || self.isp.is_some() || self.org.is_some()
    }
}

pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Builds a `reqwest::Client` configured to route all traffic through an upstream gateway proxy.
pub fn build_upstream_client(cfg: &UpstreamProxyConfig) -> Result<reqwest::Client, String> {
    let proxy_url = format!("{}://{}:{}", cfg.protocol, cfg.host, cfg.port);
    let mut proxy = reqwest::Proxy::all(&proxy_url)
        .map_err(|e| format!("Failed to configure upstream proxy URL '{proxy_url}': {e}"))?;

    if let Some(ref u) = cfg.username {
        let p = cfg.password.as_deref().unwrap_or("");
        proxy = proxy.basic_auth(u, p);
    }

    reqwest::Client::builder()
        .timeout(Duration::from_millis(REQUEST_TIMEOUT_MS))
        .pool_max_idle_per_host(0)
        .user_agent("Zastava/1.0")
        .proxy(proxy)
        .build()
        .map_err(|e| format!("Failed to build proxy reqwest client: {e}"))
}

// Models for ipwho.is
#[derive(Deserialize)]
struct IpWhoIsResponse {
    ip: Option<String>,
    country_code: Option<String>,
    connection: Option<IpWhoIsConnection>,
    timezone: Option<IpWhoIsTimezone>,
}

#[derive(Deserialize)]
struct IpWhoIsConnection {
    asn: Option<u32>,
    org: Option<String>,
    isp: Option<String>,
    domain: Option<String>,
}

#[derive(Deserialize)]
struct IpWhoIsTimezone {
    id: Option<String>,
}

// Models for ipinfo.io
#[derive(Deserialize)]
struct IpInfoResponse {
    ip: Option<String>,
    hostname: Option<String>,
    country: Option<String>,
    org: Option<String>,
    timezone: Option<String>,
}

// Models for freeipapi.com
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FreeIpApiResponse {
    ip_address: Option<String>,
    country_code: Option<String>,
    asn: Option<String>,
    asn_organization: Option<String>,
    time_zones: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct NetworkResolver {
    client: reqwest::Client,
    cache: Arc<RwLock<Option<(Option<String>, NetworkTelemetry)>>>,
    resolve_lock: Arc<tokio::sync::Mutex<()>>,
}

impl Default for NetworkResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkResolver {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(REQUEST_TIMEOUT_MS))
            // Fresh socket per lookup: a pooled keep-alive connection could still use the old route
            // after a VPN switch and report the previous IP / ASN.
            .pool_max_idle_per_host(0)
            .user_agent("Zastava/1.0")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            client,
            cache: Arc::new(RwLock::new(None)),
            resolve_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// Invalidate existing cached telemetry
    pub async fn invalidate_cache(&self) {
        let mut w = self.cache.write().await;
        *w = None;
    }

    /// Pre-populate or override cached telemetry (useful for deterministic tests and simulation)
    pub async fn set_cached(&self, telemetry: NetworkTelemetry) {
        let mut w = self.cache.write().await;
        *w = Some((None, telemetry));
    }

    /// Pre-populate cached telemetry for a specific upstream proxy key
    pub async fn set_cached_for_key(&self, key: Option<String>, telemetry: NetworkTelemetry) {
        let mut w = self.cache.write().await;
        *w = Some((key, telemetry));
    }

    /// Resolve network telemetry using fresh cache or cascading endpoints (direct egress)
    pub async fn resolve(&self, force_refresh: bool) -> Result<NetworkTelemetry, String> {
        self.resolve_with_upstream(force_refresh, None).await
    }

    /// Resolve network telemetry, routing through upstream proxy gateway if configured and enabled
    pub async fn resolve_with_upstream(
        &self,
        force_refresh: bool,
        upstream: Option<&UpstreamProxyConfig>,
    ) -> Result<NetworkTelemetry, String> {
        let current_key = upstream.filter(|c| c.enabled).map(|c| c.raw_key.clone());

        if !force_refresh {
            let r = self.cache.read().await;
            if let Some((cached_key, telemetry)) = &*r {
                let key_matches = match (&current_key, cached_key) {
                    (Some(k1), Some(k2)) => k1 == k2,
                    (None, None) => true,
                    _ => false,
                };
                let effective_ttl = if telemetry.has_provider_metadata() {
                    CACHE_TTL_SECONDS
                } else {
                    2
                };
                if key_matches && telemetry.is_fresh(effective_ttl) {
                    return Ok(telemetry.clone());
                }
            }
        }

        // Coalesce concurrent cold-start / multi-provider probes so we don't fire 4 simultaneous
        // requests to ipwho.is / ipinfo.io and trigger rate limits or timeouts on startup.
        let _guard = self.resolve_lock.lock().await;

        {
            let r = self.cache.read().await;
            if let Some((cached_key, telemetry)) = &*r {
                let key_matches = match (&current_key, cached_key) {
                    (Some(k1), Some(k2)) => k1 == k2,
                    (None, None) => true,
                    _ => false,
                };
                let effective_ttl = if force_refresh {
                    2
                } else if telemetry.has_provider_metadata() {
                    CACHE_TTL_SECONDS
                } else {
                    2
                };
                if key_matches
                    && telemetry.is_fresh(effective_ttl)
                    && (!force_refresh || telemetry.has_provider_metadata())
                {
                    return Ok(telemetry.clone());
                }
            }
        }

        let client = if let Some(cfg) = upstream {
            if cfg.enabled {
                build_upstream_client(cfg)?
            } else {
                self.client.clone()
            }
        } else {
            self.client.clone()
        };

        let telemetry = self.cascade_query(&client).await?;

        // Cache the newly acquired telemetry along with the active upstream key
        {
            let mut w = self.cache.write().await;
            *w = Some((current_key, telemetry.clone()));
        }

        Ok(telemetry)
    }

    async fn cascade_query(&self, client: &reqwest::Client) -> Result<NetworkTelemetry, String> {
        let mut errors = Vec::new();

        // 1. Primary: ipwho.is
        match self.query_ipwhois(client).await {
            Ok(t) => return Ok(t),
            Err(e) => errors.push(format!("ipwho.is failed: {e}")),
        }

        // 2. Fallback 1: ipinfo.io
        match self.query_ipinfo(client).await {
            Ok(t) => return Ok(t),
            Err(e) => errors.push(format!("ipinfo.io failed: {e}")),
        }

        // 3. Fallback 2: freeipapi.com (rich metadata with ASN/ISP/Timezone)
        match self.query_freeipapi(client).await {
            Ok(t) => return Ok(t),
            Err(e) => errors.push(format!("freeipapi.com failed: {e}")),
        }

        // 4. Fallback 3: 1.1.1.1/cdn-cgi/trace
        match self.query_cloudflare_trace(client).await {
            Ok(t) => return Ok(t),
            Err(e) => errors.push(format!("1.1.1.1/cdn-cgi/trace failed: {e}")),
        }

        Err(format!("All geo resolvers failed: {}", errors.join("; ")))
    }

    async fn query_ipwhois(&self, client: &reqwest::Client) -> Result<NetworkTelemetry, String> {
        let resp = client
            .get("https://ipwho.is/")
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let parsed: IpWhoIsResponse = resp.json().await.map_err(|e| e.to_string())?;

        let ip = parsed.ip.ok_or("ipwho.is missing 'ip'")?;
        let country_code = parsed
            .country_code
            .ok_or("ipwho.is missing 'country_code'")?
            .to_uppercase();

        let (asn, org, isp, domain) = match parsed.connection {
            Some(conn) => (conn.asn, conn.org, conn.isp, conn.domain),
            None => (None, None, None, None),
        };

        let timezone = parsed.timezone.and_then(|tz| tz.id);

        Ok(NetworkTelemetry {
            ip,
            country_code,
            hostname: domain,
            asn,
            org,
            isp,
            timezone,
            resolved_at: current_timestamp(),
        })
    }

    async fn query_ipinfo(&self, client: &reqwest::Client) -> Result<NetworkTelemetry, String> {
        let resp = client
            .get("https://ipinfo.io/json")
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let parsed: IpInfoResponse = resp.json().await.map_err(|e| e.to_string())?;

        let ip = parsed.ip.ok_or("ipinfo.io missing 'ip'")?;
        let country_code = parsed
            .country
            .ok_or("ipinfo.io missing 'country'")?
            .to_uppercase();

        let mut asn = None;
        let mut org_name = None;

        if let Some(org_str) = parsed.org {
            // E.g. "AS20473 The Constant Company, LLC"
            let trimmed = org_str.trim();
            if let Some(rest) = trimmed.strip_prefix("AS") {
                let mut parts = rest.splitn(2, ' ');
                if let Some(asn_str) = parts.next() {
                    asn = asn_str.parse::<u32>().ok();
                }
                if let Some(name) = parts.next() {
                    org_name = Some(name.to_string());
                }
            } else {
                org_name = Some(trimmed.to_string());
            }
        }

        Ok(NetworkTelemetry {
            ip,
            country_code,
            hostname: parsed.hostname,
            asn,
            org: org_name.clone(),
            isp: org_name,
            timezone: parsed.timezone,
            resolved_at: current_timestamp(),
        })
    }

    async fn query_freeipapi(&self, client: &reqwest::Client) -> Result<NetworkTelemetry, String> {
        let resp = client
            .get("https://freeipapi.com/api/json")
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let parsed: FreeIpApiResponse = resp.json().await.map_err(|e| e.to_string())?;

        let ip = parsed.ip_address.ok_or("freeipapi.com missing 'ipAddress'")?;
        let country_code = parsed
            .country_code
            .ok_or("freeipapi.com missing 'countryCode'")?
            .to_uppercase();

        let asn = parsed.asn.and_then(|s| {
            let trimmed = s.trim();
            let digits = trimmed.strip_prefix("AS").unwrap_or(trimmed);
            digits.parse::<u32>().ok()
        });
        let org = parsed.asn_organization;
        let timezone = parsed
            .time_zones
            .and_then(|mut v| if v.is_empty() { None } else { Some(v.remove(0)) });

        Ok(NetworkTelemetry {
            ip,
            country_code,
            hostname: None,
            asn,
            org: org.clone(),
            isp: org,
            timezone,
            resolved_at: current_timestamp(),
        })
    }

    async fn query_cloudflare_trace(&self, client: &reqwest::Client) -> Result<NetworkTelemetry, String> {
        let resp = client
            .get("https://1.1.1.1/cdn-cgi/trace")
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let text = resp.text().await.map_err(|e| e.to_string())?;
        let mut ip = None;
        let mut loc = None;

        for line in text.lines() {
            if let Some(val) = line.strip_prefix("ip=") {
                ip = Some(val.trim().to_string());
            } else if let Some(val) = line.strip_prefix("loc=") {
                loc = Some(val.trim().to_uppercase());
            }
        }

        let ip = ip.ok_or("1.1.1.1/cdn-cgi/trace missing 'ip'")?;
        let country_code = loc.ok_or("1.1.1.1/cdn-cgi/trace missing 'loc'")?;

        Ok(NetworkTelemetry {
            ip,
            country_code,
            hostname: None,
            asn: None,
            org: None,
            isp: None,
            timezone: None,
            resolved_at: current_timestamp(),
        })
    }
}

/// Gate 10: Evaluates reverse DNS / hostname / ISP / org for suspicious proxy and anonymizer markers.
pub fn evaluate_reverse_dns_ptr(telemetry: &NetworkTelemetry) -> CheckResult {
    let mut examined = Vec::new();
    if let Some(h) = &telemetry.hostname {
        examined.push(h.as_str());
    }
    if let Some(isp) = &telemetry.isp {
        examined.push(isp.as_str());
    }
    if let Some(org) = &telemetry.org {
        examined.push(org.as_str());
    }

    for text in examined {
        let lower = text.to_lowercase();
        for &marker in SUSPICIOUS_ANONYMIZER_MARKERS {
            if lower.contains(marker) {
                return CheckResult::warning(
                    "REVERSE_DNS_ANONYMIZER_MARKER",
                    format!(
                        "В данных узла ({text}) обнаружен маркер публичного анонимайзера «{marker}». Антифрод-системы ИИ блокируют такие адреса. Рекомендуется сменить сервер на чистый VPS."
                    ),
                    Some(crate::auditor::geo_matrix::GUIDE_PERSONAL_VPN.to_string()),
                );
            }
        }
    }

    let detail = telemetry
        .hostname
        .as_deref()
        .or(telemetry.isp.as_deref())
        .or(telemetry.org.as_deref())
        .unwrap_or("чистый хостинг");

    CheckResult::ok(
        "REVERSE_DNS_CLEAN",
        format!("Reverse DNS и ISP выходного IP ({detail}) чисты, маркеров публичных прокси не обнаружено."),
    )
}

/// Gate 11: Verifies whether the exit IP geolocation across independent telemetry endpoints
/// and DNS resolution is consistent (detects cross-country DNS splits or transparent DNS hijacking).
pub fn evaluate_dns_country_consistency(
    telemetry: &NetworkTelemetry,
    secondary_country: Option<&str>,
) -> CheckResult {
    let exit_country = telemetry.country_code.trim().to_uppercase();

    // 1. Check if a secondary independent telemetry source reports a mismatched country
    if let Some(sec_country) = secondary_country {
        let sec = sec_country.trim().to_uppercase();
        if !sec.is_empty() && sec != exit_country {
            return CheckResult::warning(
                "DNS_COUNTRY_SPLIT",
                format!(
                    "Обнаружен рассинхрон юрисдикций: выходной IP находится в {exit_country}, а вторичный резолвер сообщает о локации {sec}. Это может указывать на утечку DNS или прозрачный DNS-прокси."
                ),
                Some(crate::auditor::geo_matrix::GUIDE_OS_PRIVACY.to_string()),
            );
        }
    }

    CheckResult::ok(
        "DNS_COUNTRY_CONSISTENT",
        format!("Географическая юрисдикция DNS и выходного узла ({exit_country}) полностью согласованы."),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_freshness() {
        let mut tel = NetworkTelemetry {
            ip: "185.1.1.1".to_string(),
            country_code: "NL".to_string(),
            hostname: None,
            asn: Some(20473),
            org: Some("Choopa".to_string()),
            isp: Some("Choopa".to_string()),
            timezone: Some("Europe/Amsterdam".to_string()),
            resolved_at: current_timestamp(),
        };

        assert!(tel.is_fresh(30));

        // Expired telemetry
        tel.resolved_at = current_timestamp().saturating_sub(35);
        assert!(!tel.is_fresh(30));
    }

    #[test]
    fn test_ipinfo_org_parsing() {
        let org_str = "AS20473 The Constant Company, LLC";
        let rest = org_str.strip_prefix("AS").unwrap();
        let mut parts = rest.splitn(2, ' ');
        let asn: u32 = parts.next().unwrap().parse().unwrap();
        let org = parts.next().unwrap();
        assert_eq!(asn, 20473);
        assert_eq!(org, "The Constant Company, LLC");
    }

    #[test]
    fn test_build_upstream_client() {
        let socks_cfg = UpstreamProxyConfig {
            enabled: true,
            raw_key: "socks5://u:p@127.0.0.1:1080".to_string(),
            protocol: "socks5".to_string(),
            host: "127.0.0.1".to_string(),
            port: 1080,
            username: Some("u".to_string()),
            password: Some("p".to_string()),
            label: None,
        };
        assert!(build_upstream_client(&socks_cfg).is_ok());

        let http_cfg = UpstreamProxyConfig {
            enabled: true,
            raw_key: "http://127.0.0.1:8080".to_string(),
            protocol: "http".to_string(),
            host: "127.0.0.1".to_string(),
            port: 8080,
            username: None,
            password: None,
            label: None,
        };
        assert!(build_upstream_client(&http_cfg).is_ok());
    }

    #[tokio::test]
    async fn test_resolver_cache_with_upstream_key() {
        let resolver = NetworkResolver::new();

        let tel_direct = NetworkTelemetry {
            ip: "109.1.1.1".to_string(),
            country_code: "RU".to_string(),
            hostname: None,
            asn: Some(1234),
            org: Some("Rostelecom".to_string()),
            isp: Some("Rostelecom".to_string()),
            timezone: Some("Europe/Moscow".to_string()),
            resolved_at: current_timestamp(),
        };

        let tel_nl = NetworkTelemetry {
            ip: "185.10.10.1".to_string(),
            country_code: "NL".to_string(),
            hostname: Some("nl-exit-01.cleanhost.net".to_string()),
            asn: Some(20473),
            org: Some("Choopa".to_string()),
            isp: Some("Choopa".to_string()),
            timezone: Some("Europe/Amsterdam".to_string()),
            resolved_at: current_timestamp(),
        };

        // Cache direct IP
        resolver.set_cached(tel_direct.clone()).await;
        let res1 = resolver.resolve(false).await.unwrap();
        assert_eq!(res1.ip, "109.1.1.1");

        // Cache NL upstream proxy IP under specific proxy key
        let key = Some("socks5://185.10.10.1:1080".to_string());
        resolver.set_cached_for_key(key.clone(), tel_nl.clone()).await;

        let cfg = UpstreamProxyConfig {
            enabled: true,
            raw_key: "socks5://185.10.10.1:1080".to_string(),
            protocol: "socks5".to_string(),
            host: "185.10.10.1".to_string(),
            port: 1080,
            username: None,
            password: None,
            label: None,
        };

        let res2 = resolver.resolve_with_upstream(false, Some(&cfg)).await.unwrap();
        assert_eq!(res2.ip, "185.10.10.1");
        assert_eq!(res2.country_code, "NL");
    }

    #[test]
    fn test_reverse_dns_ptr_detection() {
        let bad_tel = NetworkTelemetry {
            ip: "185.220.101.5".to_string(),
            country_code: "DE".to_string(),
            hostname: Some("tor-exit-node-03.anonymizer.org".to_string()),
            asn: Some(1234),
            org: Some("Privacy Inc".to_string()),
            isp: Some("Tor Exit Relay".to_string()),
            timezone: Some("Europe/Berlin".to_string()),
            resolved_at: current_timestamp(),
        };
        let bad_res = evaluate_reverse_dns_ptr(&bad_tel);
        assert_eq!(bad_res.status, crate::auditor::geo_matrix::CheckStatus::Warning);
        assert_eq!(bad_res.code, "REVERSE_DNS_ANONYMIZER_MARKER");

        let clean_tel = NetworkTelemetry {
            ip: "95.217.1.1".to_string(),
            country_code: "FI".to_string(),
            hostname: Some("static.95-217-1-1.clients.your-server.de".to_string()),
            asn: Some(24940),
            org: Some("Hetzner Online GmbH".to_string()),
            isp: Some("Hetzner Online GmbH".to_string()),
            timezone: Some("Europe/Helsinki".to_string()),
            resolved_at: current_timestamp(),
        };
        let clean_res = evaluate_reverse_dns_ptr(&clean_tel);
        assert_eq!(clean_res.status, crate::auditor::geo_matrix::CheckStatus::Ok);
        assert_eq!(clean_res.code, "REVERSE_DNS_CLEAN");
    }

    #[test]
    fn test_dns_country_consistency() {
        let tel = NetworkTelemetry {
            ip: "185.10.10.1".to_string(),
            country_code: "NL".to_string(),
            hostname: None,
            asn: Some(20473),
            org: None,
            isp: None,
            timezone: None,
            resolved_at: current_timestamp(),
        };

        // Split country
        let split_res = evaluate_dns_country_consistency(&tel, Some("RU"));
        assert_eq!(split_res.status, crate::auditor::geo_matrix::CheckStatus::Warning);
        assert_eq!(split_res.code, "DNS_COUNTRY_SPLIT");

        // Matching country
        let clean_res = evaluate_dns_country_consistency(&tel, Some("NL"));
        assert_eq!(clean_res.status, crate::auditor::geo_matrix::CheckStatus::Ok);
        assert_eq!(clean_res.code, "DNS_COUNTRY_CONSISTENT");
    }

    #[test]
    fn test_has_provider_metadata() {
        let bare = NetworkTelemetry {
            ip: "185.10.10.1".to_string(),
            country_code: "NL".to_string(),
            hostname: None,
            asn: None,
            org: None,
            isp: None,
            timezone: None,
            resolved_at: current_timestamp(),
        };
        assert!(!bare.has_provider_metadata());

        let rich = NetworkTelemetry {
            asn: Some(20473),
            ..bare
        };
        assert!(rich.has_provider_metadata());
    }
}
