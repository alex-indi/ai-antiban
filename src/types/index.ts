export type ProviderId = "anthropic" | "openai" | "gemini";

export type OverallStatus = "green" | "yellow" | "red";

export type CheckStatus = "ok" | "warning" | "critical" | "green" | "yellow" | "red";

export type InterceptionBehavior = "ask_popup" | "auto_lock_silent" | "auto_allow_trusted";
export type PopupPosition = "top_right" | "top_center";

export interface CheckResult {
  status: CheckStatus;
  code: string;
  message: string;
  guide_ref?: string | null;
}

export interface NetworkTelemetry {
  ip: string;
  country_code: string;
  asn?: number;
  org?: string;
  isp?: string;
  timezone?: string;
  resolved_at: number;
}

export interface RiskReport {
  provider: ProviderId;
  overall_status: OverallStatus;
  trust_score: number;
  telemetry: NetworkTelemetry;
  geo_result: CheckResult;
  pinning_result: CheckResult;
  travel_result: CheckResult;
  dnsbl_result: CheckResult;
  dnsbl_clean_count: string;
  asn_result: CheckResult;
  ipv6_result: CheckResult;
  timezone_result: CheckResult;
  checks: CheckResult[];
}

export interface CleanResult {
  provider: ProviderId;
  cleaned_targets: string[];
  files_removed: number;
  bytes_freed: number;
  success: boolean;
  message: string;
}

export interface BinAuditResult {
  bin: string;
  country_code?: string;
  card_type?: string;
  bank_name?: string;
  is_prepaid: boolean;
  status: CheckStatus;
  advisory: string;
  guide_ref?: string;
}

export interface IpHistoryEntry {
  ip: string;
  country_code: string;
  asn?: number;
  timestamp: number;
}

export interface UpstreamProxyConfig {
  enabled: boolean;
  raw_key: string;
  protocol: string;
  host: string;
  port: number;
  username?: string | null;
  password?: string | null;
  label?: string | null;
}

export interface AppStateData {
  pinned_ip: string | null;
  strict_pinning_enabled: boolean;
  ip_history: Record<string, IpHistoryEntry[]>;
  session_allowed_providers: string[];
  blocked_providers: string[];
  upstream_proxy?: UpstreamProxyConfig | null;
  warmup_started_at?: number | null;
  synced_ai_timezone?: string | null;
  auto_update_enabled?: boolean;
  interception_behavior?: InterceptionBehavior;
  popup_position?: PopupPosition;
  custom_ai_domains?: string[];
}

export type AppStateSnapshot = AppStateData;

export type InterceptionAction =
  | "AllowSession"
  | "BlockUntilNetworkChange"
  | "OpenGuideAndBlock"
  | "allowSession"
  | "blockUntilNetworkChange"
  | "openGuideAndBlock";

export interface NetworkChange {
  old_ip?: string | null;
  old_country?: string | null;
  new_ip: string;
  new_country: string;
}

export interface RiskInterceptPayload {
  id: string;
  provider: ProviderId;
  host: string;
  report: RiskReport;
  ttl_seconds: number;
  scope?: "provider" | "network" | string;
  network_change?: NetworkChange | null;
  is_simulation?: boolean;
}

export interface UpdateCheckResult {
  current_version: string;
  latest_version: string;
  update_available: boolean;
  release_name: string;
  release_notes: string;
  release_url: string;
  published_at?: string | null;
  download_url?: string | null;
  asset_name?: string | null;
  asset_size?: number | null;
}

export interface UpdateProgressPayload {
  downloaded_bytes: number;
  total_bytes: number;
  percent: number;
  phase: string;
}

export interface UpdateStatusPayload {
  stage: string;
  message: string;
}

export interface ClaudeProjectBrief {
  slug: string;
  decoded_path: string;
  sessions_count: number;
  has_memory: boolean;
  total_bytes: number;
  last_modified?: number | null;
}

export interface ClaudeLocalDataSummary {
  exists: boolean;
  claude_dir_path: string;
  projects_count: number;
  sessions_count: number;
  plans_count: number;
  memory_files_count: number;
  total_size_bytes: number;
  projects: ClaudeProjectBrief[];
  has_claude_md: boolean;
  has_history: boolean;
  has_settings: boolean;
  has_desktop_config: boolean;
  has_claude_json: boolean;
}

export interface ClaudeExportResult {
  export_dir_path: string;
  zip_archive_path?: string | null;
  index_summary_path: string;
  restore_prompt_path: string;
  restore_prompt_text: string;
  projects_exported: number;
  sessions_exported: number;
  total_bytes: number;
}

export interface InstallUpdateResult {
  success: boolean;
  message: string;
  will_restart: boolean;
}

export interface CliProtectionReport {
  enabled: boolean;
  proxy_url: string;
  configured_targets: string[];
  shell_command: string;
}

export interface RecentGatewayEvent {
  timestamp: number;
  host: string;
  provider: string;
  action: string; // "PROTECTED" | "HELD" | "BLOCKED" | "DIRECT"
  kind?: string;
  country: string;
}

export interface GatewayStatsSnapshot {
  protected_ai_requests: number;
  intercepted_leaks: number;
  direct_bypassed_requests: number;
  active_ai_tunnels: number;
  uptime_seconds: number;
  last_protected_host: string | null;
  recent_events: RecentGatewayEvent[];
}

export interface ClaudeProfile {
  id: string;
  name: string;
  bound_ip?: string | null;
  bound_country?: string | null;
  bound_upstream_key?: string | null;
  isolated_device_id: string;
  created_at: number;
  last_used_at: number;
  is_active: boolean;
  has_saved_session: boolean;
}

export interface ClaudeProfileListResponse {
  profiles: ClaudeProfile[];
  active_profile_id: string;
  claude_desktop_installed: boolean;
  storage_dir: string;
}

export interface ProfileSwitchResult {
  previous_profile_id: string;
  active_profile: ClaudeProfile;
  cleaned_cache_items: number;
  restored_files: number;
  pinned_ip_updated?: string | null;
  bound_upstream_key_updated?: string | null;
  message: string;
}

export interface CrashTestStepResult {
  timestamp: number;
  target_host: string;
  intercepted_by_killswitch: boolean;
  http_status: number;
  latency_ms: number;
  status_code: string;
  message: string;
}
