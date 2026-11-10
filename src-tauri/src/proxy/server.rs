use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tauri::Emitter;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::auditor::geo_matrix::ProviderId;
use crate::auditor::{Auditor, OverallStatus};
use crate::proxy::guard::{gate_closed, wait_open, Gate, NetworkGuard};
use crate::proxy::hold_queue::{
    HoldQueue, HoldResolution, RiskInterceptPayload, DEFAULT_HOLD_TTL_SECS,
};
use crate::proxy::pac::generate_pac_script;
use crate::state::{AppState, IpHistoryEntry};

pub const DEFAULT_PROXY_PORT: u16 = 8899;
pub const FALLBACK_PROXY_PORT: u16 = 8900;
const FIRST_REQUEST_TIMEOUT_SECS: u64 = 10;
const UPSTREAM_CONNECT_TIMEOUT_SECS: u64 = 15;
const MAX_CONCURRENT_CONNECTIONS: usize = 256;
const ALLOWED_TUNNEL_PORTS: &[u16] = &[80, 443, 8443];

pub struct ProxyServer {
    pub bound_addr: SocketAddr,
    listener: TcpListener,
    auditor: Auditor,
    state: AppState,
    pub hold_queue: Arc<HoldQueue>,
    pub guard: Arc<NetworkGuard>,
    app_handle: Option<tauri::AppHandle>,
    pub upstream_redirect: Option<SocketAddr>,
    conn_limiter: Arc<Semaphore>,
}

impl ProxyServer {
    /// Attempts to bind to 127.0.0.1:8899, with fallback to 8900 or ephemeral port.
    pub async fn bind(
        auditor: Auditor,
        state: AppState,
        hold_queue: Arc<HoldQueue>,
        app_handle: Option<tauri::AppHandle>,
    ) -> Result<Self, std::io::Error> {
        let listener = match TcpListener::bind(format!("127.0.0.1:{DEFAULT_PROXY_PORT}")).await {
            Ok(l) => l,
            Err(_) => match TcpListener::bind(format!("127.0.0.1:{FALLBACK_PROXY_PORT}")).await {
                Ok(l) => l,
                Err(_) => TcpListener::bind("127.0.0.1:0").await?,
            },
        };

        let bound_addr = listener.local_addr()?;

        Ok(Self {
            bound_addr,
            listener,
            auditor,
            state,
            hold_queue,
            guard: Arc::new(NetworkGuard::new()),
            app_handle,
            upstream_redirect: None,
            conn_limiter: Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS)),
        })
    }

    /// Binds to an ephemeral port (127.0.0.1:0) for isolated integration testing.
    pub async fn bind_ephemeral(
        auditor: Auditor,
        state: AppState,
        hold_queue: Arc<HoldQueue>,
        upstream_redirect: Option<SocketAddr>,
    ) -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let bound_addr = listener.local_addr()?;

        Ok(Self {
            bound_addr,
            listener,
            auditor,
            state,
            hold_queue,
            guard: Arc::new(NetworkGuard::new()),
            app_handle: None,
            upstream_redirect,
            conn_limiter: Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS)),
        })
    }

    /// Runs the proxy accept loop in the background.
    pub async fn run(self: Arc<Self>) {
        loop {
            match self.listener.accept().await {
                Ok((stream, client_addr)) => {
                    let permit = match Arc::clone(&self.conn_limiter).try_acquire_owned() {
                        Ok(p) => p,
                        Err(_) => {
                            eprintln!("Proxy connection limit reached, dropping {client_addr}");
                            continue;
                        }
                    };
                    let server = Arc::clone(&self);
                    tokio::spawn(async move {
                        let _permit = permit;
                        if let Err(err) = server.handle_connection(stream, client_addr).await {
                            eprintln!("Proxy connection error from {client_addr}: {err}");
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Proxy accept error: {e}");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        }
    }

    async fn handle_connection(
        &self,
        mut client: TcpStream,
        _client_addr: SocketAddr,
    ) -> Result<(), std::io::Error> {
        let mut buffer = [0u8; 4096];
        let bytes_read = match tokio::time::timeout(
            Duration::from_secs(FIRST_REQUEST_TIMEOUT_SECS),
            client.read(&mut buffer),
        )
        .await
        {
            Ok(res) => res?,
            Err(_) => return Ok(()), // idle / slow client: drop silently
        };
        if bytes_read == 0 {
            return Ok(());
        }

        let request_str = String::from_utf8_lossy(&buffer[..bytes_read]);
        let mut lines = request_str.lines();
        let request_line = match lines.next() {
            Some(l) => l,
            None => return Ok(()),
        };

        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("");
        let target = parts.next().unwrap_or("");

        // 1. Route GET /proxy.pac (supports relative "/proxy.pac" or full "http://127.0.0.1:8899/proxy.pac")
        if method == "GET" && (target.starts_with("/proxy.pac") || target.contains("/proxy.pac")) {
            return self.handle_pac_request(&mut client).await;
        }

        // 2. Route CONNECT <host>:<port>
        if method == "CONNECT" {
            return self.handle_connect_request(&mut client, target).await;
        }

        // Generic HTTP health / info response
        let resp = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 23\r\nConnection: close\r\n\r\nZastava Gateway Active\n";
        client.write_all(resp.as_bytes()).await?;
        client.flush().await?;
        Ok(())
    }

    async fn handle_pac_request(&self, client: &mut TcpStream) -> Result<(), std::io::Error> {
        let matrix = self.auditor.rules.provider_matrix.read().await;
        let custom_domains = self.state.get_custom_ai_domains().await;
        let domains = crate::proxy::pac::get_all_ai_domains_with_custom(&matrix, &custom_domains);
        let pac_script = generate_pac_script(&domains, &self.bound_addr.to_string());

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/x-ns-proxy-autoconfig\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            pac_script.len(),
            pac_script
        );

        client.write_all(response.as_bytes()).await?;
        client.flush().await?;
        Ok(())
    }

    async fn handle_connect_request(
        &self,
        client: &mut TcpStream,
        raw_target: &str,
    ) -> Result<(), std::io::Error> {
        let (host, port) = parse_host_port(raw_target);

        if let Err(reason) = validate_tunnel_target(&host, port, self.bound_addr.port()) {
            return send_forbidden(client, reason).await;
        }

        let is_canary = host == "canary-test.anthropic.zastava.local";
        let custom_domains = self.state.get_custom_ai_domains().await;
        let provider_opt = ProviderId::from_host_with_custom(&host, &custom_domains);

        let provider = match provider_opt {
            Some(p) => p,
            None => {
                // Direct pass-through for safe public non-AI domains: allows terminal users to set
                // HTTPS_PROXY=http://127.0.0.1:8899 safely without breaking git, npm, brew, etc.
                self.state.stats().record_direct_bypassed_host(&host);
                return self.tunnel_to_target(client, &host, port, false).await;
            }
        };

        // Step 0: Fast Kernel Route Check & Network guard — a sudden IP change freezes every connection until the
        // user confirms the new network (or the old IP comes back).
        let upstream_opt = self.state.get_upstream_proxy().await;
        let active_upstream = upstream_opt.as_ref().filter(|c| c.enabled);
        let upstream_key = active_upstream.map(|c| c.raw_key.as_str());
        if self.guard.check_route_fingerprint_sync(upstream_key) {
            self.auditor.resolver.invalidate_cache().await;
        }

        self.guard.ensure_fresh().await;
        match self.guard.gate() {
            Gate::Open => {}
            Gate::Pending => {
                self.state.stats().record_intercepted(&host, provider.as_str(), "BLOCKED", "??");
                if is_canary {
                    return send_canary_intercepted(client).await;
                }
                if self.guard.await_decision().await != Gate::Open {
                    return send_forbidden(
                        client,
                        "Network change blocked by Zastava Risk Auditor: connection was not confirmed.",
                    )
                    .await;
                }
            }
            Gate::Blocked => {
                self.state.stats().record_intercepted(&host, provider.as_str(), "BLOCKED", "??");
                if is_canary {
                    return send_canary_intercepted(client).await;
                }
                return send_forbidden(
                    client,
                    "Network change blocked by Zastava Risk Auditor: switch back to the trusted IP or confirm the new network.",
                )
                .await;
            }
        }

        // Step 1: Check if provider is blocked
        if self.state.is_provider_blocked(provider.as_str()).await {
            self.state.stats().record_intercepted(&host, provider.as_str(), "BLOCKED", "??");
            if is_canary {
                return send_canary_intercepted(client).await;
            }
            return send_forbidden(client, "Requests to this provider are blocked until network change.").await;
        }

        // Step 2: Check if provider has active session allowance
        if self.state.is_provider_session_allowed(provider.as_str()).await {
            self.state.stats().record_protected(&host, provider.as_str(), "ALLOWED");
            if is_canary {
                return send_canary_protected(client).await;
            }
            return self.tunnel_to_target(client, &host, port, true).await;
        }

        // Step 3: Run comprehensive risk audit
        let report = match self.auditor.evaluate_all(provider, &self.state, false).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Auditor evaluation failed: {e}");
                self.state.stats().record_intercepted(&host, provider.as_str(), "ERROR", "??");
                if is_canary {
                    return send_canary_intercepted(client).await;
                }
                // In case of auditor error, fall back to hold for safety
                return send_forbidden(client, &format!("Audit error: {e}")).await;
            }
        };

        // Step 4: If verdict is Green -> allow and record history
        if report.overall_status == OverallStatus::Green {
            if is_canary {
                self.state.stats().record_protected(&host, provider.as_str(), &report.telemetry.country_code);
                return send_canary_protected(client).await;
            }
            let entry = IpHistoryEntry::new(
                report.telemetry.ip.clone(),
                report.telemetry.country_code.clone(),
                report.telemetry.asn,
                report.telemetry.resolved_at,
            );
            let _ = self.state.record_ip_history(provider.as_str(), entry).await;
            self.state.stats().record_protected(&host, provider.as_str(), &report.telemetry.country_code);
            return self.tunnel_to_target(client, &host, port, true).await;
        }

        // If not Green and is canary test, immediately return intercepted status (<5ms response)
        if is_canary {
            self.state.stats().record_intercepted(&host, provider.as_str(), "BLOCKED", &report.telemetry.country_code);
            return send_canary_intercepted(client).await;
        }

        // Step 5: If verdict is Yellow or Red -> check preset InterceptionBehavior
        match self.state.get_interception_behavior().await {
            crate::state::InterceptionBehavior::AutoLockSilent => {
                self.state.stats().record_intercepted(&host, provider.as_str(), "BLOCKED", &report.telemetry.country_code);
                let _ = self.state.block_provider(provider.as_str()).await;
                return send_forbidden(client, "Auto-locked by Zastava silent rule").await;
            }
            crate::state::InterceptionBehavior::AutoAllowTrusted => {
                let matrix = self.auditor.rules.provider_matrix.read().await;
                let cc = report.telemetry.country_code.trim().to_uppercase();
                let is_sanctioned = matrix.global_blocked_countries.iter().any(|b| b.eq_ignore_ascii_case(&cc));
                if !is_sanctioned {
                    let entry = IpHistoryEntry::new(
                        report.telemetry.ip.clone(),
                        report.telemetry.country_code.clone(),
                        report.telemetry.asn,
                        report.telemetry.resolved_at,
                    );
                    let _ = self.state.record_ip_history(provider.as_str(), entry).await;
                    self.state.stats().record_protected(&host, provider.as_str(), &report.telemetry.country_code);
                    return self.tunnel_to_target(client, &host, port, true).await;
                } else {
                    self.state.stats().record_intercepted(&host, provider.as_str(), "BLOCKED", &report.telemetry.country_code);
                    let _ = self.state.block_provider(provider.as_str()).await;
                    return send_forbidden(client, "Auto-locked by Zastava silent rule (sanctioned country)").await;
                }
            }
            crate::state::InterceptionBehavior::AskPopup => {
                // proceed with hold_queue and show_and_position_alert_popup
            }
        }

        self.state.stats().record_intercepted(&host, provider.as_str(), "HELD", &report.telemetry.country_code);
        let hold_id = Uuid::new_v4().to_string();
        let rx = self.hold_queue.register(hold_id.clone(), provider.as_str()).await;

        let payload = RiskInterceptPayload {
            id: hold_id.clone(),
            provider,
            host: host.clone(),
            report: report.clone(),
            ttl_seconds: DEFAULT_HOLD_TTL_SECS,
            scope: "provider".to_string(),
            network_change: None,
            is_simulation: false,
        };

        // Emit event to UI and bring emergency modal to front
        if let Some(app) = &self.app_handle {
            let _ = app.emit("interceptor://risk-detected", &payload);
            crate::show_and_position_alert_popup(app);
        }

        // Wait up to 60 seconds for user action
        let resolution = tokio::time::timeout(
            Duration::from_secs(DEFAULT_HOLD_TTL_SECS),
            rx,
        ).await;

        match resolution {
            Ok(Ok(HoldResolution::Allow)) => {
                let entry = IpHistoryEntry::new(
                    report.telemetry.ip.clone(),
                    report.telemetry.country_code.clone(),
                    report.telemetry.asn,
                    report.telemetry.resolved_at,
                );
                let _ = self.state.record_ip_history(provider.as_str(), entry).await;
                self.state.stats().record_protected(&host, provider.as_str(), &report.telemetry.country_code);
                self.tunnel_to_target(client, &host, port, true).await
            }
            _ => {
                self.hold_queue.remove(&hold_id).await;
                send_forbidden(
                    client,
                    "Connection held and blocked by Zastava Risk Auditor.",
                ).await
            }
        }
    }

    async fn connect_direct(&self, host: &str, port: u16) -> Result<TcpStream, std::io::Error> {
        let addrs: Vec<SocketAddr> = match tokio::time::timeout(
            Duration::from_secs(UPSTREAM_CONNECT_TIMEOUT_SECS),
            tokio::net::lookup_host((host, port)),
        )
        .await
        {
            Ok(Ok(resolved)) => resolved.collect(),
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "DNS lookup timeout",
                ));
            }
        };

        if addrs.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No valid upstream addresses resolved.",
            ));
        }

        let host_is_raw_ip = host.parse::<IpAddr>().is_ok();
        for addr in &addrs {
            if is_forbidden_ip(&addr.ip()) {
                if !host_is_raw_ip && is_tun_fake_ip(&addr.ip()) {
                    // Allowed: DNS resolved to a TUN FakeIP pool (198.18.0.0/15) used by sing-box/Throne/Nekoray/Hiddify
                    continue;
                }
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "SSRF Protection: resolved IP belongs to a restricted local or private network.",
                ));
            }
        }

        match tokio::time::timeout(
            Duration::from_secs(UPSTREAM_CONNECT_TIMEOUT_SECS),
            TcpStream::connect(addrs.as_slice()),
        )
        .await
        {
            Ok(res) => res,
            Err(_) => Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "upstream connect timeout",
            )),
        }
    }

    async fn tunnel_to_target(
        &self,
        client: &mut TcpStream,
        host: &str,
        port: u16,
        use_upstream: bool,
    ) -> Result<(), std::io::Error> {
        let _tunnel_guard = if use_upstream || ProviderId::from_host(host).is_some() {
            Some(self.state.stats().enter_tunnel())
        } else {
            None
        };

        let connect_fut = if let Some(redirect_addr) = self.upstream_redirect {
            tokio::time::timeout(
                Duration::from_secs(UPSTREAM_CONNECT_TIMEOUT_SECS),
                TcpStream::connect(redirect_addr),
            )
            .await
            .map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "upstream redirect connect timeout",
                )
            })
            .and_then(|r| r)
        } else if use_upstream {
            if let Some(cfg) = self.state.get_upstream_proxy().await {
                if cfg.enabled {
                    crate::proxy::upstream::connect_via_upstream(
                        &cfg,
                        host,
                        port,
                        Duration::from_secs(UPSTREAM_CONNECT_TIMEOUT_SECS),
                    )
                    .await
                } else {
                    self.connect_direct(host, port).await
                }
            } else {
                self.connect_direct(host, port).await
            }
        } else {
            self.connect_direct(host, port).await
        };

        let mut target_stream = match connect_fut {
            Ok(s) => s,
            Err(e) => {
                let err_msg = format!("Failed to connect to target {host}:{port}: {e}");
                let resp = format!(
                    "HTTP/1.1 502 Bad Gateway\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    err_msg.len(),
                    err_msg
                );
                client.write_all(resp.as_bytes()).await?;
                client.flush().await?;
                return Err(e);
            }
        };

        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
        client.flush().await?;

        pump(client, &mut target_stream, &self.guard).await
    }
}

async fn send_canary_intercepted(client: &mut TcpStream) -> Result<(), std::io::Error> {
    let resp = "HTTP/1.1 503 Service Unavailable\r\nX-Zastava-Status: INTERCEPTED_BY_KILLSWITCH\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    client.write_all(resp.as_bytes()).await?;
    client.flush().await
}

async fn send_canary_protected(client: &mut TcpStream) -> Result<(), std::io::Error> {
    let resp = "HTTP/1.1 200 OK\r\nX-Zastava-Status: PROTECTED\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    client.write_all(resp.as_bytes()).await?;
    client.flush().await
}

/// Bidirectional relay that obeys the network guard: while the gate is closed no byte is
/// forwarded in either direction, and any established session aborts immediately when
/// the network route changes or the gate closes.
async fn pump(
    client: &mut TcpStream,
    target: &mut TcpStream,
    guard: &Arc<NetworkGuard>,
) -> Result<(), std::io::Error> {
    let (mut cr, mut cw) = client.split();
    let (mut tr, mut tw) = target.split();
    let c2t = forward(&mut cr, &mut tw, guard);
    let t2c = forward(&mut tr, &mut cw, guard);
    tokio::try_join!(c2t, t2c)?;
    Ok(())
}

async fn forward<R, W>(
    reader: &mut R,
    writer: &mut W,
    guard: &Arc<NetworkGuard>,
) -> Result<(), std::io::Error>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut rx = guard.subscribe();
    let mut abort_rx = guard.subscribe_tunnel_abort();
    let mut buf = vec![0u8; 16 * 1024];
    let stalled = || {
        std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "tunnel aborted: network route changed or closed",
        )
    };

    loop {
        if !wait_open(&mut rx).await {
            return Err(stalled());
        }

        let n = tokio::select! {
            res = reader.read(&mut buf) => res?,
            _ = gate_closed(&mut rx) => {
                return Err(stalled());
            }
            _ = abort_rx.changed() => {
                return Err(stalled());
            }
        };

        if n == 0 {
            let _ = writer.shutdown().await;
            return Ok(());
        }

        // Data read right before the freeze is held back until the user decides.
        if !wait_open(&mut rx).await || guard.gate() != Gate::Open {
            return Err(stalled());
        }
        writer.write_all(&buf[..n]).await?;
    }
}

pub fn parse_host_port(target: &str) -> (String, u16) {
    let trimmed = target.trim();
    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some((ipv6_host, after_bracket)) = rest.split_once(']') {
            let port = after_bracket
                .strip_prefix(':')
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(443);
            return (ipv6_host.trim().to_string(), port);
        }
    }

    // If there are multiple colons (unbracketed IPv6 literal without port)
    if trimmed.matches(':').count() > 1 {
        return (trimmed.to_string(), 443);
    }

    if let Some((h, p)) = trimmed.rsplit_once(':') {
        let port = p.parse::<u16>().unwrap_or(443);
        let clean_host = h.trim().trim_end_matches('.').to_string();
        (clean_host, port)
    } else {
        (trimmed.trim_end_matches('.').to_string(), 443)
    }
}

/// Checks whether an IP address is loopback, private (RFC1918 / RFC4193), link-local,
/// cloud metadata (`169.254.169.254`), broadcast, multicast, or unspecified.
pub fn is_forbidden_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
                || v4.is_multicast()
                // Shared address space (RFC 6598: 100.64.0.0/10)
                || (octets[0] == 100 && (octets[1] & 0b1100_0000) == 64)
                // IETF protocol assignments & benchmarking (192.0.0.0/24, 198.18.0.0/15)
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
                || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
        }
        IpAddr::V6(v6) => {
            if let Some(mapped_v4) = v6.to_ipv4_mapped() {
                return is_forbidden_ip(&IpAddr::V4(mapped_v4));
            }
            let seg0 = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // Unique Local (fc00::/7)
                || (seg0 & 0xfe00) == 0xfc00
                // Link-Local unicast (fe80::/10)
                || (seg0 & 0xffc0) == 0xfe80
        }
    }
}

/// Returns true if the IP belongs to the benchmarking / TUN FakeIP network range (`198.18.0.0/15`).
/// Used by VPN / TUN software such as sing-box, Throne, Nekoray, Hiddify, Clash in FakeIP mode.
pub fn is_tun_fake_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            octets[0] == 198 && (octets[1] == 18 || octets[1] == 19)
        }
        IpAddr::V6(v6) => {
            if let Some(mapped_v4) = v6.to_ipv4_mapped() {
                let octets = mapped_v4.octets();
                octets[0] == 198 && (octets[1] == 18 || octets[1] == 19)
            } else {
                false
            }
        }
    }
}

/// Validates target host and port against SSRF, local proxy recursion, and port scanning.
pub fn validate_tunnel_target(
    host: &str,
    port: u16,
    bound_proxy_port: u16,
) -> Result<(), &'static str> {
    let h = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if h.is_empty() {
        return Err("Invalid empty target host.");
    }

    // Safely allow the local VPN crash-test canary hostname through validation
    if h == "canary-test.anthropic.zastava.local" {
        return Ok(());
    }

    if port == bound_proxy_port
        || port == DEFAULT_PROXY_PORT
        || port == FALLBACK_PROXY_PORT
        || !ALLOWED_TUNNEL_PORTS.contains(&port)
    {
        return Err("SSRF Protection: target port is not permitted by Zastava Gateway.");
    }

    if h == "localhost"
        || h.ends_with(".localhost")
        || h.ends_with(".local")
        || h.ends_with(".internal")
        || h.ends_with(".home.arpa")
        || h == "metadata.google.internal"
    {
        return Err("SSRF Protection: local and internal hostnames are forbidden.");
    }

    if let Ok(ip) = h.parse::<IpAddr>() {
        if is_forbidden_ip(&ip) {
            return Err("SSRF Protection: direct connections to loopback or private IPs are forbidden.");
        }
    }

    Ok(())
}

async fn send_forbidden(client: &mut TcpStream, reason: &str) -> Result<(), std::io::Error> {
    let resp = format!(
        "HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        reason.len(),
        reason
    );
    client.write_all(resp.as_bytes()).await?;
    client.flush().await?;
    Ok(())
}
