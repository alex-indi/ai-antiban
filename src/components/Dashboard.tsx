import React, { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "@tauri-apps/api/core";
import {
  Shield,
  RotateCw,
  Pin,
  Trash2,
  Radio,
  Copy,
  Check,
  CheckCircle2,
  ChevronRight,
  Send,
  Terminal,
  Download,
  ExternalLink,
  X,
  Globe,
  Key,
  ShieldCheck,
  ShieldAlert,
  AlertTriangle,
  Zap,
  Clock,
  FolderArchive,
  Settings,
  Activity,
  OctagonAlert,
  Play,
} from "lucide-react";
import {
  AppStateData,
  ClaudeExportResult,
  ClaudeLocalDataSummary,
  GatewayStatsSnapshot,
  ProviderId,
  RiskInterceptPayload,
  RiskReport,
  UpdateCheckResult,
  UpdateProgressPayload,
  UpdateStatusPayload,
} from "../types";
import {
  allowProviderSession,
  auditConnection,
  checkCliProtectionStatus,
  checkForUpdates,
  cleanAppCache,
  disableCliProtection,
  enableCliProtection,
  exportClaudeLocalData,
  fetchAppState,
  getGatewayStats,
  getMockScenario,
  getNetworkKillswitchStatus,
  getSysproxyStatus,
  installUpdate,
  MockScenario,
  openOsDatetimeSettings,
  pinCurrentIp,
  resolveInterception,
  scanClaudeLocalData,
  setMockScenario,
  simulateInterception,
  syncAiAgentsTimezone,
  toggleAutoUpdate,
  toggleNetworkKillswitch,
  toggleStrictPinning,
  toggleSysproxy,
  unblockProvider,
  unpinIp,
} from "../utils/api";
import { AlertPopup } from "./AlertPopup";
import { TrustRing } from "./TrustRing";
import { ProviderMatrix } from "./ProviderMatrix";
import { DeepAuditTab } from "./DeepAuditTab";
import { GuideViewer } from "./GuideViewer";
import { BinCheckerTab } from "./BinCheckerTab";
import { CleanIpTab } from "./CleanIpTab";
import { ToastContainer, ToastMessage } from "./Toast";
import { SettingsModal } from "./SettingsModal";
import { ClaudeProfilesCard } from "./ClaudeProfilesCard";
import { VpnCrashTestModal } from "./VpnCrashTestModal";
import { openExternal, openTelegramSupport } from "../utils/opener";

type ActiveTab = "pulse" | "clean-ip" | "claude_vault" | "bin" | "audit" | "guide";

export interface ParsedChangelogSection {
  title?: string;
  items: string[];
}

export function parseReleaseNotes(notes: string): ParsedChangelogSection[] {
  if (!notes || !notes.trim()) {
    return [{ items: ["Улучшения стабильности и безопасности сетевого шлюза."] }];
  }

  const lines = notes.split("\n");
  const sections: ParsedChangelogSection[] = [];
  let currentTitle: string | undefined = undefined;
  let currentItems: string[] = [];

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line) continue;

    if (
      line.includes(".dmg") ||
      line.includes(".exe") ||
      line.includes(".tar.gz") ||
      line.includes(".zip") ||
      line.startsWith("---") ||
      line.startsWith("***") ||
      /^#{1,6}\s*(Скачать|Загрузка|Ссылки|Assets|Downloads)/i.test(line)
    ) {
      continue;
    }

    if (line.startsWith("#")) {
      if (currentItems.length > 0) {
        sections.push({ title: currentTitle, items: currentItems });
        currentItems = [];
      }
      currentTitle = line.replace(/^#+\s*/, "").replace(/[*_]/g, "").trim();
      continue;
    }

    if (/^[-*•]\s+/.test(line)) {
      const cleanItem = line.replace(/^[-*•]\s+/, "").replace(/[*_]/g, "").trim();
      if (cleanItem) {
        currentItems.push(cleanItem);
      }
    } else if (/^\d+\.\s+/.test(line)) {
      const cleanItem = line.replace(/^\d+\.\s+/, "").replace(/[*_]/g, "").trim();
      if (cleanItem) {
        currentItems.push(cleanItem);
      }
    } else if (currentItems.length > 0 && !line.startsWith(">") && !line.includes("http")) {
      const cleanItem = line.replace(/[*_]/g, "").trim();
      if (cleanItem.length > 3 && !cleanItem.toLowerCase().startsWith("для новых")) {
        currentItems.push(cleanItem);
      }
    }
  }

  if (currentItems.length > 0) {
    sections.push({ title: currentTitle, items: currentItems });
  }

  if (sections.length === 0 || sections.every((s) => s.items.length === 0)) {
    return [{ items: [notes.slice(0, 300).trim()] }];
  }

  return sections;
}

interface DashboardProps {
  onSimulateInBrowser?: () => void;
}

export const Dashboard: React.FC<DashboardProps> = ({ onSimulateInBrowser }) => {
  const [activeTab, setActiveTab] = useState<ActiveTab>(() => {
    const t = new URLSearchParams(window.location.search).get("tab") as ActiveTab | null;
    if (t === "audit" || t === "guide" || t === "bin" || t === "pulse" || t === "clean-ip") return t;
    return "pulse";
  });
  const [state, setState] = useState<AppStateData | null>(null);
  const [reports, setReports] = useState<Partial<Record<ProviderId, RiskReport>>>({});
  const [refreshing, setRefreshing] = useState<boolean>(false);
  const [scanLatencyMs, setScanLatencyMs] = useState<number>(38);
  const [sysproxyActive, setSysproxyActive] = useState<boolean>(true);
  const [copiedIp, setCopiedIp] = useState<boolean>(false);
  const [copiedCli, setCopiedCli] = useState<boolean>(false);
  const [syncingTz, setSyncingTz] = useState<boolean>(false);
  const [tzSyncedNotice, setTzSyncedNotice] = useState<boolean>(false);
  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  const [interceptPayload, setInterceptPayload] = useState<RiskInterceptPayload | null>(null);
  const [initialGuideTarget, setInitialGuideTarget] = useState<string>("01-personal-vpn-15min");
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResult | null>(null);
  const [checkingUpdate, setCheckingUpdate] = useState<boolean>(false);
  const [installingUpdate, setInstallingUpdate] = useState<boolean>(false);
  const [updateProgress, setUpdateProgress] = useState<UpdateProgressPayload | null>(null);
  const [showUpdateModal, setShowUpdateModal] = useState<boolean>(false);
  const [gatewayStats, setGatewayStats] = useState<GatewayStatsSnapshot | null>(null);
  const [pulseProtected, setPulseProtected] = useState<boolean>(false);
  const [feedExpanded, setFeedExpanded] = useState<boolean>(false);

  // v1.2.0: Settings and Crash Test Modals
  const [showSettingsModal, setShowSettingsModal] = useState<boolean>(false);
  const [showCrashTestModal, setShowCrashTestModal] = useState<boolean>(false);
  const [autoUpdateEnabled, setAutoUpdateEnabled] = useState<boolean>(true);
  const [killswitchFrozen, setKillswitchFrozen] = useState<boolean>(false);
  const [hideOnboarding, setHideOnboarding] = useState<boolean>(() => {
    try {
      return localStorage.getItem("zastava_hide_onboarding_v13") === "true";
    } catch {
      return false;
    }
  });

  const [updaterStatus, setUpdaterStatus] = useState<UpdateStatusPayload | null>(null);
  const [showChangelog, setShowChangelog] = useState<boolean>(false);
  const [showFullReleaseText, setShowFullReleaseText] = useState<boolean>(false);

  const [claudeSummary, setClaudeSummary] = useState<ClaudeLocalDataSummary | null>(null);
  const [loadingClaudeSummary, setLoadingClaudeSummary] = useState<boolean>(false);
  const [claudeExporting, setClaudeExporting] = useState<boolean>(false);
  const [claudeExportResult, setClaudeExportResult] = useState<ClaudeExportResult | null>(null);
  const [copiedPrompt, setCopiedPrompt] = useState<boolean>(false);
  const [cleaningClaude, setCleaningClaude] = useState<boolean>(false);
  const [cleaningGpt, setCleaningGpt] = useState<boolean>(false);
  const [scenario, setScenario] = useState<MockScenario>(() => {
    const s = new URLSearchParams(window.location.search).get("scenario") as MockScenario | null;
    if (s === "clean" || s === "turkey" || s === "ru") {
      setMockScenario(s);
      return s;
    }
    return getMockScenario();
  });

  const showToast = (
    title: string,
    description?: string,
    type: "success" | "warning" | "error" | "info" = "info"
  ) => {
    const id = Date.now().toString() + Math.random().toString(36).substring(2, 6);
    setToasts((prev) => [...prev, { id, title, description, type }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 3500);
  };

  const dismissToast = (id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  };

  const runFullAudit = async (force: boolean = false, silentError: boolean = false) => {
    const startTime = performance.now();
    setRefreshing(true);

    try {
      try {
        const st = await fetchAppState();
        setState(st);
        if (st.auto_update_enabled !== undefined) {
          setAutoUpdateEnabled(st.auto_update_enabled);
        }
      } catch (stateErr) {
        console.warn("Could not fetch app state:", stateErr);
      }

      try {
        const proxyActive = await getSysproxyStatus();
        setSysproxyActive(proxyActive);
      } catch (proxyErr) {
        console.warn("Could not fetch sysproxy status:", proxyErr);
      }

      let claudeReport: RiskReport;
      try {
        claudeReport = await auditConnection("anthropic", force);
      } catch {
        await new Promise((r) => setTimeout(r, 600));
        claudeReport = await auditConnection("anthropic", true);
      }

      // Immediately update primary report so IP, ISP, and 12-point DeepAuditTab render right away
      setReports((prev) => ({
        ...prev,
        anthropic: claudeReport,
      }));

      const [gptSettled, geminiSettled] = await Promise.allSettled([
        auditConnection("openai", false),
        auditConnection("gemini", false),
      ]);

      setReports((prev) => ({
        ...prev,
        anthropic: claudeReport,
        ...(gptSettled.status === "fulfilled" ? { openai: gptSettled.value } : {}),
        ...(geminiSettled.status === "fulfilled" ? { gemini: geminiSettled.value } : {}),
      }));

      const elapsed = Math.round(performance.now() - startTime);
      setScanLatencyMs(Math.max(18, elapsed));
      await refreshGatewayStats();
    } catch (err: any) {
      console.error("Failed to run audit:", err);
      if (!silentError) {
        showToast("Ошибка аудита", err?.message || String(err), "error");
      }
    } finally {
      setRefreshing(false);
      refreshGatewayStats();
    }
  };

  const refreshGatewayStats = async () => {
    try {
      const stats = await getGatewayStats();
      setGatewayStats((prev) => {
        if (prev && stats.protected_ai_requests > prev.protected_ai_requests) {
          setPulseProtected(true);
          setTimeout(() => setPulseProtected(false), 900);
        }
        return stats;
      });
    } catch (err) {
      console.warn("Could not fetch gateway stats:", err);
    }
  };

  useEffect(() => {
    if (activeTab !== "pulse") return;

    refreshGatewayStats();
    const interval = setInterval(() => {
      refreshGatewayStats();
    }, 2000);

    return () => clearInterval(interval);
  }, [activeTab]);

  useEffect(() => {
    if (activeTab === "claude_vault" && !claudeSummary) {
      loadClaudeSummary();
    }
    if (activeTab === "audit" && !reports.anthropic && !refreshing) {
      runFullAudit(true, true);
    }
  }, [activeTab, claudeSummary, reports.anthropic, refreshing]);

  const handleCheckUpdates = async (silent: boolean = false) => {
    setCheckingUpdate(true);
    try {
      const info = await checkForUpdates();
      setUpdateInfo(info);
      if (info.update_available) {
        setShowUpdateModal(true);
        showToast(
          `Доступно обновление v${info.latest_version}`,
          info.release_name || "Нажмите «Обновить» для автоматической установки",
          "info"
        );
      } else if (!silent) {
        setShowUpdateModal(true);
        showToast(
          "Обновлений нет",
          `У вас установлена актуальная версия v${info.current_version}`,
          "success"
        );
      }
    } catch (err: any) {
      if (!silent) {
        showToast("Ошибка проверки обновлений", err?.message || String(err), "error");
      }
    } finally {
      setCheckingUpdate(false);
    }
  };

  const handleInstallUpdate = async () => {
    if (!updateInfo?.download_url || !updateInfo?.asset_name) {
      if (updateInfo?.release_url) {
        await openExternal(updateInfo.release_url);
      }
      return;
    }

    setInstallingUpdate(true);
    setUpdateProgress({
      downloaded_bytes: 0,
      total_bytes: updateInfo.asset_size || 0,
      percent: 0,
      phase: "starting",
    });

    try {
      const res = await installUpdate(updateInfo.download_url, updateInfo.asset_name);
      showToast(
        res.will_restart ? "Обновление установлено" : "Обновление готово",
        res.message,
        "success"
      );
      if (!res.will_restart) {
        setShowUpdateModal(false);
      }
    } catch (err: any) {
      showToast("Ошибка установки обновления", err?.message || String(err), "error");
    } finally {
      setInstallingUpdate(false);
      setUpdateProgress(null);
    }
  };

  const handleToggleAutoUpdate = async (enabled: boolean) => {
    try {
      await toggleAutoUpdate(enabled);
      setAutoUpdateEnabled(enabled);
      showToast(
        enabled ? "Автообновление включено" : "Автообновление выключено",
        enabled
          ? "Застава будет автоматически проверять стабильные релизы"
          : "Автоматическая проверка обновлений отключена",
        "info"
      );
    } catch (err: any) {
      showToast("Ошибка переключения", err?.message || String(err), "error");
    }
  };

  useEffect(() => {
    runFullAudit(true, true);
    handleCheckUpdates(true);
    getNetworkKillswitchStatus()
      .then((frozen) => setKillswitchFrozen(frozen))
      .catch(() => {});
    checkCliProtectionStatus()
      .then((active) => {
        if (active) setCopiedCli(true);
      })
      .catch(() => {});

    const unlisteners: Array<() => void> = [];
    if (isTauri()) {
      listen<string>("interceptor://open-guide", (event) => {
        setInitialGuideTarget(event.payload || "01-personal-vpn-15min");
        setActiveTab("guide");
      }).then((un) => unlisteners.push(un));

      listen("tray://rescan", () => {
        runFullAudit(true);
      }).then((un) => unlisteners.push(un));

      listen("tray://check-updates", () => {
        handleCheckUpdates(false);
      }).then((un) => unlisteners.push(un));

      // v1.2.0: Tray Event Listeners
      listen("tray://sync-tz", () => {
        handleSyncTz();
      }).then((un) => unlisteners.push(un));

      listen("tray://backup-claude", () => {
        handleExportClaude();
      }).then((un) => unlisteners.push(un));

      listen("tray://open-crash-test", () => {
        setShowCrashTestModal(true);
      }).then((un) => unlisteners.push(un));

      // v1.3.0: Tray Event Listeners (Kill-Switch, Profiles & State)
      listen<boolean>("tray://killswitch-toggled", (event) => {
        const frozen = event.payload;
        setKillswitchFrozen(frozen);
        showToast(
          frozen ? "СТОП СЕТИ включен" : "Защита в штатном режиме",
          frozen
            ? "СТОП СЕТИ включен — все запросы к ИИ заморожены"
            : "Защита в штатном режиме — доступ к ИИ открыт",
          frozen ? "warning" : "success"
        );
      }).then((un) => unlisteners.push(un));

      listen("tray://state-changed", async () => {
        try {
          const st = await fetchAppState();
          setState(st);
        } catch {}
      }).then((un) => unlisteners.push(un));

      listen<any>("tray://profile-switched", async (event) => {
        try {
          const st = await fetchAppState();
          setState(st);
        } catch {}
        showToast(
          "Профиль переключен",
          event.payload?.name || "Активный профиль Claude обновлен",
          "info"
        );
        runFullAudit(false);
      }).then((un) => unlisteners.push(un));

      listen("tray://open-profiles", () => {
        setActiveTab("claude_vault");
      }).then((un) => unlisteners.push(un));

      listen<UpdateProgressPayload>("updater://progress", (event) => {
        setUpdateProgress(event.payload);
      }).then((un) => unlisteners.push(un));

      listen<UpdateCheckResult>("updater://available", (event) => {
        setUpdateInfo(event.payload);
      }).then((un) => unlisteners.push(un));

      listen<UpdateStatusPayload>("updater://status", (event) => {
        setUpdaterStatus(event.payload);
        if (event.payload.stage === "installing" || event.payload.stage === "restarting") {
          setInstallingUpdate(true);
        }
      }).then((un) => unlisteners.push(un));

      listen("tray://pin-current-ip", async () => {
        const rep = await auditConnection("anthropic", false);
        if (rep.telemetry.ip) {
          await pinCurrentIp(rep.telemetry.ip);
          runFullAudit(false);
        }
      }).then((un) => unlisteners.push(un));

      listen("network://changed", () => {
        runFullAudit(true);
      }).then((un) => unlisteners.push(un));

      listen<RiskInterceptPayload>("interceptor://risk-detected", (event) => {
        setInterceptPayload(event.payload);
      }).then((un) => unlisteners.push(un));

      listen("interceptor://resolved", () => {
        setInterceptPayload(null);
        runFullAudit(false);
      }).then((un) => unlisteners.push(un));
    }

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, []);

  const activeReport =
    reports.anthropic || reports.openai || reports.gemini || null;
  const currentIp = activeReport?.telemetry.ip || "--";
  const currentCountry = activeReport?.telemetry.country_code || "--";
  const trustScore = activeReport?.trust_score ?? 100;

  // Startup self-healing poll: if initial audit was still waiting on cold-start network/PAC setup
  // or returned without IP/provider metadata, automatically re-probe without requiring manual click.
  useEffect(() => {
    const missingIp = !activeReport || !activeReport.telemetry?.ip || activeReport.telemetry.ip === "--";
    const missingProvider =
      activeReport &&
      !activeReport.telemetry?.asn &&
      !activeReport.telemetry?.isp &&
      !activeReport.telemetry?.org;

    if (!missingIp && !missingProvider) {
      return;
    }

    let attempts = 0;
    const maxAttempts = 6;
    const timer = setInterval(() => {
      attempts += 1;
      if (attempts > maxAttempts) {
        clearInterval(timer);
        return;
      }
      runFullAudit(true, true);
    }, 1500);

    return () => clearInterval(timer);
  }, [
    activeReport?.telemetry?.ip,
    activeReport?.telemetry?.asn,
    activeReport?.telemetry?.isp,
    activeReport?.telemetry?.org,
  ]);

  const handleSwitchScenario = (next: MockScenario) => {
    setMockScenario(next);
    setScenario(next);
    runFullAudit(true);
  };

  const handleCopyIp = () => {
    if (currentIp && currentIp !== "--") {
      navigator.clipboard.writeText(currentIp);
      setCopiedIp(true);
      showToast("IP скопирован", currentIp, "success");
      setTimeout(() => setCopiedIp(false), 2000);
    }
  };

  const handleToggleCliProtection = async () => {
    try {
      if (copiedCli) {
        await disableCliProtection();
        setCopiedCli(false);
        showToast(
          "Защита CLI отключена",
          "Маршрутизация прокси удалена из настроек редакторов и терминала.",
          "info"
        );
      } else {
        const report = await enableCliProtection();
        try {
          await navigator.clipboard.writeText(report.shell_command);
        } catch {
          // optional clipboard
        }
        setCopiedCli(true);
        const targets =
          report.configured_targets.length > 0
            ? report.configured_targets.join(", ")
            : "Терминал и IDE";
        showToast(
          "Защита VS Code, Cursor и CLI включена",
          `Автоматически настроено: ${targets}. При выходе из Заставы прокси отключается сам.`,
          "success"
        );
      }
    } catch (err: any) {
      showToast("Ошибка настройки CLI", err?.message || String(err), "error");
    }
  };

  const handleSyncTz = async (tzOverride?: string) => {
    const tz = tzOverride || activeReport?.telemetry?.timezone || "Europe/Amsterdam";
    setSyncingTz(true);
    try {
      const res = await syncAiAgentsTimezone(tz);
      setTzSyncedNotice(true);
      showToast("Часовой пояс ИИ синхронизирован", res, "success");
      await runFullAudit(true);
    } catch (err: any) {
      showToast("Ошибка синхронизации TZ", err?.message || String(err), "error");
    } finally {
      setSyncingTz(false);
    }
  };

  const handleOpenOsClockAndGuide = async () => {
    try {
      await openOsDatetimeSettings();
    } catch {}
    setInitialGuideTarget("03-os-privacy-setup");
    setActiveTab("guide");
  };

  const handleToggleSysproxy = async () => {
    try {
      const next = !sysproxyActive;
      const res = await toggleSysproxy(next);
      setSysproxyActive(res);
      showToast(
        res ? "PAC-перехват включен" : "PAC-перехват отключен",
        res
          ? "Селективная маршрутизация ИИ-доменов :8899 активна"
          : "Восстановлена прямая системная маршрутизация",
        res ? "success" : "info"
      );
    } catch (err: any) {
      showToast("Ошибка прокси", err?.message || String(err), "error");
    }
  };

  const handleToggleKillswitch = async () => {
    try {
      const next = !killswitchFrozen;
      const isFrozen = await toggleNetworkKillswitch(next);
      setKillswitchFrozen(isFrozen);
      if (isFrozen) {
        showToast(
          "СТОП СЕТИ включен",
          "СТОП СЕТИ включен — все запросы к ИИ заморожены",
          "warning"
        );
      } else {
        showToast(
          "Защита в штатном режиме",
          "Защита в штатном режиме — доступ к ИИ открыт",
          "success"
        );
      }
    } catch (err: any) {
      showToast("Ошибка Kill-Switch", err?.message || String(err), "error");
    }
  };

  const handleAllowInterception = async () => {
    if (!interceptPayload) return;
    try {
      await resolveInterception(
        interceptPayload.id,
        "AllowSession",
        interceptPayload.provider,
        interceptPayload.scope
      );
      showToast("Пауза снята", "Трафик к ИИ разрешен для этой сессии", "success");
    } catch (err: any) {
      showToast("Ошибка", err?.message || String(err), "error");
    } finally {
      setInterceptPayload(null);
      runFullAudit(false);
    }
  };

  const handlePinIp = async (ip: string) => {
    try {
      await pinCurrentIp(ip);
      const st = await fetchAppState();
      setState(st);
      showToast("VPS закреплен", `IP ${ip} сохранен как эталонный узел`, "success");
      runFullAudit(false);
    } catch (err: any) {
      showToast("Ошибка закрепления", err?.message || String(err), "error");
    }
  };

  const handleUnpinIp = async () => {
    try {
      await unpinIp();
      const st = await fetchAppState();
      setState(st);
      showToast("VPS откреплен", "Привязка к эталонному IP сброшена", "info");
      runFullAudit(false);
    } catch (err: any) {
      showToast("Ошибка", err?.message || String(err), "error");
    }
  };

  const handleToggleStrict = async () => {
    if (!state) return;
    try {
      const next = !state.strict_pinning_enabled;
      await toggleStrictPinning(next);
      const st = await fetchAppState();
      setState(st);
      showToast(
        next ? "Строгий режим включен" : "Строгий режим отключен",
        next
          ? "Любая смена IP будет перехвачена до отправки запроса"
          : "Включен режим уведомлений",
        next ? "warning" : "info"
      );
      runFullAudit(false);
    } catch (err: any) {
      showToast("Ошибка", err?.message || String(err), "error");
    }
  };

  const handleAllowProvider = async (provider: ProviderId) => {
    try {
      await allowProviderSession(provider);
      showToast(
        "Доступ разрешен",
        `Блокировка для ${provider === "anthropic" ? "Claude" : provider === "openai" ? "ChatGPT" : "Gemini"} снята на текущую сессию`,
        "success"
      );
      await runFullAudit(false);
    } catch (err: any) {
      showToast("Ошибка", err?.message || String(err), "error");
    }
  };

  const handleUnblockProvider = async (provider: ProviderId) => {
    try {
      await unblockProvider(provider);
      await runFullAudit(false);
    } catch (err: any) {
      showToast("Ошибка", err?.message || String(err), "error");
    }
  };

  const loadClaudeSummary = async () => {
    setLoadingClaudeSummary(true);
    try {
      const summary = await scanClaudeLocalData();
      setClaudeSummary(summary);
    } catch (err: any) {
      console.error("Failed to scan Claude data:", err);
    } finally {
      setLoadingClaudeSummary(false);
    }
  };

  const handleExportClaude = async () => {
    setClaudeExporting(true);
    try {
      const res = await exportClaudeLocalData();
      setClaudeExportResult(res);
      showToast(
        "Архив Claude готов",
        `Экспорт сохранен на Рабочем столе: ${res.zip_archive_path || res.export_dir_path}`,
        "success"
      );
    } catch (err: any) {
      showToast("Ошибка экспорта", err?.message || String(err), "error");
    } finally {
      setClaudeExporting(false);
    }
  };

  const handleCopyRestorePrompt = async () => {
    try {
      let promptText = claudeExportResult?.restore_prompt_text;
      if (!promptText) {
        setClaudeExporting(true);
        const res = await exportClaudeLocalData();
        setClaudeExportResult(res);
        promptText = res.restore_prompt_text;
        setClaudeExporting(false);
      }
      if (promptText) {
        await navigator.clipboard.writeText(promptText);
        setCopiedPrompt(true);
        showToast(
          "Промт скопирован в буфер",
          "Вставьте его в Antigravity, Cursor, Windsurf или новый Claude",
          "success"
        );
        setTimeout(() => setCopiedPrompt(false), 2000);
      }
    } catch (err: any) {
      setClaudeExporting(false);
      showToast("Ошибка копирования", err?.message || String(err), "error");
    }
  };

  const handleCleanSpecificClientCache = async (provider: ProviderId) => {
    const isClaude = provider === "anthropic";
    if (isClaude) setCleaningClaude(true);
    else setCleaningGpt(true);

    try {
      const res = await cleanAppCache(provider);
      const totalFiles = res.files_removed || 0;
      const mbFreed = ((res.bytes_freed || 0) / (1024 * 1024)).toFixed(1);
      const clientName = isClaude ? "Claude Desktop" : "ChatGPT Desktop";
      showToast(
        `Кэш ${clientName} сброшен`,
        `Удалено файлов: ${totalFiles} (${mbFreed} МБ). Cookies и антифрод-реестры очищены.`,
        "success"
      );
    } catch (err: any) {
      showToast("Ошибка сброса", err?.message || String(err), "error");
    } finally {
      if (isClaude) setCleaningClaude(false);
      else setCleaningGpt(false);
    }
  };

  const handleSimulate = async () => {
    try {
      await simulateInterception("anthropic");
      showToast("Тест перехвата", "Вызвано окно экстренного удержания трафика", "info");
      if (onSimulateInBrowser) {
        onSimulateInBrowser();
      }
    } catch (err: any) {
      showToast("Ошибка симуляции", err?.message || String(err), "error");
    }
  };

  const isPinned = state?.pinned_ip != null && state.pinned_ip === currentIp;
  const anyRed =
    reports.anthropic?.overall_status === "red" ||
    reports.openai?.overall_status === "red" ||
    reports.gemini?.overall_status === "red";
  const anyYellow =
    reports.anthropic?.overall_status === "yellow" ||
    reports.openai?.overall_status === "yellow" ||
    reports.gemini?.overall_status === "yellow";

  const trayBadgeText = anyRed
    ? `${currentCountry} · БЛОК`
    : anyYellow
    ? `${currentCountry} · РИСК`
    : `${currentCountry} · ${trustScore}%`;

  const trayDotClass = anyRed
    ? "bg-rose-500 shadow-[0_0_8px_rgba(244,63,94,0.8)]"
    : anyYellow
    ? "bg-amber-400 shadow-[0_0_8px_rgba(251,191,36,0.8)]"
    : "bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.8)]";

  const ipv6Leak =
    activeReport?.ipv6_result?.status === "critical" ||
    activeReport?.ipv6_result?.status === "red" ||
    activeReport?.ipv6_result?.status === "warning";
  const tzMismatch =
    activeReport?.timezone_result?.status === "warning" ||
    activeReport?.timezone_result?.status === "critical" ||
    activeReport?.timezone_result?.status === "yellow" ||
    activeReport?.timezone_result?.status === "red";

  const handleOpenSupport = async () => {
    showToast("Связь / Поддержка", "Переход в Telegram: @nomadnocode", "info");
    await openTelegramSupport();
  };

  return (
    <div className="w-full max-w-[460px] min-h-[660px] bg-[#0A0B0E] text-zinc-100 flex flex-col justify-between p-3.5 select-none font-sans border border-white/[0.08] rounded-xl shadow-2xl relative">
      <ToastContainer toasts={toasts} onDismiss={dismissToast} />

      {/* Top Header */}
      <div>
        {/* Live System Tray Strip (Cross-platform: Windows / macOS / Linux) */}
        <div className="flex items-center justify-between px-2 py-1 mb-2 rounded-md bg-[#12141A] border border-white/[0.06] text-[10px]">
          <div className="flex items-center gap-1.5">
            <span className={`w-2 h-2 rounded-full ${trayDotClass}`} />
            <span className="text-zinc-400 font-medium">Системный трей:</span>
            <span className="font-mono font-bold text-zinc-100 tracking-tight tabular-nums">
              {trayBadgeText}
            </span>
            {isPinned && (
              <span className="px-1 py-0.2 rounded bg-emerald-500/15 text-emerald-300 font-mono text-[9px]">
                PIN
              </span>
            )}
          </div>
          {!isTauri() && (
            <div className="flex items-center gap-1 font-mono text-[9px]">
              {(["clean", "turkey", "ru"] as MockScenario[]).map((sc) => (
                <button
                  key={sc}
                  type="button"
                  onClick={() => handleSwitchScenario(sc)}
                  className={`px-1.5 py-0.5 rounded cursor-pointer transition-colors ${
                    scenario === sc
                      ? "bg-white/[0.12] text-white font-bold"
                      : "text-zinc-500 hover:text-zinc-300"
                  }`}
                >
                  {sc === "clean" ? "NL" : sc === "turkey" ? "TR" : "RU"}
                </button>
              ))}
            </div>
          )}
        </div>

        <header className="pb-2.5 border-b border-white/[0.07] space-y-2">
          {/* Tier 1: Brand & Quick Utilities */}
          <div className="flex items-center justify-between gap-2">
            {/* Logo & Title */}
            <div className="flex items-center gap-2 min-w-0">
              <div className="w-7 h-7 rounded-lg bg-emerald-500/10 border border-emerald-500/25 flex items-center justify-center text-emerald-400 shrink-0">
                <Shield className="w-4 h-4" strokeWidth={1.75} />
              </div>
              <div className="min-w-0">
                <div className="flex items-center gap-1.5">
                  <h1 className="text-xs font-bold tracking-tight text-white whitespace-nowrap">
                    Застава
                  </h1>
                  <button
                    type="button"
                    disabled={checkingUpdate}
                    onClick={() => handleCheckUpdates(false)}
                    title="Проверить обновления из GitHub Releases"
                    className={`text-[9px] px-1.5 py-0.2 rounded border font-mono tabular-nums flex items-center gap-1 cursor-pointer transition-colors shrink-0 whitespace-nowrap ${
                      updateInfo?.update_available
                        ? "bg-amber-500/15 border-amber-500/40 text-amber-300 font-semibold"
                        : "bg-zinc-900 hover:bg-zinc-800 border-white/[0.08] text-zinc-300"
                    }`}
                  >
                    <span>v1.3.1</span>
                    <Download className={`w-2.5 h-2.5 ${checkingUpdate ? "animate-bounce text-emerald-400" : updateInfo?.update_available ? "text-amber-300" : "text-zinc-400"}`} />
                  </button>
                </div>
                <p className="text-[9.5px] text-zinc-500 leading-none mt-0.5 truncate">
                  Защита от бана ИИ · Без чтения чатов
                </p>
              </div>
            </div>

            {/* Quick Utilities: Refresh, Telegram Support, Settings Gear */}
            <div className="flex items-center gap-1.5 shrink-0">
              <button
                type="button"
                disabled={refreshing}
                onClick={() => runFullAudit(true)}
                title="Перепроверить соединение"
                className="whitespace-nowrap shrink-0 flex items-center gap-1 px-2 py-1.5 rounded-lg bg-zinc-900 hover:bg-zinc-800 border border-white/[0.08] text-[10px] font-mono text-zinc-400 hover:text-white transition-colors cursor-pointer"
              >
                <RotateCw
                  className={`w-3 h-3 shrink-0 ${refreshing ? "animate-spin text-emerald-400" : ""}`}
                />
                <span className="tabular-nums text-[9px]">{scanLatencyMs}мс</span>
              </button>

              <button
                type="button"
                onClick={handleOpenSupport}
                title="Связь и поддержка в Telegram: @nomadnocode"
                className="flex items-center gap-1 px-2 py-1.5 rounded-lg bg-sky-500/10 hover:bg-sky-500/20 border border-sky-500/25 text-[10px] font-sans font-medium text-sky-300 hover:text-sky-200 transition-colors cursor-pointer shrink-0 whitespace-nowrap"
              >
                <Send className="w-2.5 h-2.5 text-sky-400 shrink-0" />
                <span>ТГ</span>
              </button>

              {/* Gear Button -> SettingsModal */}
              <button
                type="button"
                onClick={() => setShowSettingsModal(true)}
                title="Настройки и сетевая прозрачность"
                className="p-1.5 rounded-lg bg-zinc-900 hover:bg-zinc-800 border border-white/[0.08] text-zinc-400 hover:text-white transition-colors cursor-pointer shrink-0"
              >
                <Settings className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>

          {/* Tier 2: Tactical Protection & Emergency Kill-Switch Bar */}
          <div className="flex items-center justify-between gap-2">
            <button
              type="button"
              onClick={handleToggleSysproxy}
              title={sysproxyActive ? "Отключить защиту трафика" : "Включить защиту трафика ИИ"}
              className={`flex-1 flex items-center justify-center gap-1.5 px-2.5 py-1.5 rounded-lg text-[10px] font-mono border transition-all cursor-pointer whitespace-nowrap ${
                sysproxyActive
                  ? "bg-emerald-500/15 border-emerald-500/30 text-emerald-300 font-semibold shadow-xs"
                  : "bg-zinc-900 border-white/[0.08] text-zinc-400 hover:text-zinc-200"
              }`}
            >
              <Radio
                className={`w-3 h-3 shrink-0 ${
                  sysproxyActive ? "text-emerald-400 animate-pulse" : "text-zinc-500"
                }`}
              />
              <span>{sysproxyActive ? "ЗАЩИТА ВКЛ" : "ЗАЩИТА ВЫКЛ"}</span>
            </button>

            {/* Kill-switch button */}
            <button
              type="button"
              onClick={handleToggleKillswitch}
              title={
                killswitchFrozen
                  ? "СТОП СЕТИ АКТИВЕН — Нажмите, чтобы разморозить ИИ"
                  : "Экстренно заморозить все запросы к ИИ в 1 клик"
              }
              className={`flex-1 flex items-center justify-center gap-1.5 px-2.5 py-1.5 rounded-lg text-[10px] font-bold tracking-wide border transition-colors cursor-pointer whitespace-nowrap ${
                killswitchFrozen
                  ? "bg-rose-500/20 border-rose-500/50 text-rose-200 animate-pulse font-bold"
                  : "bg-rose-500/10 hover:bg-rose-500/20 border-rose-500/30 text-rose-300"
              }`}
            >
              {killswitchFrozen ? (
                <>
                  <Play className="w-3 h-3 text-emerald-400 fill-emerald-400 shrink-0" />
                  <span>ВКЛЮЧИТЬ СЕТЬ ИИ</span>
                </>
              ) : (
                <>
                  <OctagonAlert className="w-3 h-3 text-rose-400 shrink-0" />
                  <span>СТОП СЕТИ</span>
                </>
              )}
            </button>
          </div>
        </header>

        {/* Segmented Control Navigation (6 tabs) */}
        <nav className="mt-2.5 grid grid-cols-6 gap-1 p-0.5 bg-[#12141A] rounded-lg border border-white/[0.07] text-[10px] font-medium tracking-tight">
          <button
            type="button"
            onClick={() => setActiveTab("pulse")}
            className={`py-1.5 px-0.5 rounded-md transition-all text-center cursor-pointer truncate ${
              activeTab === "pulse"
                ? "bg-white/[0.12] text-white font-bold shadow-xs"
                : "text-zinc-400 hover:text-zinc-200"
            }`}
          >
            Защита
          </button>

          <button
            type="button"
            onClick={() => setActiveTab("clean-ip")}
            className={`py-1.5 px-0.5 rounded-md transition-all text-center cursor-pointer flex items-center justify-center gap-1 truncate ${
              activeTab === "clean-ip"
                ? "bg-white/[0.12] text-white font-bold shadow-xs"
                : "text-zinc-400 hover:text-zinc-200"
            }`}
          >
            <span className="truncate">Чистый IP</span>
            {state?.upstream_proxy?.enabled && (
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse shrink-0" />
            )}
          </button>

          <button
            type="button"
            onClick={() => setActiveTab("claude_vault")}
            title="Бэкап данных Claude и мульти-аккаунт профили"
            className={`py-1.5 px-0.5 rounded-md transition-all text-center cursor-pointer truncate ${
              activeTab === "claude_vault"
                ? "bg-white/[0.12] text-white font-bold shadow-xs"
                : "text-zinc-400 hover:text-zinc-200"
            }`}
          >
            Бэкап
          </button>

          <button
            type="button"
            onClick={() => setActiveTab("bin")}
            className={`py-1.5 px-0.5 rounded-md transition-all text-center cursor-pointer truncate ${
              activeTab === "bin"
                ? "bg-white/[0.12] text-white font-bold shadow-xs"
                : "text-zinc-400 hover:text-zinc-200"
            }`}
          >
            Подписка
          </button>

          <button
            type="button"
            onClick={() => setActiveTab("audit")}
            className={`py-1.5 px-0.5 rounded-md transition-all text-center cursor-pointer truncate ${
              activeTab === "audit"
                ? "bg-white/[0.12] text-white font-bold shadow-xs"
                : "text-zinc-400 hover:text-zinc-200"
            }`}
          >
            Проверки
          </button>

          <button
            type="button"
            onClick={() => setActiveTab("guide")}
            className={`py-1.5 px-0.5 rounded-md transition-all text-center cursor-pointer truncate ${
              activeTab === "guide"
                ? "bg-white/[0.12] text-white font-bold shadow-xs"
                : "text-zinc-400 hover:text-zinc-200"
            }`}
          >
            Гайды
          </button>
        </nav>

        {/* Tauri In-Window Non-blocking Interception Alert Banner */}
        {isTauri() && interceptPayload && (
          <div className="mt-2.5 bg-rose-950/40 border border-rose-500/40 rounded-xl p-2.5 flex items-center justify-between gap-2 shadow-lg animate-in fade-in duration-200">
            <div className="flex items-center gap-2 min-w-0">
              <div className="w-6 h-6 rounded-md bg-rose-500/20 border border-rose-500/30 flex items-center justify-center text-rose-300 shrink-0">
                <ShieldAlert className="w-3.5 h-3.5" />
              </div>
              <div className="min-w-0">
                <div className="text-[11px] font-bold text-rose-200 truncate">
                  Запрос к {interceptPayload.provider.toUpperCase()} приостановлен
                </div>
                <div className="text-[9.5px] text-zinc-400 truncate">
                  {interceptPayload.network_change?.new_country === "RU"
                    ? "Обнаружен выход в сеть РФ без VPN"
                    : `IP: ${interceptPayload.report?.telemetry?.ip || "--"} (${interceptPayload.report?.telemetry?.country_code || "--"})`}
                </div>
              </div>
            </div>
            <div className="flex items-center gap-1.5 shrink-0">
              <button
                type="button"
                onClick={handleAllowInterception}
                className="px-2 py-1 rounded-md bg-emerald-500/20 hover:bg-emerald-500/30 border border-emerald-500/40 text-[10px] font-medium text-emerald-300 transition-colors cursor-pointer"
              >
                Снять паузу (Разрешить)
              </button>
              <button
                type="button"
                onClick={() => setInterceptPayload(null)}
                className="p-1 rounded-md bg-zinc-800/80 hover:bg-zinc-700 text-zinc-400 hover:text-white transition-colors cursor-pointer"
                title="✕ Скрыть"
              >
                <X className="w-3 h-3" />
              </button>
            </div>
          </div>
        )}

        {/* Main Tab Content */}
        <main className="mt-2.5 flex-1">
          {activeTab === "pulse" && (
            <div className="space-y-3 animate-in fade-in duration-150">
              {/* 0. Live System Update Banner */}
              {updateInfo?.update_available && (
                <div className="bg-[#12141A] rounded-xl border border-amber-500/30 p-3 space-y-2.5 shadow-sm font-sans">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2 min-w-0">
                      <span className="w-1.5 h-1.5 rounded-full bg-amber-400 shrink-0 animate-pulse" />
                      <div className="min-w-0">
                        <div className="text-xs font-bold text-white tracking-tight truncate">
                          Доступно обновление v{updateInfo.latest_version}
                        </div>
                        <div className="text-[10px] font-mono text-zinc-400">
                          Текущая: v{updateInfo.current_version || "1.2.0"} · Релиз на GitHub
                        </div>
                      </div>
                    </div>
                    <div className="flex items-center gap-1.5 shrink-0 ml-2">
                      <button
                        type="button"
                        onClick={() => setShowChangelog((v) => !v)}
                        className="px-2 py-1 rounded-md bg-zinc-900 hover:bg-zinc-800 border border-white/[0.08] text-[10px] font-medium text-zinc-300 hover:text-white cursor-pointer transition-colors"
                      >
                        {showChangelog ? "Скрыть" : "Что нового"}
                      </button>
                      <button
                        type="button"
                        disabled={installingUpdate}
                        onClick={handleInstallUpdate}
                        className="px-2.5 py-1 rounded-md bg-emerald-500 hover:bg-emerald-400 active:scale-[0.99] text-zinc-950 font-bold text-[10.5px] cursor-pointer transition-colors flex items-center gap-1 disabled:opacity-50"
                      >
                        <Download className="w-3 h-3" />
                        <span>
                          {installingUpdate
                            ? updaterStatus?.message || updateProgress?.phase || "Установка..."
                            : "Обновить"}
                        </span>
                      </button>
                    </div>
                  </div>

                  {showChangelog && (
                    <div className="bg-black/50 border border-white/[0.06] rounded-lg p-2.5 space-y-2 text-[10.5px] max-h-[170px] overflow-y-auto animate-in fade-in duration-150">
                      <div className="flex items-center justify-between pb-1 border-b border-white/[0.05] text-[9.5px] text-zinc-500 font-mono">
                        <span>СПИСОК ИЗМЕНЕНИЙ</span>
                        <button
                          type="button"
                          onClick={() => setShowFullReleaseText((v) => !v)}
                          className="text-zinc-400 hover:text-zinc-200 underline cursor-pointer"
                        >
                          {showFullReleaseText ? "Краткий список" : "Полный текст"}
                        </button>
                      </div>

                      {showFullReleaseText ? (
                        <div className="whitespace-pre-wrap font-sans text-zinc-300 leading-relaxed text-[10px]">
                          {updateInfo.release_notes || updateInfo.release_name || "Описание релиза отсутствует."}
                        </div>
                      ) : (
                        <div className="space-y-2">
                          {parseReleaseNotes(updateInfo.release_notes || updateInfo.release_name || "").map(
                            (section, sIdx) => (
                              <div key={sIdx} className="space-y-1">
                                {section.title && (
                                  <div className="text-[10px] font-semibold text-zinc-200 font-mono">
                                    {section.title}
                                  </div>
                                )}
                                <ul className="space-y-1 pl-1">
                                  {section.items.map((item, iIdx) => (
                                    <li key={iIdx} className="flex items-start gap-1.5 text-zinc-300 leading-tight">
                                      <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shrink-0 mt-1" />
                                      <span>{item}</span>
                                    </li>
                                  ))}
                                </ul>
                              </div>
                            )
                          )}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              )}

              {/* Plain-Language 3-Step Onboarding Strip for Non-Coders */}
              {!hideOnboarding && (
                <div className="bg-[#12141A] rounded-xl border border-emerald-500/25 p-3 space-y-2 shadow-sm font-sans relative">
                  <div className="flex items-center justify-between pb-1.5 border-b border-white/[0.06]">
                    <div className="flex items-center gap-1.5">
                      <Zap className="w-3.5 h-3.5 text-emerald-400" />
                      <span className="text-[11px] font-bold text-white tracking-tight">
                        Как пользоваться Заставой (3 простых шага)
                      </span>
                    </div>
                    <button
                      type="button"
                      onClick={() => {
                        setHideOnboarding(true);
                        try {
                          localStorage.setItem("zastava_hide_onboarding_v13", "true");
                        } catch {}
                      }}
                      className="text-[9.5px] text-zinc-400 hover:text-white px-1.5 py-0.5 rounded bg-zinc-900 hover:bg-zinc-800 border border-white/[0.06] cursor-pointer transition-colors"
                    >
                      Понятно, скрыть ✕
                    </button>
                  </div>
                  <div className="grid grid-cols-3 gap-2 pt-0.5">
                    <div className="bg-zinc-900/60 rounded-lg p-2 border border-white/[0.04] space-y-1">
                      <div className="text-[10.5px] font-bold text-emerald-300">
                        1. Включите ваш VPN
                      </div>
                      <p className="text-[9.5px] text-zinc-400 leading-tight">
                        Застава сама проверит чистоту IP-адреса и совпадение часового пояса
                      </p>
                    </div>
                    <div className="bg-zinc-900/60 rounded-lg p-2 border border-white/[0.04] space-y-1">
                      <div className="text-[10.5px] font-bold text-sky-300">
                        2. Откройте ИИ как обычно
                      </div>
                      <p className="text-[9.5px] text-zinc-400 leading-tight">
                        Работайте в браузере, приложении Claude, Cursor или Word. Обычные сайты идут напрямую без тормозов
                      </p>
                    </div>
                    <div className="bg-zinc-900/60 rounded-lg p-2 border border-white/[0.04] space-y-1">
                      <div className="text-[10.5px] font-bold text-amber-300">
                        3. Авто-защита от бана
                      </div>
                      <p className="text-[9.5px] text-zinc-400 leading-tight">
                        Если VPN оборвётся, Застава мгновенно поставит ИИ на паузу, чтобы ваш российский IP не улетел в нейросеть
                      </p>
                    </div>
                  </div>
                </div>
              )}

              {/* 1. Главный вердикт (Human-Friendly Status Hero Card) */}
              <div className="bg-[#12141A] rounded-xl border border-white/[0.08] p-3.5 space-y-3 shadow-xs">
                <div className="flex items-center justify-between text-[10px] text-zinc-400 pb-2 border-b border-white/[0.05]">
                  <div className="flex items-center gap-1.5">
                    <span className="uppercase tracking-wider font-semibold">Ваш IP и безопасность</span>
                    <span className="text-zinc-600">·</span>
                    {state?.upstream_proxy?.enabled ? (
                      <span className="px-1.5 py-0.2 rounded bg-emerald-500/15 border border-emerald-500/30 text-emerald-300 font-mono text-[9px] font-bold flex items-center gap-1">
                        <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
                        <span>Ключ Заставы 2-Hop</span>
                      </span>
                    ) : (
                      <span className="px-1.5 py-0.2 rounded bg-zinc-900 border border-white/[0.07] text-zinc-400 font-mono text-[9px]">
                        Системная сеть
                      </span>
                    )}
                  </div>
                  <div className="flex items-center gap-1.5">
                    <span className={`w-1.5 h-1.5 rounded-full ${trayDotClass}`} />
                    <span className="text-zinc-300 font-mono text-[9.5px]">АКТИВНО</span>
                  </div>
                </div>

                <div className="flex items-center gap-3.5">
                  <TrustRing score={trustScore} size={54} strokeWidth={4.5} />

                  <div className="flex-1 min-w-0 space-y-1">
                    <div className="flex items-center justify-between">
                      <div
                        onClick={handleCopyIp}
                        className="flex items-center gap-1.5 cursor-pointer group"
                        title="Нажмите, чтобы скопировать IP"
                      >
                        <span className="text-sm font-bold text-white font-mono tabular-nums tracking-tight group-hover:text-emerald-400 transition-colors">
                          {currentIp}
                        </span>
                        {copiedIp ? (
                          <Check className="w-3.5 h-3.5 text-emerald-400" />
                        ) : (
                          <Copy className="w-3 h-3 text-zinc-500 opacity-0 group-hover:opacity-100 transition-opacity" />
                        )}
                      </div>

                      <span className="px-2 py-0.5 rounded bg-zinc-900 border border-white/[0.08] text-[11px] font-mono text-zinc-200 font-bold">
                        {currentCountry}
                      </span>
                    </div>

                    <div className="text-[11px] text-zinc-400 font-mono truncate">
                      {activeReport?.telemetry.asn
                        ? `AS${activeReport.telemetry.asn} · ${
                            activeReport.telemetry.isp || activeReport.telemetry.org || "Хостинг"
                          }`
                        : activeReport?.telemetry.isp || activeReport?.telemetry.org
                        ? `${activeReport.telemetry.isp || activeReport.telemetry.org}`
                        : activeReport?.telemetry.ip && activeReport.telemetry.ip !== "--"
                        ? `Узел ${activeReport.telemetry.country_code || "VPN"} · Защищённый шлюз`
                        : "Определение провайдера..."}
                    </div>

                    <div className="flex items-center gap-2 text-[10px] font-mono text-zinc-500 tabular-nums">
                      <span>
                        DNSBL:{" "}
                        <strong
                          className={
                            activeReport?.dnsbl_result?.status === "critical"
                              ? "text-rose-400"
                              : "text-emerald-400"
                          }
                        >
                          {activeReport?.dnsbl_clean_count || "4/4 чисто"}
                        </strong>
                      </span>
                      <span>·</span>
                      <span>
                        IPv6:{" "}
                        <strong className={ipv6Leak ? "text-rose-400" : "text-emerald-400"}>
                          {ipv6Leak ? "УТЕЧКА" : "Выкл"}
                        </strong>
                      </span>
                      <span>·</span>
                      <span>
                        TZ:{" "}
                        <strong className={tzMismatch ? "text-amber-400" : "text-zinc-300"}>
                          {tzMismatch ? "Рассинхрон" : "OK"}
                        </strong>
                      </span>
                    </div>
                  </div>
                </div>

                {/* Human-Friendly Verdict Message */}
                <div
                  className={`p-2.5 rounded-lg border text-xs leading-relaxed flex items-start gap-2 ${
                    anyRed
                      ? "bg-rose-500/10 border-rose-500/25 text-rose-200"
                      : anyYellow
                      ? "bg-amber-500/10 border-amber-500/25 text-amber-200"
                      : "bg-emerald-500/10 border-emerald-500/25 text-emerald-200"
                  }`}
                >
                  {anyRed ? (
                    <ShieldAlert className="w-4 h-4 text-rose-400 shrink-0 mt-0.5" />
                  ) : anyYellow ? (
                    <AlertTriangle className="w-4 h-4 text-amber-400 shrink-0 mt-0.5" />
                  ) : (
                    <ShieldCheck className="w-4 h-4 text-emerald-400 shrink-0 mt-0.5" />
                  )}
                  <div className="flex-1 text-[11px] font-sans">
                    {anyRed ? (
                      <span>
                        <strong className="font-semibold text-rose-300">Опасно! </strong>
                        Обнаружен неподдерживаемый регион или критическая утечка. Запросы к ИИ автоматически удерживаются шлюзом.
                      </span>
                    ) : anyYellow ? (
                      <div className="space-y-1">
                        <span>
                          <strong className="font-semibold text-amber-300">Внимание: </strong>
                          {tzMismatch
                            ? "Часовой пояс компьютера не совпадает со страной IP (повышает риск проверок биллинга)."
                            : activeReport?.dnsbl_result?.status === "critical" ||
                              activeReport?.dnsbl_result?.status === "warning"
                            ? "IP замечен в спам-базах, коммерческий VPN может вызвать проверку платформы."
                            : "Обнаружены предупреждения безопасности сети. Рекомендуется использовать выделенный IP."}
                        </span>
                        {tzMismatch && (
                          <div className="pt-2 flex flex-wrap items-center gap-2">
                            <button
                              type="button"
                              onClick={() => handleSyncTz()}
                              disabled={syncingTz}
                              className="px-2.5 py-1 rounded bg-amber-500/20 hover:bg-amber-500/30 border border-amber-500/40 text-amber-300 text-[10.5px] font-sans font-semibold flex items-center gap-1 cursor-pointer transition-colors"
                            >
                              <Zap className="w-3 h-3 text-amber-400" />
                              <span>
                                {tzSyncedNotice
                                  ? "TZ синхронизирован!"
                                  : syncingTz
                                  ? "Синхронизация..."
                                  : "⚡ Авто-TZ для ИИ"}
                              </span>
                            </button>

                            <button
                              type="button"
                              onClick={handleOpenOsClockAndGuide}
                              className="px-2.5 py-1 rounded bg-zinc-900 hover:bg-zinc-800 border border-white/[0.08] text-zinc-300 hover:text-white text-[10.5px] font-sans font-medium flex items-center gap-1 cursor-pointer transition-colors"
                            >
                              <Clock className="w-3 h-3 text-zinc-400" />
                              <span>🕒 Часы системы и Гайд →</span>
                            </button>
                          </div>
                        )}
                      </div>
                    ) : (
                      <span>
                        <strong className="font-semibold text-emerald-300">Вход безопасен: </strong>
                        Вход в Claude и ChatGPT защищен. Ваш IP чистый, чаты зашифрованы, утечек региона не обнаружено.
                      </span>
                    )}
                  </div>
                </div>

                {/* ЖИВОЙ ПУЛЬС ЗАЩИТЫ · БЕЗ ЧТЕНИЯ ЧАТОВ */}
                <div className="pt-2.5 border-t border-white/[0.06] space-y-2">
                  <div className="flex items-center justify-between text-[10px]">
                    <span className="text-zinc-400 font-semibold uppercase tracking-wider flex items-center gap-1.5 font-sans">
                      <span className="relative flex h-2 w-2">
                        <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75" />
                        <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500" />
                      </span>
                      <span>ЖИВОЙ ПУЛЬС ЗАЩИТЫ · БЕЗ ЧТЕНИЯ ЧАТОВ</span>
                    </span>
                    <span className="text-zinc-500 font-mono text-[9px] tabular-nums">
                      {gatewayStats?.uptime_seconds ? `Uptime ${Math.floor(gatewayStats.uptime_seconds / 60)}м` : "Live Gateway"}
                    </span>
                  </div>

                  {/* 3 Interactive Counter Cards */}
                  <div className="grid grid-cols-3 gap-2">
                    {/* Card 1: Защищено ИИ */}
                    <div
                      className={`bg-[#0c0e14] rounded-lg p-2 border transition-all ${
                        pulseProtected
                          ? "border-emerald-400 ring-1 ring-emerald-400/50 scale-[1.02]"
                          : "border-emerald-500/25 hover:border-emerald-500/40"
                      }`}
                    >
                      <div className="text-[10px] font-sans text-emerald-400 font-semibold flex items-center gap-1">
                        <ShieldCheck className="w-3 h-3 text-emerald-400 shrink-0" />
                        <span className="truncate">Защищено ИИ</span>
                      </div>
                      <div className="text-base font-bold font-mono text-emerald-300 mt-0.5 tabular-nums">
                        {gatewayStats?.protected_ai_requests ?? 0}
                      </div>
                      <div className="text-[9px] font-sans text-zinc-500 mt-0.5 leading-tight truncate">
                        пакетов проверено
                      </div>
                    </div>

                    {/* Card 2: Сбоев VPN отбито */}
                    <div className="bg-[#0c0e14] rounded-lg p-2 border border-rose-500/25 hover:border-rose-500/40 transition-all">
                      <div className="text-[10px] font-sans text-rose-400 font-semibold flex items-center gap-1">
                        <ShieldAlert className="w-3 h-3 text-rose-400 shrink-0" />
                        <span className="truncate">Сбоев VPN отбито</span>
                      </div>
                      <div className="text-base font-bold font-mono text-rose-300 mt-0.5 tabular-nums">
                        {gatewayStats?.intercepted_leaks ?? 0}
                      </div>
                      <div className="text-[9px] font-sans text-zinc-500 mt-0.5 leading-tight truncate">
                        утечек остановлено
                      </div>
                    </div>

                    {/* Card 3: Обычные сайты */}
                    <div className="bg-[#0c0e14] rounded-lg p-2 border border-sky-500/25 hover:border-sky-500/40 transition-all">
                      <div className="text-[10px] font-sans text-sky-400 font-semibold flex items-center gap-1">
                        <Zap className="w-3 h-3 text-sky-400 shrink-0" />
                        <span className="truncate">Обычные сайты</span>
                      </div>
                      <div className="text-base font-bold font-mono text-sky-300 mt-0.5 tabular-nums">
                        {gatewayStats?.direct_bypassed_requests ?? 0}
                      </div>
                      <div className="text-[9px] font-sans text-zinc-500 mt-0.5 leading-tight truncate">
                        прямой транзит · 0 мс
                      </div>
                    </div>
                  </div>

                  {/* Calm explanation string */}
                  <div className="text-[10px] text-neutral-500 leading-relaxed font-sans px-0.5">
                    Все запросы к ИИ проходят 12 проверок шлюза. Обычные сайты, GitHub и менеджеры пакетов идут напрямую без задержки. Чаты зашифрованы.
                  </div>

                  {/* Expandable radar feed */}
                  <div
                    onClick={() => setFeedExpanded(!feedExpanded)}
                    className="bg-[#0c0e14] hover:bg-[#11131c] rounded-lg px-2.5 py-1.5 border border-white/[0.06] hover:border-white/[0.12] cursor-pointer transition-colors space-y-1.5"
                    title="Нажмите, чтобы развернуть/свернуть живую ленту событий шлюза"
                  >
                    <div className="flex items-center justify-between text-[10px]">
                      <div className="flex items-center gap-1.5 text-zinc-300 font-mono text-[9.5px] truncate">
                        <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse shrink-0" />
                        <span>Активных туннелей ИИ: <strong className="text-emerald-300 font-bold">{gatewayStats?.active_ai_tunnels ?? 0}</strong></span>
                      </div>
                      <div className="flex items-center gap-1 text-[9px] font-sans text-zinc-400 hover:text-zinc-200 shrink-0">
                        <span>{feedExpanded ? "Свернуть" : "Лента событий"}</span>
                        <ChevronRight className={`w-3 h-3 transition-transform duration-200 ${feedExpanded ? "rotate-90" : ""}`} />
                      </div>
                    </div>

                    <div className="text-[9.5px] font-mono text-zinc-400 truncate">
                      <span className="text-zinc-500">Последний: </span>
                      <span className="text-zinc-200">
                        {gatewayStats?.last_protected_host || "Ожидание запросов Claude / GPT / Gemini..."}
                      </span>
                    </div>

                    {feedExpanded && (
                      <div className="pt-2 border-t border-white/[0.05] space-y-1.5 animate-in fade-in duration-150">
                        {(!gatewayStats?.recent_events || gatewayStats.recent_events.length === 0) ? (
                          <div className="text-[10px] font-sans text-zinc-500 text-center py-1.5">
                            Ожидание первых запросов Claude, GPT, Gemini...
                          </div>
                        ) : (
                          gatewayStats.recent_events.slice().reverse().map((ev, idx) => {
                            const isDirect =
                              ev.action?.toUpperCase() === "DIRECT" ||
                              ev.kind === "direct" ||
                              ev.provider?.toLowerCase() === "non-ai" ||
                              ev.country === "DIRECT";
                            const isProtected = ev.action?.toUpperCase() === "PROTECTED";

                            return (
                              <div
                                key={idx}
                                className="flex items-center justify-between text-[10px] bg-black/40 px-2 py-1 rounded border border-white/[0.04]"
                              >
                                <div className="flex items-center gap-1.5 min-w-0 truncate">
                                  <span className="font-mono text-[8.5px] text-zinc-500 tabular-nums shrink-0">
                                    {new Date(ev.timestamp > 1e11 ? ev.timestamp : ev.timestamp * 1000).toLocaleTimeString([], {
                                      hour: "2-digit",
                                      minute: "2-digit",
                                      second: "2-digit",
                                    })}
                                  </span>
                                  <span className="font-mono text-zinc-200 text-[9.5px] truncate">{ev.host}</span>
                                </div>
                                <div className="flex items-center gap-1 shrink-0 ml-1.5">
                                  {isDirect ? (
                                    <div className="flex items-center gap-1 px-1.5 py-0.2 rounded bg-neutral-800/70 border border-neutral-700/60 text-neutral-400 text-[8.5px] font-mono font-medium">
                                      <span className="w-1.5 h-1.5 rounded-full bg-neutral-400 shrink-0" />
                                      <span>НЕ-ИИ · НАПРЯМУЮ</span>
                                    </div>
                                  ) : isProtected ? (
                                    <div className="flex items-center gap-1 px-1.5 py-0.2 rounded bg-emerald-500/15 border border-emerald-500/30 text-emerald-400 text-[8.5px] font-mono font-bold">
                                      <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shrink-0" />
                                      <span>ИИ · ЗАЩИЩЕНО · {ev.country || "NL"}</span>
                                    </div>
                                  ) : (
                                    <div className="flex items-center gap-1 px-1.5 py-0.2 rounded bg-rose-500/15 border border-rose-500/30 text-rose-400 text-[8.5px] font-mono font-bold">
                                      <span className="w-1.5 h-1.5 rounded-full bg-rose-400 shrink-0" />
                                      <span>ИИ · УДЕРЖАНО</span>
                                    </div>
                                  )}
                                </div>
                              </div>
                            );
                          })
                        )}
                      </div>
                    )}
                  </div>
                </div>
              </div>

              {/* БЫСТРЫЕ ДЕЙСТВИЯ (Quick Actions Bar) */}
              <div className="bg-[#12141A] rounded-xl border border-white/[0.08] p-3 space-y-2 shadow-xs">
                <div className="flex items-center justify-between text-[10px] text-zinc-400 pb-1.5 border-b border-white/[0.05]">
                  <span className="font-semibold uppercase tracking-wider">БЫСТРЫЕ ДЕЙСТВИЯ</span>
                  <span className="text-[9.5px] font-mono text-emerald-400">Чаты зашифрованы</span>
                </div>

                <div className="grid grid-cols-2 gap-2">
                  <button
                    type="button"
                    onClick={() => setShowCrashTestModal(true)}
                    className="py-2 px-2.5 rounded-lg bg-emerald-500 hover:bg-emerald-400 active:scale-[0.99] text-zinc-950 text-xs font-bold flex items-center justify-center gap-1.5 cursor-pointer shadow-md shadow-emerald-500/15 transition-all truncate"
                  >
                    <Activity className="w-3.5 h-3.5 shrink-0" />
                    <span className="truncate">Краш-тест обрыва VPN</span>
                  </button>

                  <button
                    type="button"
                    onClick={() => handleSyncTz()}
                    disabled={syncingTz}
                    className="py-2 px-2.5 rounded-lg bg-zinc-900 hover:bg-zinc-800 active:scale-[0.99] border border-white/[0.1] hover:border-white/[0.2] text-zinc-200 text-xs font-semibold flex items-center justify-center gap-1.5 cursor-pointer transition-all truncate"
                  >
                    <Zap className="w-3.5 h-3.5 text-amber-400 shrink-0" />
                    <span className="truncate">{syncingTz ? "Синхронизация..." : "Авто-TZ для ИИ"}</span>
                  </button>
                </div>
              </div>

              {/* 2. Карточка «Что сейчас под защитой» */}
              <div className="bg-[#12141A] rounded-xl border border-white/[0.08] p-3.5 space-y-2.5 shadow-xs">
                <div className="flex items-center justify-between pb-1.5 border-b border-white/[0.05]">
                  <span className="text-[10px] tracking-wider uppercase text-zinc-400 font-semibold">
                    Что сейчас под защитой
                  </span>
                  <span className="text-[10px] font-mono text-emerald-400 font-medium">Без чтения чатов</span>
                </div>

                <div className="space-y-2.5 divide-y divide-white/[0.04]">
                  {/* 1. Браузеры */}
                  <div className="flex items-center justify-between pt-1 first:pt-0">
                    <div className="flex items-center gap-2.5 min-w-0">
                      <div className="w-7 h-7 rounded-lg bg-zinc-900 border border-white/[0.06] flex items-center justify-center text-zinc-300 shrink-0">
                        <Globe className="w-3.5 h-3.5" />
                      </div>
                      <div className="min-w-0">
                        <div className="text-xs font-semibold text-white">Браузеры</div>
                        <div className="text-[10px] text-zinc-400 truncate">
                          Chrome, Safari, Arc, Edge, Яндекс (через PAC)
                        </div>
                      </div>
                    </div>

                    <div className="flex items-center gap-1.5 shrink-0">
                      <span
                        className={`px-2 py-0.5 rounded text-[10px] font-mono font-semibold border ${
                          sysproxyActive
                            ? "bg-emerald-500/10 border-emerald-500/25 text-emerald-400"
                            : "bg-zinc-900 border-white/[0.06] text-zinc-500"
                        }`}
                      >
                        {sysproxyActive ? "Защищено" : "Отключено"}
                      </span>
                      <button
                        type="button"
                        onClick={handleToggleSysproxy}
                        className="px-2 py-1 rounded bg-zinc-900 hover:bg-zinc-800 border border-white/[0.08] text-[10px] font-medium text-zinc-300 hover:text-white cursor-pointer transition-colors"
                      >
                        {sysproxyActive ? "Выкл" : "Вкл"}
                      </button>
                    </div>
                  </div>

                  {/* 2. Редакторы кода */}
                  <div className="flex items-center justify-between pt-2.5">
                    <div className="flex items-center gap-2.5 min-w-0">
                      <div className="w-7 h-7 rounded-lg bg-zinc-900 border border-white/[0.06] flex items-center justify-center text-sky-400 shrink-0">
                        <Terminal className="w-3.5 h-3.5" />
                      </div>
                      <div className="min-w-0">
                        <div className="text-xs font-semibold text-white">Редакторы кода и CLI</div>
                        <div className="text-[10px] text-zinc-400 truncate">
                          VS Code, Cursor, Windsurf, терминал
                        </div>
                      </div>
                    </div>

                    <div className="flex items-center gap-1.5 shrink-0">
                      <span
                        className={`px-2 py-0.5 rounded text-[10px] font-mono font-semibold border ${
                          copiedCli
                            ? "bg-emerald-500/10 border-emerald-500/25 text-emerald-400"
                            : "bg-zinc-900 border-white/[0.06] text-zinc-500"
                        }`}
                      >
                        {copiedCli ? "Защищено" : "Отключено"}
                      </span>
                      <button
                        type="button"
                        onClick={handleToggleCliProtection}
                        className="px-2 py-1 rounded bg-zinc-900 hover:bg-zinc-800 border border-white/[0.08] text-[10px] font-medium text-zinc-300 hover:text-white cursor-pointer transition-colors"
                      >
                        {copiedCli ? "Выкл" : "Вкл"}
                      </button>
                    </div>
                  </div>

                  {/* 3. Закрепление безопасного IP */}
                  <div className="flex items-center justify-between pt-2.5">
                    <div className="flex items-center gap-2.5 min-w-0">
                      <div
                        className={`w-7 h-7 rounded-lg border flex items-center justify-center shrink-0 ${
                          isPinned
                            ? "bg-emerald-500/10 border-emerald-500/25 text-emerald-400"
                            : "bg-zinc-900 border-white/[0.06] text-zinc-400"
                        }`}
                      >
                        <Pin className="w-3.5 h-3.5" />
                      </div>
                      <div className="min-w-0">
                        <div className="text-xs font-semibold text-white">
                          {state?.pinned_ip ? `Закреплен: ${state.pinned_ip}` : "Закрепление безопасного IP"}
                        </div>
                        <div className="text-[10px] text-zinc-400 truncate">
                          {state?.pinned_ip
                            ? state.strict_pinning_enabled
                              ? "Строгий режим: блокирует запросы при смене сети"
                              : "Режим уведомлений при смене IP"
                            : "Защищает от банов при случайной смене VPN-сервера"}
                        </div>
                      </div>
                    </div>

                    <div className="flex items-center gap-1 shrink-0">
                      {state?.pinned_ip ? (
                        <>
                          <button
                            type="button"
                            onClick={handleToggleStrict}
                            title="Переключить строгий режим"
                            className={`px-1.5 py-1 rounded border text-[9.5px] font-mono cursor-pointer ${
                              state.strict_pinning_enabled
                                ? "bg-emerald-500/10 border-emerald-500/25 text-emerald-400 font-bold"
                                : "bg-zinc-900 border-white/[0.06] text-zinc-400"
                            }`}
                          >
                            {state.strict_pinning_enabled ? "СТРОГИЙ" : "УВЕДОМЛ."}
                          </button>
                          <button
                            type="button"
                            onClick={handleUnpinIp}
                            className="px-1.5 py-1 rounded bg-zinc-900 hover:bg-zinc-800 border border-white/[0.06] text-[9.5px] text-zinc-400 hover:text-rose-400 cursor-pointer"
                          >
                            Сброс
                          </button>
                        </>
                      ) : (
                        <button
                          type="button"
                          onClick={() => handlePinIp(currentIp)}
                          className="flex items-center gap-1 px-2.5 py-1 rounded-md bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-emerald-300 text-[10.5px] font-semibold cursor-pointer transition-colors"
                        >
                          <Pin className="w-3 h-3" />
                          <span>Закрепить IP</span>
                        </button>
                      )}
                    </div>
                  </div>
                </div>
              </div>

              {/* 3. Статус по сервисам (ProviderMatrix) */}
              <ProviderMatrix
                reports={reports}
                state={state}
                onNavigateToGuide={(guideId) => {
                  if (guideId) setInitialGuideTarget(guideId);
                  setActiveTab("guide");
                }}
                onAllowProvider={handleAllowProvider}
                onUnblockProvider={handleUnblockProvider}
                onRefresh={() => runFullAudit(true)}
              />

              {/* Quick Navigation Card to Claude Vault (Backup & Profiles) */}
              <div
                onClick={() => setActiveTab("claude_vault")}
                className="p-3 rounded-xl bg-gradient-to-r from-emerald-950/30 via-zinc-900 to-[#12141A] border border-white/[0.08] hover:border-emerald-500/40 cursor-pointer flex items-center justify-between transition-all group shadow-xs"
              >
                <div className="flex items-center gap-2.5 min-w-0">
                  <div className="w-7 h-7 rounded-lg bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 flex items-center justify-center shrink-0">
                    <FolderArchive className="w-3.5 h-3.5" />
                  </div>
                  <div className="min-w-0">
                    <div className="text-xs font-semibold text-white group-hover:text-emerald-300 transition-colors flex items-center gap-1.5 truncate">
                      <span>Бэкап памяти Claude и мульти-аккаунты</span>
                    </div>
                    <div className="text-[10px] text-zinc-400 truncate">
                      Нужен бэкап памяти Claude или переключение между аккаунтами? → Открыть вкладку «Бэкап и Профили»
                    </div>
                  </div>
                </div>
                <ChevronRight className="w-4 h-4 text-emerald-400 group-hover:translate-x-1 transition-transform shrink-0" />
              </div>

              {/* 5. Баннер быстрого перехода в «Чистый IP» */}
              <div
                onClick={() => setActiveTab("clean-ip")}
                className="p-3 rounded-xl bg-gradient-to-r from-emerald-950/40 via-sky-950/20 to-[#12141A] border border-emerald-500/30 hover:border-emerald-500/50 cursor-pointer flex items-center justify-between transition-all group shadow-sm"
              >
                <div className="flex items-center gap-2.5">
                  <div className="w-7 h-7 rounded-lg bg-emerald-500/20 text-emerald-400 flex items-center justify-center shrink-0">
                    <Key className="w-4 h-4" />
                  </div>
                  <div>
                    <div className="text-xs font-bold text-white group-hover:text-emerald-300 transition-colors flex items-center gap-1.5">
                      <span>Чистый IP только для ИИ (2-Hop)</span>
                      {state?.upstream_proxy?.enabled && (
                        <span className="px-1.5 py-0.2 rounded bg-emerald-500/20 text-emerald-300 font-mono text-[9px] font-bold">
                          АКТИВЕН
                        </span>
                      )}
                    </div>
                    <div className="text-[10.5px] text-zinc-400">
                      Подключить выделенный ключ или настроить свой VPS за 15 минут
                    </div>
                  </div>
                </div>
                <ChevronRight className="w-4 h-4 text-emerald-400 group-hover:translate-x-1 transition-transform" />
              </div>
            </div>
          )}

          {activeTab === "clean-ip" && (
            <div className="animate-in fade-in duration-150">
              <CleanIpTab
                state={state}
                onRefreshState={async () => {
                  const st = await fetchAppState();
                  setState(st);
                }}
                onAuditRefresh={async () => {
                  await runFullAudit(true);
                }}
                onOpenGuide={(ref) => {
                  setInitialGuideTarget(ref);
                  setActiveTab("guide");
                }}
                onToast={showToast}
              />
            </div>
          )}

          {activeTab === "claude_vault" && (
            <div className="space-y-3.5 animate-in fade-in duration-150">
              {/* 1. Claude Data Backup & Export / Import card */}
              <div className="bg-[#12141A] rounded-xl border border-white/[0.08] p-3.5 space-y-3 shadow-xs">
                <div className="flex items-center justify-between pb-2 border-b border-white/[0.05]">
                  <div className="flex items-center gap-2 min-w-0">
                    <div className="w-6 h-6 rounded-md bg-emerald-500/10 border border-emerald-500/25 flex items-center justify-center text-emerald-400 shrink-0">
                      <FolderArchive className="w-3.5 h-3.5" />
                    </div>
                    <div className="min-w-0">
                      <div className="text-xs font-bold text-white tracking-tight flex items-center gap-1.5 truncate">
                        <span>Экспорт и бэкап данных Claude</span>
                        <span className="px-1.5 py-0.2 rounded bg-emerald-500/15 text-emerald-300 font-mono text-[9px] font-bold">
                          ~/.claude
                        </span>
                      </div>
                      <div className="text-[10px] text-zinc-400 truncate">
                        Резервное копирование проектов, сессий и контекста в 1 клик
                      </div>
                    </div>
                  </div>

                  <button
                    type="button"
                    disabled={loadingClaudeSummary}
                    onClick={loadClaudeSummary}
                    className="p-1 rounded-md text-zinc-400 hover:text-white hover:bg-white/[0.06] cursor-pointer transition-colors shrink-0"
                    title="Пересканировать директорию ~/.claude"
                  >
                    <RotateCw
                      className={`w-3 h-3 ${
                        loadingClaudeSummary ? "animate-spin text-emerald-400" : ""
                      }`}
                    />
                  </button>
                </div>

                <div className="text-[10.5px] text-zinc-300 leading-relaxed bg-[#0c0e14] rounded-lg p-2.5 border border-white/[0.04]">
                  Автоматически собирает историю диалогов, проекты и долговременную память Claude из директории <span className="text-zinc-100 font-mono">~/.claude</span>. Позволяет перенести накопленный контекст на другой аккаунт или упаковать защищенный архив на Рабочий стол перед сменой профиля.
                </div>

                {/* Stats summary */}
                <div className="bg-black/50 rounded-lg px-3 py-2 border border-white/[0.04] text-[10px] font-mono tabular-nums text-zinc-300">
                  <div className="flex items-center justify-between flex-wrap gap-x-2 gap-y-1">
                    <span>
                      Проектов:{" "}
                      <strong className="text-emerald-300 font-bold">
                        {claudeSummary?.projects_count ?? 0}
                      </strong>
                    </span>
                    <span>·</span>
                    <span>
                      Сессий (.jsonl):{" "}
                      <strong className="text-emerald-300 font-bold">
                        {claudeSummary?.sessions_count ?? 0}
                      </strong>
                    </span>
                    <span>·</span>
                    <span>
                      Память:{" "}
                      <strong className="text-emerald-300 font-bold">
                        {claudeSummary?.memory_files_count ?? 0}
                      </strong>
                    </span>
                    <span>·</span>
                    <span>
                      Объем:{" "}
                      <strong className="text-emerald-300 font-bold">
                        {((claudeSummary?.total_size_bytes ?? 0) / (1024 * 1024)).toFixed(1)} МБ
                      </strong>
                    </span>
                  </div>
                </div>

                {/* Export & copy buttons */}
                <div className="grid grid-cols-2 gap-2 pt-0.5">
                  <button
                    type="button"
                    disabled={claudeExporting}
                    onClick={handleExportClaude}
                    className="py-2 px-3 rounded-lg bg-emerald-500 hover:bg-emerald-400 active:scale-[0.99] text-zinc-950 text-xs font-sans font-bold flex items-center justify-center gap-1.5 cursor-pointer disabled:opacity-50 transition-colors shadow-sm truncate"
                  >
                    <FolderArchive className="w-3.5 h-3.5 shrink-0" />
                    <span className="truncate">
                      {claudeExporting ? "Упаковка..." : "Упаковать архив на Рабочий стол"}
                    </span>
                  </button>

                  <button
                    type="button"
                    disabled={claudeExporting}
                    onClick={handleCopyRestorePrompt}
                    className="py-2 px-3 rounded-lg bg-zinc-900 hover:bg-zinc-800 active:scale-[0.99] border border-white/[0.1] hover:border-white/[0.2] text-zinc-200 text-xs font-sans font-medium flex items-center justify-center gap-1.5 cursor-pointer disabled:opacity-50 transition-colors truncate"
                  >
                    {copiedPrompt ? (
                      <>
                        <Check className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                        <span className="text-emerald-300 truncate">Промт скопирован</span>
                      </>
                    ) : (
                      <>
                        <Copy className="w-3.5 h-3.5 text-zinc-400 shrink-0" />
                        <span className="truncate">Скопировать промт переноса</span>
                      </>
                    )}
                  </button>
                </div>

                {claudeExportResult && (
                  <div className="bg-emerald-500/10 border border-emerald-500/25 rounded-lg p-2.5 text-[10px] font-mono text-emerald-300 space-y-1 animate-in fade-in duration-150">
                    <div className="font-bold flex items-center gap-1.5 text-xs">
                      <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                      <span>Архив успешно сохранен на Рабочем столе:</span>
                    </div>
                    <div className="text-zinc-300 truncate text-[10px] pl-5">
                      {claudeExportResult.zip_archive_path || claudeExportResult.export_dir_path}
                    </div>
                  </div>
                )}

                {/* Deep Cache Cleaner */}
                <div className="bg-[#0c0e14] rounded-lg p-3 border border-white/[0.06] space-y-2 pt-2.5">
                  <div className="text-[11px] font-semibold text-zinc-200 flex items-center justify-between">
                    <div className="flex items-center gap-1.5">
                      <span className="w-1.5 h-1.5 rounded-full bg-rose-400 shrink-0" />
                      <span>Глубокий сброс кэша и меток устройства</span>
                    </div>
                    <span className="text-[9.5px] font-mono text-zinc-500">
                      Сброс Device ID и сессий
                    </span>
                  </div>

                  <div className="grid grid-cols-2 gap-2 pt-0.5">
                    <button
                      type="button"
                      disabled={cleaningClaude}
                      onClick={() => handleCleanSpecificClientCache("anthropic")}
                      className="py-1.5 px-2 rounded-lg bg-zinc-900 hover:bg-zinc-800 active:scale-[0.99] border border-rose-500/30 hover:border-rose-500/50 text-rose-300 text-[10.5px] font-sans font-medium flex items-center justify-center gap-1.5 cursor-pointer disabled:opacity-50 transition-colors truncate"
                    >
                      <Trash2 className="w-3 h-3 text-rose-400 shrink-0" />
                      <span className="truncate">
                        {cleaningClaude ? "Очистка..." : "Кэш Claude Desktop"}
                      </span>
                    </button>

                    <button
                      type="button"
                      disabled={cleaningGpt}
                      onClick={() => handleCleanSpecificClientCache("openai")}
                      className="py-1.5 px-2 rounded-lg bg-zinc-900 hover:bg-zinc-800 active:scale-[0.99] border border-white/[0.1] hover:border-white/[0.2] text-zinc-300 hover:text-white text-[10.5px] font-sans font-medium flex items-center justify-center gap-1.5 cursor-pointer disabled:opacity-50 transition-colors truncate"
                    >
                      <Trash2 className="w-3 h-3 text-zinc-400 shrink-0" />
                      <span className="truncate">
                        {cleaningGpt ? "Очистка..." : "Кэш ChatGPT Desktop"}
                      </span>
                    </button>
                  </div>
                </div>
              </div>

              {/* 2. Claude Multi-Account Sandbox Card right below it */}
              <div id="claude-profiles-section">
                <ClaudeProfilesCard
                  currentIp={currentIp}
                  currentCountry={currentCountry}
                  activeUpstreamKey={state?.upstream_proxy?.raw_key}
                  onProfileSwitched={(_name, pin) => {
                    if (pin) {
                      handlePinIp(pin);
                    } else {
                      runFullAudit(false);
                    }
                  }}
                  onToast={showToast}
                />
              </div>
            </div>
          )}

          {activeTab === "audit" && (
            <div className="animate-in fade-in duration-150">
              <DeepAuditTab
                report={activeReport}
                state={state}
                onNavigateToGuide={(ref) => {
                  if (ref) setInitialGuideTarget(ref);
                  setActiveTab("guide");
                }}
                onSimulate={handleSimulate}
                onPinCurrentIp={handlePinIp}
                onSyncTimezone={() => handleSyncTz()}
                onOpenCrashTest={() => setShowCrashTestModal(true)}
              />
            </div>
          )}

          {activeTab === "guide" && (
            <div className="animate-in fade-in duration-150">
              <GuideViewer
                initialGuide={initialGuideTarget}
                currentIp={currentIp}
                currentTimezone={activeReport?.telemetry?.timezone}
                onPinIp={handlePinIp}
                onToast={showToast}
              />
            </div>
          )}

          {activeTab === "bin" && (
            <div className="animate-in fade-in duration-150">
              <BinCheckerTab
                vpnCountry={currentCountry}
                activeReport={activeReport}
                state={state}
                onStateChange={() => runFullAudit(false)}
                onNavigateToPaymentGuide={() => {
                  setInitialGuideTarget("02-safe-payment-cards");
                  setActiveTab("guide");
                }}
                onToast={showToast}
              />
            </div>
          )}
        </main>
      </div>

      {/* Footer */}
      <footer className="mt-3 pt-2 border-t border-white/[0.07] text-[9.5px] text-zinc-500 flex items-center justify-between">
        <span className="flex items-center gap-1.5">
          <CheckCircle2 className="w-3 h-3 text-emerald-500/80 shrink-0" />
          <span>100% локально · Без чтения чатов</span>
        </span>
        <div className="flex items-center gap-2.5">
          <button
            type="button"
            disabled={checkingUpdate}
            onClick={() => handleCheckUpdates(false)}
            className={`flex items-center gap-1 transition-colors font-medium cursor-pointer hover:underline ${
              updateInfo?.update_available
                ? "text-amber-300 font-semibold"
                : "text-zinc-400 hover:text-zinc-200"
            }`}
            title="Проверить обновления из релизов на GitHub"
          >
            <Download className="w-2.5 h-2.5" />
            <span>{checkingUpdate ? "Проверка..." : updateInfo?.update_available ? `Обновить (v${updateInfo.latest_version})` : "Обновления"}</span>
          </button>
          <button
            type="button"
            onClick={handleOpenSupport}
            className="flex items-center gap-1 text-sky-400/90 hover:text-sky-300 transition-colors font-medium cursor-pointer hover:underline"
            title="Связь и поддержка в Telegram: @nomadnocode"
          >
            <Send className="w-2.5 h-2.5 text-sky-400" />
            <span>@nomadnocode</span>
          </button>
        </div>
      </footer>

      {/* Modals: Settings, Crash-Test, Update, Intercept */}
      <SettingsModal
        isOpen={showSettingsModal}
        onClose={() => setShowSettingsModal(false)}
        autoUpdateEnabled={autoUpdateEnabled}
        onToggleAutoUpdate={handleToggleAutoUpdate}
        interceptionBehavior={state?.interception_behavior}
        popupPosition={state?.popup_position}
        customAiDomains={state?.custom_ai_domains}
        onStateUpdated={() => fetchAppState().then(setState)}
        onToast={(title, msg, type) =>
          showToast(title, msg, type === "warn" ? "warning" : type || "info")
        }
      />

      <VpnCrashTestModal
        isOpen={showCrashTestModal}
        onClose={() => setShowCrashTestModal(false)}
        onToast={showToast}
      />

      {/* Update Modal */}
      {showUpdateModal && updateInfo && (
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="update-dialog-title"
          className="fixed inset-0 z-50 bg-black/80 backdrop-blur-xs flex items-center justify-center p-3 animate-in fade-in duration-150"
        >
          <div className="w-full max-w-[348px] bg-[#12141A] border border-white/[0.12] rounded-xl p-3.5 shadow-2xl space-y-2.5 font-sans">
            <div className="flex items-start justify-between gap-2">
              <div className="flex items-center gap-2">
                <div
                  className={`w-7 h-7 rounded-lg flex items-center justify-center border ${
                    updateInfo.update_available
                      ? "bg-emerald-500/15 border-emerald-500/30 text-emerald-400"
                      : "bg-zinc-900 border-white/[0.08] text-zinc-300"
                  }`}
                >
                  <Download className="w-4 h-4" />
                </div>
                <div>
                  <h2 id="update-dialog-title" className="text-xs font-bold text-white">
                    {updateInfo.update_available
                      ? `Доступно обновление v${updateInfo.latest_version}`
                      : "Установлена актуальная версия"}
                  </h2>
                  <p className="text-[10px] font-mono text-zinc-400">
                    Текущая: v{updateInfo.current_version} · Релиз: v{updateInfo.latest_version}
                  </p>
                </div>
              </div>
              <button
                type="button"
                disabled={installingUpdate}
                onClick={() => setShowUpdateModal(false)}
                className="p-1 rounded-md text-zinc-400 hover:text-white hover:bg-white/[0.06] cursor-pointer"
                title="Закрыть"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            </div>

            <div className="flex items-center justify-between gap-2 pt-1">
              <button
                type="button"
                onClick={() => openExternal(updateInfo.release_url)}
                className="px-2.5 py-1.5 rounded-lg bg-zinc-900 hover:bg-zinc-800 border border-white/[0.08] text-[10.5px] text-zinc-300 flex items-center gap-1 cursor-pointer"
              >
                <span>GitHub Release</span>
                <ExternalLink className="w-3 h-3" />
              </button>

              {updateInfo.update_available ? (
                <button
                  type="button"
                  disabled={installingUpdate}
                  onClick={handleInstallUpdate}
                  className="px-3 py-1.5 rounded-lg bg-emerald-500 hover:bg-emerald-400 disabled:opacity-60 text-zinc-950 font-bold text-[11px] flex items-center gap-1.5 cursor-pointer transition-colors"
                >
                  <Download className="w-3.5 h-3.5" />
                  <span>{installingUpdate ? "Обновление..." : "Обновить"}</span>
                </button>
              ) : (
                <button
                  type="button"
                  onClick={() => setShowUpdateModal(false)}
                  className="px-3 py-1.5 rounded-lg bg-zinc-100 hover:bg-white text-zinc-950 font-semibold text-[11px] cursor-pointer"
                >
                  Понятно
                </button>
              )}
            </div>
          </div>
        </div>
      )}

      {/* In-window Emergency Interception Overlay (browser preview only) */}
      {!isTauri() && interceptPayload && (
        <div className="fixed inset-0 z-50 bg-black/85 backdrop-blur-xs flex items-center justify-center p-2">
          <div className="scale-[0.78] origin-center">
            <AlertPopup
              initialPayload={interceptPayload}
              onResolved={() => {
                setInterceptPayload(null);
                runFullAudit(false);
              }}
            />
          </div>
        </div>
      )}
    </div>
  );
};

export default Dashboard;
