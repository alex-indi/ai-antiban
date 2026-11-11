//! Network guard: detects sudden egress-IP changes (VPN switch, reconnect, roaming),
//! freezes ALL AI traffic (new and already established tunnels) and asks the user
//! to confirm the new network before anything is forwarded again.
//!
//! Providers whose traffic goes through a route (see `crate::routing`) have a fixed exit IP, so
//! local network / VPN changes do not apply to them: the automatic `Pending` / `Blocked` gate
//! states, popups and provider blocks only concern unrouted providers. The manual kill-switch
//! ("СТОП СЕТИ") is an explicit, separate flag that stops everything, routed traffic included.

use std::net::IpAddr;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use tauri::{Emitter, Manager};
use tokio::sync::{watch, Mutex};
use uuid::Uuid;

use crate::auditor::geo_matrix::ProviderId;
use crate::auditor::{Auditor, NetworkTelemetry, OverallStatus, RiskReport};
use crate::proxy::hold_queue::{
    HoldQueue, HoldResolution, NetworkChange, RiskInterceptPayload, DEFAULT_HOLD_TTL_SECS,
};
use crate::state::{AppState, IpHistoryEntry, KNOWN_PROVIDERS};

/// Fast kernel routing table check interval (sub-250ms reaction time on VPN disconnect).
pub const ROUTE_CHECK_INTERVAL_MS: u64 = 250;
/// Remote IP probe interval when route fingerprint has not changed.
pub const PROBE_INTERVAL_MS: u64 = 1000;
/// Minimum gap between two remote HTTP probes.
pub const PROBE_MIN_GAP_MS: u64 = 1000;
/// How long an established tunnel may stall before it is dropped (looks like a dead network).
pub const MAX_STALL_SECS: u64 = 90;
/// Pseudo provider id used for network-wide holds.
pub const ALL_PROVIDERS: &str = "all";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// Traffic flows normally.
    Open,
    /// Network changed, waiting for the user's decision.
    Pending,
    /// User refused the new network: traffic stays frozen until the IP is restored or confirmed.
    Blocked,
}

#[derive(Clone)]
pub struct GuardCtx {
    pub auditor: Auditor,
    pub state: AppState,
    pub hold_queue: Arc<HoldQueue>,
    pub app: tauri::AppHandle,
}

#[derive(Clone)]
struct Baseline {
    ip: String,
    country: Option<String>,
    /// True only if the baseline was either user-confirmed or verified non-sanctioned.
    /// Prevents a VPN drop back to an initial sanctioned IP (e.g. RU at boot) from auto-opening the gate.
    trusted: bool,
}

struct Pending {
    id: String,
}

#[derive(Default)]
struct Inner {
    baseline: Option<Baseline>,
    observed_ip: Option<String>,
    /// IP of the network the user has already been asked about (avoids popup loops).
    handled_ip: Option<String>,
    pending: Option<Pending>,
    last_probe: Option<Instant>,
    last_route_fp: Option<String>,
    /// Unrouted providers seen on the previous tick (a provider becoming unrouted re-arms checks).
    last_unrouted: Option<Vec<&'static str>>,
}

pub struct NetworkGuard {
    gate_tx: watch::Sender<Gate>,
    tunnel_abort_tx: watch::Sender<u64>,
    /// Manual kill-switch. Unlike the automatic gate it also stops routed providers.
    kill_tx: watch::Sender<bool>,
    /// Serialises every gate write with the kill flag: while the manual kill-switch is engaged the
    /// gate can never be anything but `Blocked`, no matter which task (tick, accept, restore,
    /// popup resolution) races with it.
    gate_lock: std::sync::Mutex<()>,
    inner: Mutex<Inner>,
    ctx: OnceLock<GuardCtx>,
    client: reqwest::Client,
}

/// Computes the local default-route network interface fingerprint in < 0.05ms
/// using a local UDP routing table lookup without transmitting any network packet.
pub fn compute_local_route_fingerprint(upstream_key: Option<&str>) -> String {
    let local_ip = match std::net::UdpSocket::bind("0.0.0.0:0") {
        Ok(socket) => match socket.connect("1.1.1.1:80") {
            Ok(_) => match socket.local_addr() {
                Ok(addr) => addr.ip().to_string(),
                Err(_) => "0.0.0.0".to_string(),
            },
            Err(_) => "0.0.0.0".to_string(),
        },
        Err(_) => "0.0.0.0".to_string(),
    };
    format!("{local_ip}|upstream:{}", upstream_key.unwrap_or(""))
}

impl Default for NetworkGuard {
    fn default() -> Self {
        Self::new()
    }
}

fn severity(report: &RiskReport) -> u8 {
    match report.overall_status {
        OverallStatus::Red => 2,
        OverallStatus::Yellow => 1,
        OverallStatus::Green => 0,
    }
}

/// Audits anthropic, openai and gemini (each on its own egress). Order is fixed: anthropic first.
async fn audit_all(ctx: &GuardCtx) -> Result<Vec<RiskReport>, String> {
    // First call forces a fresh geo lookup (with 1 automatic retry for transient VPN interface handshakes),
    // and the others reuse the freshly filled cache.
    let anthropic = match ctx
        .auditor
        .evaluate_all(ProviderId::Anthropic, &ctx.state, true)
        .await
    {
        Ok(r) => r,
        Err(_) => {
            tokio::time::sleep(Duration::from_millis(400)).await;
            ctx.auditor
                .evaluate_all(ProviderId::Anthropic, &ctx.state, true)
                .await?
        }
    };
    let (openai, gemini) = tokio::join!(
        ctx.auditor
            .evaluate_all(ProviderId::Openai, &ctx.state, false),
        ctx.auditor
            .evaluate_all(ProviderId::Gemini, &ctx.state, false)
    );
    Ok(vec![anthropic, openai?, gemini?])
}

/// Waits until the gate is open. Returns `false` immediately when blocked or stall limit is exceeded.
pub async fn wait_open(rx: &mut watch::Receiver<Gate>) -> bool {
    let fut = async {
        loop {
            match *rx.borrow_and_update() {
                Gate::Open => return true,
                Gate::Blocked => return false,
                Gate::Pending => {}
            }
            if rx.changed().await.is_err() {
                return false;
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(MAX_STALL_SECS), fut)
        .await
        .unwrap_or(false)
}

/// Completes as soon as the gate leaves the `Open` state.
pub async fn gate_closed(rx: &mut watch::Receiver<Gate>) {
    loop {
        if *rx.borrow_and_update() != Gate::Open {
            return;
        }
        if rx.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

impl NetworkGuard {
    pub fn new() -> Self {
        let (gate_tx, _rx) = watch::channel(Gate::Open);
        let (tunnel_abort_tx, _abort_rx) = watch::channel(0u64);
        let (kill_tx, _kill_rx) = watch::channel(false);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(2000))
            // Never reuse keep-alive sockets: they may still point to the previous route.
            .pool_max_idle_per_host(0)
            .user_agent("Zastava/1.0")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            gate_tx,
            tunnel_abort_tx,
            kill_tx,
            gate_lock: std::sync::Mutex::new(()),
            inner: Mutex::new(Inner::default()),
            ctx: OnceLock::new(),
            client,
        }
    }

    pub fn gate(&self) -> Gate {
        *self.gate_tx.borrow()
    }

    pub fn subscribe(&self) -> watch::Receiver<Gate> {
        self.gate_tx.subscribe()
    }

    pub fn subscribe_tunnel_abort(&self) -> watch::Receiver<u64> {
        self.tunnel_abort_tx.subscribe()
    }

    /// True while the manual kill-switch is engaged (automatic freezes do not set it).
    pub fn is_manual_killswitch(&self) -> bool {
        *self.kill_tx.borrow()
    }

    pub fn subscribe_killswitch(&self) -> watch::Receiver<bool> {
        self.kill_tx.subscribe()
    }

    /// Aborts unrouted tunnels (routed tunnels only obey the manual kill-switch).
    pub fn abort_active_tunnels(&self) {
        let _ = self.tunnel_abort_tx.send_modify(|val| *val = val.wrapping_add(1));
    }

    pub fn check_route_fingerprint_sync(&self, upstream_key: Option<&str>) -> bool {
        let fp = compute_local_route_fingerprint(upstream_key);
        let mut g = match self.inner.try_lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        if g.baseline.is_none() {
            g.last_route_fp = Some(fp);
            return false;
        }
        if let Some(ref old_fp) = g.last_route_fp {
            if fp != *old_fp {
                g.last_route_fp = Some(fp);
                self.set_gate(Gate::Pending);
                self.abort_active_tunnels();
                return true;
            }
        } else {
            g.last_route_fp = Some(fp);
        }
        false
    }

    /// Sets the gate. While the manual kill-switch is engaged every request is coerced to
    /// `Blocked`: `Open` (or `Pending`) cannot be set behind the kill-switch's back.
    fn set_gate(&self, gate: Gate) {
        let _g = self.gate_lock.lock().unwrap_or_else(|p| p.into_inner());
        let effective = if *self.kill_tx.borrow() { Gate::Blocked } else { gate };
        self.gate_tx.send_replace(effective);
    }

    /// Engages the kill flag and blocks the gate as one step with respect to `set_gate`.
    fn engage_kill(&self) {
        let _g = self.gate_lock.lock().unwrap_or_else(|p| p.into_inner());
        self.kill_tx.send_replace(true);
        self.gate_tx.send_replace(Gate::Blocked);
    }

    /// Lifts the kill flag and opens the gate as one step with respect to `set_gate`.
    fn release_kill(&self) {
        let _g = self.gate_lock.lock().unwrap_or_else(|p| p.into_inner());
        self.kill_tx.send_replace(false);
        self.gate_tx.send_replace(Gate::Open);
    }

    /// Re-engages a manual kill-switch saved by a previous run (a restart must not lift it).
    /// Returns whether it is engaged.
    pub async fn restore_manual_killswitch(&self, state: &AppState) -> bool {
        if state.is_manual_killswitch_saved().await {
            self.engage_kill();
            true
        } else {
            false
        }
    }

    pub fn is_gate_blocked(&self) -> bool {
        self.gate() == Gate::Blocked
    }

    pub async fn set_manual_killswitch(&self, freeze: bool, state: &AppState) -> bool {
        if freeze {
            self.engage_kill();
            self.abort_active_tunnels();
            if let Err(e) = state.set_manual_killswitch_saved(true).await {
                eprintln!("Warning: could not persist the kill-switch: {e}");
            }
            let mut inner = self.inner.lock().await;
            if let Some(p) = inner.pending.take() {
                if let Some(ctx) = self.ctx.get() {
                    ctx.hold_queue.remove(&p.id).await;
                }
            }
            drop(inner);
            for p in KNOWN_PROVIDERS {
                let _ = state.block_provider(p).await;
            }
            if let Some(ctx) = self.ctx.get() {
                self.hide_popup(ctx);
                crate::update_tray_status(&ctx.app, "red", Some("⛔ СТОП СЕТИ (Ручная блокировка ИИ)"));
            }
            true
        } else {
            self.release_kill();
            if let Err(e) = state.set_manual_killswitch_saved(false).await {
                eprintln!("Warning: could not persist the kill-switch: {e}");
            }
            let mut inner = self.inner.lock().await;
            if let Some(p) = inner.pending.take() {
                if let Some(ctx) = self.ctx.get() {
                    ctx.hold_queue.remove(&p.id).await;
                }
            }
            drop(inner);
            let _ = state.clear_blocked_providers().await;
            if let Some(ctx) = self.ctx.get() {
                self.hide_popup(ctx);
                crate::update_tray_status(&ctx.app, "green", None);
            }
            false
        }
    }

    pub async fn freeze_for_test(&self) {
        self.set_gate(Gate::Blocked);
        self.abort_active_tunnels();
        if let Some(ctx) = self.ctx.get() {
            crate::show_and_position_alert_popup(&ctx.app);
        }
    }

    pub async fn verify_before_connect(self: &Arc<Self>) -> bool {
        self.ensure_fresh().await;
        self.gate() == Gate::Open
    }

    /// Used by new CONNECT requests: waits for the user's decision while the gate is pending.
    pub async fn await_decision(&self) -> Gate {
        let mut rx = self.gate_tx.subscribe();
        let fut = async {
            loop {
                let g = *rx.borrow_and_update();
                if g != Gate::Pending {
                    return g;
                }
                if rx.changed().await.is_err() {
                    return Gate::Blocked;
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(DEFAULT_HOLD_TTL_SECS + 15), fut)
            .await
            .unwrap_or(Gate::Blocked)
    }

    /// Starts the background monitor (call once from the Tauri setup hook).
    pub fn start(self: &Arc<Self>, ctx: GuardCtx) {
        if self.ctx.set(ctx).is_err() {
            return;
        }
        let guard = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let mut n: u64 = 0;
            loop {
                if let Some(ctx) = guard.ctx.get() {
                    guard.check_route_and_tick(ctx).await;
                    if n % 20 == 0 {
                        let _ = tokio::task::spawn_blocking(
                            crate::sysproxy::refresh_if_services_changed,
                        )
                        .await;
                    }
                }
                n += 1;
                tokio::time::sleep(Duration::from_millis(ROUTE_CHECK_INTERVAL_MS)).await;
            }
        });
    }

    /// Cheap pre-flight check used by every new CONNECT (no-op when the monitor is not running).
    pub async fn ensure_fresh(self: &Arc<Self>) {
        if let Some(ctx) = self.ctx.get() {
            self.check_route_and_tick(ctx).await;
        }
    }

    /// Fetches the current egress IP (Cloudflare trace, falls back to ipify).
    async fn probe_ip(&self) -> Option<String> {
        if let Ok(resp) = self.client.get("https://1.1.1.1/cdn-cgi/trace").send().await {
            if let Ok(text) = resp.text().await {
                for line in text.lines() {
                    if let Some(val) = line.strip_prefix("ip=") {
                        let val = val.trim();
                        if val.parse::<IpAddr>().is_ok() {
                            return Some(val.to_string());
                        }
                    }
                }
            }
        }
        if let Ok(resp) = self.client.get("https://api.ipify.org").send().await {
            if let Ok(text) = resp.text().await {
                let val = text.trim();
                if val.parse::<IpAddr>().is_ok() {
                    return Some(val.to_string());
                }
            }
        }
        None
    }

    pub async fn check_route_and_tick(self: &Arc<Self>, ctx: &GuardCtx) {
        // Routes do not take part in the fingerprint: routed traffic has a fixed exit and is not
        // governed by the gate, and the probe below measures the direct egress only.
        let current_fp = compute_local_route_fingerprint(None);
        let unrouted = ctx.state.unrouted_providers().await;

        let mut inner = self.inner.lock().await;

        // A provider that just lost its route must be re-checked against the current network.
        let gained_unrouted = inner
            .last_unrouted
            .as_ref()
            .map(|prev| unrouted.iter().any(|p| !prev.contains(p)))
            .unwrap_or(false);
        if gained_unrouted {
            inner.handled_ip = None;
        }
        inner.last_unrouted = Some(unrouted.clone());

        // Manual kill-switch: nothing to decide until the user lifts it.
        if self.is_manual_killswitch() {
            inner.last_route_fp = Some(current_fp);
            return;
        }

        // Every provider has a fixed exit: local network changes are irrelevant.
        if unrouted.is_empty() {
            inner.last_route_fp = Some(current_fp);
            if self.gate() != Gate::Open {
                self.set_gate(Gate::Open);
            }
            return;
        }

        let route_changed = if inner.baseline.is_some() {
            if let Some(ref old_fp) = inner.last_route_fp {
                *old_fp != current_fp
            } else {
                false
            }
        } else {
            false
        };

        if route_changed {
            inner.last_route_fp = Some(current_fp.clone());
            // 1. Immediately freeze gate (so zero packets leak during the probe!)
            self.set_gate(Gate::Pending);

            // 2. Probe external IP
            let probed = self.probe_ip().await;
            inner.last_probe = Some(Instant::now());

            // 3. Check if probed IP equals baseline IP
            let (baseline_ip, baseline_trusted) = inner
                .baseline
                .as_ref()
                .map(|b| (b.ip.clone(), b.trusted))
                .unzip();
            let baseline_trusted = baseline_trusted.unwrap_or(false);

            if let Some(ref new_ip) = probed {
                inner.observed_ip = Some(new_ip.clone());

                if baseline_ip.as_deref() == Some(new_ip.as_str()) {
                    // The external IP did NOT change at all — local utun refresh on macOS (e.g. Amnezia)!
                    if baseline_trusted {
                        self.set_gate(Gate::Open);
                        return;
                    }
                    if inner.handled_ip.as_deref() == Some(new_ip.as_str()) {
                        return;
                    }
                }
            }

            // 4. Only if new_ip actually changed (or if probed is None / different from baseline):
            self.abort_active_tunnels();
            ctx.auditor.resolver.invalidate_cache().await;
            let old_country = inner
                .baseline
                .as_ref()
                .and_then(|b| b.country.clone())
                .unwrap_or_else(|| "??".to_string());
            ctx.state
                .stats()
                .record_intercepted("all", "NetworkGuard", "HELD", &old_country);

            if let Some(new_ip) = probed {
                self.handle_change_locked(ctx, &mut inner, new_ip).await;
            }
            return;
        }

        inner.last_route_fp = Some(current_fp);

        if let Some(t) = inner.last_probe {
            if t.elapsed() < Duration::from_millis(PROBE_MIN_GAP_MS) {
                return;
            }
        }
        let probed = self.probe_ip().await;
        inner.last_probe = Some(Instant::now());
        let Some(ip) = probed else {
            return; // offline: nothing to compare
        };
        inner.observed_ip = Some(ip.clone());

        let Some((baseline_ip, baseline_trusted)) = inner
            .baseline
            .as_ref()
            .map(|b| (b.ip.clone(), b.trusted))
        else {
            let country = ctx
                .auditor
                .resolver
                .resolve(false)
                .await
                .ok()
                .map(|t| t.country_code);
            let is_sanctioned = if let Some(ref cc) = country {
                let matrix = ctx.auditor.rules.provider_matrix.read().await;
                matrix
                    .global_blocked_countries
                    .iter()
                    .any(|b| b.eq_ignore_ascii_case(cc))
            } else {
                false
            };
            inner.baseline = Some(Baseline {
                ip,
                country,
                trusted: !is_sanctioned,
            });
            return;
        };

        if ip == baseline_ip {
            if baseline_trusted {
                if self.gate() != Gate::Open {
                    self.restore_locked(ctx, &mut inner).await;
                } else {
                    inner.handled_ip = None;
                }
                return;
            }
            // If the initial baseline was untrusted (e.g. app started on a sanctioned IP before VPN)
            // and the user switched away and dropped back to it, treat it as an unsafe change!
        }

        if inner.handled_ip.as_deref() == Some(ip.as_str()) {
            return; // user already decided about this network
        }

        self.handle_change_locked(ctx, &mut inner, ip).await;
    }

    pub async fn tick(self: &Arc<Self>, ctx: &GuardCtx) {
        self.check_route_and_tick(ctx).await;
    }

    async fn handle_change_locked(
        self: &Arc<Self>,
        ctx: &GuardCtx,
        inner: &mut Inner,
        new_ip: String,
    ) {
        let unrouted = ctx.state.unrouted_providers().await;
        if unrouted.is_empty() {
            // All providers are routed: nothing flows over the changed network, no popup.
            inner.handled_ip = Some(new_ip);
            self.set_gate(Gate::Open);
            return;
        }

        // 1. Freeze everything immediately (unrouted traffic only; routed tunnels ignore the gate).
        self.set_gate(Gate::Pending);
        self.abort_active_tunnels();
        if let Some(p) = inner.pending.take() {
            ctx.hold_queue.remove(&p.id).await;
        }

        // 2. Previous allowances / blocks belonged to the old network (the manual kill-switch's
        //    blocks do not).
        let _ = ctx.state.clear_session_allowed_providers().await;
        if !self.is_manual_killswitch() {
            let _ = ctx.state.clear_blocked_providers().await;
        }
        ctx.auditor.resolver.invalidate_cache().await;

        // 3. Full re-audit of the new network.
        let reports = match audit_all(ctx).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Network guard: audit of the new network failed: {e}");
                return; // gate stays closed, next tick retries
            }
        };

        let old = inner.baseline.clone();
        if old.as_ref().map(|b| b.ip.as_str()) == Some(new_ip.as_str())
            && inner.handled_ip.as_deref() == Some(new_ip.as_str())
        {
            return;
        }
        inner.handled_ip = Some(new_ip.clone());

        // Only the providers that actually use the changed network decide.
        let relevant: Vec<&RiskReport> = reports
            .iter()
            .filter(|r| unrouted.contains(&r.provider.as_str()))
            .collect();
        let mut worst = relevant.first().copied().unwrap_or(&reports[0]);
        for r in relevant.iter().skip(1) {
            if severity(r) > severity(worst) {
                worst = r;
            }
        }
        let worst = worst.clone();
        let tel = worst.telemetry.clone();

        let st = ctx.state.get_data().await;
        let is_pinned = st
            .pinned_ip
            .as_deref()
            .map(|p| p.trim() == new_ip.trim() || p.trim() == tel.ip.trim())
            .unwrap_or(false);
        let all_green = relevant
            .iter()
            .all(|r| r.overall_status == OverallStatus::Green);

        self.update_tray(ctx, &reports[0]).await;

        // Returning to the trusted pinned VPS with a clean audit needs no confirmation.
        if is_pinned && all_green {
            self.accept_locked(ctx, inner, &new_ip, &tel).await;
            return;
        }

        // Respect preset InterceptionBehavior
        match ctx.state.get_interception_behavior().await {
            crate::state::InterceptionBehavior::AutoLockSilent => {
                for p in &unrouted {
                    let _ = ctx.state.block_provider(p).await;
                }
                self.set_gate(Gate::Blocked);
                self.hide_popup(ctx);
                self.update_tray(ctx, &worst).await;
                let _ = ctx.app.emit("network://changed", &worst);
                return;
            }
            crate::state::InterceptionBehavior::AutoAllowTrusted => {
                let is_blocked = crate::is_sanctioned_or_blocked_country(&tel.country_code)
                    || tel.country_code.eq_ignore_ascii_case("HK")
                    || tel.country_code.eq_ignore_ascii_case("CN");
                if !is_blocked {
                    self.accept_locked(ctx, inner, &new_ip, &tel).await;
                    let _ = ctx.app.emit("network://changed", &worst);
                    return;
                } else {
                    for p in &unrouted {
                        let _ = ctx.state.block_provider(p).await;
                    }
                    self.set_gate(Gate::Blocked);
                    self.hide_popup(ctx);
                    self.update_tray(ctx, &worst).await;
                    let _ = ctx.app.emit("network://changed", &worst);
                    return;
                }
            }
            crate::state::InterceptionBehavior::AskPopup => {
                // proceed with hold_queue and show_and_position_alert_popup
            }
        }

        // 4. Ask the user (popup in configured corner without stealing focus), traffic stays frozen.
        let id = Uuid::new_v4().to_string();
        let rx = ctx
            .hold_queue
            .register(id.clone(), ALL_PROVIDERS)
            .await;
        inner.pending = Some(Pending { id: id.clone() });

        let payload = RiskInterceptPayload {
            id: id.clone(),
            provider: worst.provider,
            host: "Смена сети · все ИИ-сервисы".to_string(),
            report: worst.clone(),
            ttl_seconds: DEFAULT_HOLD_TTL_SECS,
            scope: "network".to_string(),
            network_change: Some(NetworkChange {
                old_ip: old.as_ref().map(|b| b.ip.clone()),
                old_country: old.as_ref().and_then(|b| b.country.clone()),
                new_ip: tel.ip.clone(),
                new_country: tel.country_code.clone(),
            }),
            is_simulation: false,
        };

        let _ = ctx.app.emit("interceptor://risk-detected", &payload);
        crate::show_and_position_alert_popup(&ctx.app);
        let _ = ctx.app.emit("network://changed", &worst);

        // 5. Wait for the decision in the background.
        let guard = Arc::clone(self);
        let ctx2 = ctx.clone();
        tauri::async_runtime::spawn(async move {
            let res = tokio::time::timeout(Duration::from_secs(DEFAULT_HOLD_TTL_SECS), rx).await;
            let allow = matches!(res, Ok(Ok(HoldResolution::Allow)));

            let mut inner = guard.inner.lock().await;
            let still_current = inner
                .pending
                .as_ref()
                .map(|p| p.id == id)
                .unwrap_or(false);
            if !still_current {
                return; // superseded by a newer change or already accepted/restored
            }
            inner.pending = None;
            ctx2.hold_queue.remove(&id).await;

            if allow {
                let ip = inner.observed_ip.clone().unwrap_or_else(|| tel.ip.clone());
                guard.accept_locked(&ctx2, &mut inner, &ip, &tel).await;
            } else {
                for p in ctx2.state.unrouted_providers().await {
                    let _ = ctx2.state.block_provider(p).await;
                }
                guard.set_gate(Gate::Blocked);
                guard.hide_popup(&ctx2);
                guard.refresh_tray(&ctx2).await;
            }
        });
    }

    /// Confirms the current network as the new trusted baseline and resumes traffic.
    async fn accept_locked(
        &self,
        ctx: &GuardCtx,
        inner: &mut Inner,
        ip: &str,
        tel: &NetworkTelemetry,
    ) {
        inner.baseline = Some(Baseline {
            ip: ip.to_string(),
            country: Some(tel.country_code.clone()),
            trusted: true,
        });
        inner.handled_ip = None;
        if let Some(p) = inner.pending.take() {
            ctx.hold_queue.remove(&p.id).await;
        }

        // Routed providers keep their own (route) history.
        let now = crate::auditor::network::current_timestamp();
        for p in ctx.state.unrouted_providers().await {
            let entry = IpHistoryEntry::new(tel.ip.clone(), tel.country_code.clone(), tel.asn, now);
            let _ = ctx.state.record_ip_history(p, entry).await;
        }

        self.set_gate(Gate::Open);
        self.hide_popup(ctx);
        let _ = ctx.app.emit("network://changed", ());
        self.refresh_tray(ctx).await;
    }

    /// The previous (trusted) IP is back: resume traffic.
    async fn restore_locked(&self, ctx: &GuardCtx, inner: &mut Inner) {
        if let Some(p) = inner.pending.take() {
            ctx.hold_queue.remove(&p.id).await;
        }
        inner.handled_ip = None;
        ctx.auditor.resolver.invalidate_cache().await;
        if !self.is_manual_killswitch() {
            let _ = ctx.state.clear_blocked_providers().await;
        }
        self.set_gate(Gate::Open);
        self.hide_popup(ctx);
        let _ = ctx.app.emit("network://changed", ());
        self.refresh_tray(ctx).await;
    }

    /// Manual override from the main window ("Разблокировать и разрешить доступ").
    pub async fn accept_current(self: &Arc<Self>) {
        let Some(ctx) = self.ctx.get() else {
            return;
        };
        // Only the explicit toggle lifts the manual kill-switch.
        if self.is_manual_killswitch() {
            return;
        }
        let mut inner = self.inner.lock().await;
        if self.gate() == Gate::Open {
            return;
        }
        if let Ok(tel) = ctx.auditor.resolver.resolve(false).await {
            let ip = inner.observed_ip.clone().unwrap_or_else(|| tel.ip.clone());
            self.accept_locked(ctx, &mut inner, &ip, &tel).await;
        } else {
            self.set_gate(Gate::Open);
        }
    }

    fn hide_popup(&self, ctx: &GuardCtx) {
        if let Some(popup) = ctx.app.get_webview_window("alert-popup") {
            let _ = popup.hide();
        }
        let _ = ctx.app.emit("interceptor://resolved", "");
    }

    async fn update_tray(&self, ctx: &GuardCtx, report: &RiskReport) {
        let st = ctx.state.get_data().await;
        let is_pinned = st
            .pinned_ip
            .as_deref()
            .map(|p| p.trim() == report.telemetry.ip.trim())
            .unwrap_or(false);
        let held = self.gate() != Gate::Open || !st.blocked_providers.is_empty();
        crate::update_tray_from_report(&ctx.app, report, is_pinned, held);
    }

    async fn refresh_tray(&self, ctx: &GuardCtx) {
        if let Ok(report) = ctx
            .auditor
            .evaluate_all(ProviderId::Anthropic, &ctx.state, false)
            .await
        {
            self.update_tray(ctx, &report).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gate_default_open_and_wait() {
        let guard = NetworkGuard::new();
        assert_eq!(guard.gate(), Gate::Open);

        let mut rx = guard.subscribe();
        assert!(wait_open(&mut rx).await);
    }

    #[tokio::test]
    async fn test_gate_pending_blocks_until_open() {
        let guard = Arc::new(NetworkGuard::new());
        guard.set_gate(Gate::Pending);

        let g2 = Arc::clone(&guard);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            g2.set_gate(Gate::Open);
        });

        let decision = guard.await_decision().await;
        assert_eq!(decision, Gate::Open);
    }

    #[tokio::test]
    async fn test_await_decision_returns_blocked() {
        let guard = Arc::new(NetworkGuard::new());
        guard.set_gate(Gate::Pending);

        let g2 = Arc::clone(&guard);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            g2.set_gate(Gate::Blocked);
        });

        assert_eq!(guard.await_decision().await, Gate::Blocked);
    }

    #[tokio::test]
    async fn test_wait_open_returns_false_immediately_on_blocked() {
        let guard = NetworkGuard::new();
        guard.set_gate(Gate::Blocked);
        let mut rx = guard.subscribe();
        assert!(!wait_open(&mut rx).await);
    }

    #[test]
    fn test_compute_local_route_fingerprint() {
        let fp1 = compute_local_route_fingerprint(None);
        assert!(!fp1.is_empty());
        assert!(fp1.contains("|upstream:"));

        let fp2 = compute_local_route_fingerprint(Some("socks5://user:pass@127.0.0.1:1080"));
        assert!(fp2.contains("|upstream:socks5://user:pass@127.0.0.1:1080"));
        assert_ne!(fp1, fp2);
    }

    #[tokio::test]
    async fn test_tunnel_abort_notification() {
        let guard = NetworkGuard::new();
        let mut abort_rx = guard.subscribe_tunnel_abort();
        assert_eq!(*abort_rx.borrow_and_update(), 0);

        guard.abort_active_tunnels();
        assert!(abort_rx.changed().await.is_ok());
        assert_eq!(*abort_rx.borrow_and_update(), 1);
    }

    #[tokio::test]
    async fn test_route_fingerprint_sync_detects_change() {
        let guard = NetworkGuard::new();
        {
            let mut inner = guard.inner.lock().await;
            inner.baseline = Some(Baseline {
                ip: "1.2.3.4".to_string(),
                country: Some("NL".to_string()),
                trusted: true,
            });
            inner.last_route_fp = Some("192.168.1.100|upstream:old".to_string());
        }

        // Calling with different upstream key detects route change immediately
        let changed = guard.check_route_fingerprint_sync(Some("socks5://new"));
        assert!(changed);
        assert_eq!(guard.gate(), Gate::Pending);
    }

    #[tokio::test]
    async fn test_manual_killswitch_toggle() {
        let guard = NetworkGuard::new();
        assert!(!guard.is_gate_blocked());

        let state = AppState::new_in_memory();

        // Freeze
        let frozen = guard.set_manual_killswitch(true, &state).await;
        assert!(frozen);
        assert!(guard.is_gate_blocked());
        assert_eq!(guard.gate(), Gate::Blocked);
        for p in KNOWN_PROVIDERS {
            assert!(state.is_provider_blocked(p).await);
        }

        // Unfreeze
        let unfrozen = guard.set_manual_killswitch(false, &state).await;
        assert!(!unfrozen);
        assert!(!guard.is_gate_blocked());
        assert_eq!(guard.gate(), Gate::Open);
        for p in KNOWN_PROVIDERS {
            assert!(!state.is_provider_blocked(p).await);
        }
    }

    #[tokio::test]
    async fn test_manual_killswitch_flag_is_distinct_from_automatic_freeze() {
        let guard = Arc::new(NetworkGuard::new());
        let state = AppState::new_in_memory();
        let mut kill_rx = guard.subscribe_killswitch();

        // Automatic freeze (VPN drop simulation): gate closes, kill flag stays off.
        guard.freeze_for_test().await;
        assert_eq!(guard.gate(), Gate::Blocked);
        assert!(!guard.is_manual_killswitch());
        guard.set_gate(Gate::Pending);
        assert!(!guard.is_manual_killswitch());

        // Manual kill-switch: explicit flag, observable by routed tunnels.
        guard.set_manual_killswitch(true, &state).await;
        assert!(guard.is_manual_killswitch());
        assert!(kill_rx.changed().await.is_ok());
        assert!(*kill_rx.borrow_and_update());

        // accept_current (popup "allow") must not lift the manual kill-switch.
        guard.accept_current().await;
        assert!(guard.is_manual_killswitch());
        assert_eq!(guard.gate(), Gate::Blocked);

        guard.set_manual_killswitch(false, &state).await;
        assert!(!guard.is_manual_killswitch());
        assert_eq!(guard.gate(), Gate::Open);
    }
}
