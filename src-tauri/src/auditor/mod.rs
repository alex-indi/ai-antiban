pub mod cache_cleaner;
pub mod claude_exporter;
pub mod dnsbl;
pub mod geo_matrix;
pub mod history;
pub mod leaks;
pub mod network;
pub mod os_env;
pub mod profile_switcher;
pub mod rules;
pub mod telemetry;

use serde::{Deserialize, Serialize};

pub use cache_cleaner::clean_ai_app_cache;
pub use dnsbl::{check_all_dnsbl, evaluate_asn_reputation, evaluate_dnsbl_report};
pub use geo_matrix::{evaluate_provider_geo, CheckResult, CheckStatus, ProviderId};
pub use history::{evaluate_impossible_travel, evaluate_ip_pinning};
pub use leaks::{check_dns_leak, check_ipv6_leak};
pub use network::{
    build_upstream_client, evaluate_dns_country_consistency, evaluate_reverse_dns_ptr,
    NetworkResolver, NetworkTelemetry,
};
pub use os_env::{
    check_os_location_services, evaluate_locale_env_hygiene, evaluate_system_timezone,
};
pub use rules::RuleSet;
use crate::routing::{Route, RouteSelection};
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverallStatus {
    Green,
    Yellow,
    Red,
}

impl OverallStatus {
    pub fn is_allowed_by_default(&self) -> bool {
        matches!(self, OverallStatus::Green)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskReport {
    pub provider: ProviderId,
    pub overall_status: OverallStatus,
    pub trust_score: u8,
    pub telemetry: NetworkTelemetry,
    pub geo_result: CheckResult,
    pub pinning_result: CheckResult,
    pub travel_result: CheckResult,
    pub dnsbl_result: CheckResult,
    pub dnsbl_clean_count: String,
    pub asn_result: CheckResult,
    pub ipv6_result: CheckResult,
    pub timezone_result: CheckResult,
    pub checks: Vec<CheckResult>,
}

#[derive(Clone)]
pub struct Auditor {
    pub rules: RuleSet,
    pub resolver: NetworkResolver,
}

impl Default for Auditor {
    fn default() -> Self {
        Self::new().expect("Failed to initialize default Auditor")
    }
}

impl Auditor {
    pub fn new() -> Result<Self, String> {
        let rules = RuleSet::load_bundled().map_err(|e| e.to_string())?;
        let resolver = NetworkResolver::new();
        Ok(Self { rules, resolver })
    }

    /// Telemetry of the egress `provider` traffic actually uses: its route, or direct.
    /// A selected but unusable route is an error (never silently replaced by the direct egress).
    pub async fn resolve_for_provider(
        &self,
        provider: ProviderId,
        state: &AppState,
        force_refresh: bool,
    ) -> Result<NetworkTelemetry, String> {
        match state.route_selection(provider.as_str()).await {
            RouteSelection::Direct => self.resolver.resolve(force_refresh).await,
            RouteSelection::Missing(id) => Err(format!(
                "Маршрут {id} для {} не найден: трафик заблокирован",
                provider.display_name()
            )),
            RouteSelection::Route(route) => {
                if let Err(reason) = route.check_usable(network::current_timestamp()) {
                    return Err(format!(
                        "Маршрут «{}» недоступен ({}): трафик {} заблокирован",
                        route.label,
                        reason.message(),
                        provider.display_name()
                    ));
                }
                self.resolve_route(&route, state, force_refresh).await
            }
        }
    }

    /// Resolves the exit of a route and records it (incl. the declared `exit_ip` check).
    pub async fn resolve_route(
        &self,
        route: &Route,
        state: &AppState,
        force_refresh: bool,
    ) -> Result<NetworkTelemetry, String> {
        let tel = self.resolver.resolve_with_route(force_refresh, Some(route)).await?;
        state.route_health().record_exit(
            &route.id,
            &tel.ip,
            &tel.country_code,
            route.exit_ip.as_deref(),
        );
        Ok(tel)
    }

    /// Evaluates complete 12-point risk assessment for the target AI provider,
    /// on the egress of that provider's route (or the direct egress).
    pub async fn evaluate_all(
        &self,
        provider: ProviderId,
        state: &AppState,
        force_refresh_telemetry: bool,
    ) -> Result<RiskReport, String> {
        let telemetry = self
            .resolve_for_provider(provider, state, force_refresh_telemetry)
            .await?;

        let matrix = self.rules.provider_matrix.read().await;
        let dangerous_asns = self.rules.dangerous_asns.read().await;

        let state_data = state.get_data().await;

        let is_pinned = match &state_data.pinned_ip {
            Some(pinned) => pinned.trim() == telemetry.ip.trim(),
            None => false,
        };

        // 1. Geo Matrix Check
        let geo_result = evaluate_provider_geo(&matrix, provider, &telemetry.country_code);

        // 2. IP Pinning Check
        let pinning_result = evaluate_ip_pinning(
            state_data.pinned_ip.as_deref(),
            state_data.strict_pinning_enabled,
            &telemetry.ip,
        );

        // 3. Impossible Travel Check
        let history = state_data
            .ip_history
            .get(provider.as_str())
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let travel_result = evaluate_impossible_travel(
            history,
            &telemetry.country_code,
            telemetry.resolved_at,
            provider,
        );

        // 4. DNSBL Spam & Blacklist Check
        let dnsbl_report = check_all_dnsbl(&telemetry.ip, &dangerous_asns).await;
        let dnsbl_clean_count = dnsbl_report.summary_label.clone();
        let dnsbl_result = evaluate_dnsbl_report(&dnsbl_report);

        // 5. ASN Reputation Check
        let asn_result = evaluate_asn_reputation(
            telemetry.asn,
            telemetry.isp.as_deref(),
            telemetry.org.as_deref(),
            &dangerous_asns,
            is_pinned,
        );

        // 6. IPv6 Leak Check
        let ipv6_result =
            check_ipv6_leak(&telemetry.country_code, &matrix.global_blocked_countries).await;

        // 7. DNS Resolver Check
        let dns_result = check_dns_leak(
            &telemetry.country_code,
            &dangerous_asns.russian_isp_keywords,
        );

        // 8. Timezone Check (Gate 8)
        let timezone_result = evaluate_system_timezone(
            telemetry.timezone.as_deref(),
            state_data.synced_ai_timezone.as_deref(),
        );

        // 9. OS Location Services Check (Gate 9)
        let location_result = check_os_location_services();

        // 10. Reverse DNS PTR & Anonymizer Check (Gate 10)
        let reverse_dns_result = evaluate_reverse_dns_ptr(&telemetry);

        // 11. DNS Country Consistency Check (Gate 11)
        let dns_country_result = evaluate_dns_country_consistency(&telemetry, None);

        // 12. Locale & Environment Hygiene Check (Gate 12)
        let locale_result = evaluate_locale_env_hygiene(state_data.synced_ai_timezone.as_deref());

        let checks = vec![
            geo_result.clone(),
            pinning_result.clone(),
            travel_result.clone(),
            dnsbl_result.clone(),
            asn_result.clone(),
            ipv6_result.clone(),
            dns_result,
            timezone_result.clone(),
            location_result,
            reverse_dns_result,
            dns_country_result,
            locale_result,
        ];

        // Compute Overall Status and Trust Score
        let mut score: i32 = 100;
        let mut has_critical = false;
        let mut has_warning = false;

        for check in &checks {
            match check.status {
                CheckStatus::Critical => {
                    has_critical = true;
                    score -= 40;
                }
                CheckStatus::Warning => {
                    has_warning = true;
                    score -= 15;
                }
                CheckStatus::Ok => {}
            }
        }

        if is_pinned && !has_critical {
            score = (score + 10).min(100);
        }

        let overall_status = if has_critical {
            score = score.clamp(0, 35);
            OverallStatus::Red
        } else if has_warning {
            score = score.clamp(36, 85);
            OverallStatus::Yellow
        } else {
            score = score.clamp(86, 100);
            OverallStatus::Green
        };

        Ok(RiskReport {
            provider,
            overall_status,
            trust_score: score as u8,
            telemetry,
            geo_result,
            pinning_result,
            travel_result,
            dnsbl_result,
            dnsbl_clean_count,
            asn_result,
            ipv6_result,
            timezone_result,
            checks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overall_status_allowed() {
        assert!(OverallStatus::Green.is_allowed_by_default());
        assert!(!OverallStatus::Yellow.is_allowed_by_default());
        assert!(!OverallStatus::Red.is_allowed_by_default());
    }

    #[tokio::test]
    async fn test_auditor_initialization() {
        let auditor = Auditor::new().expect("Auditor initialization should succeed");
        let matrix = auditor.rules.provider_matrix.read().await;
        assert_eq!(matrix.version, "1.0.0");
    }
}
