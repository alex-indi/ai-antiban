import { useCallback, useEffect, useState } from "react";
import type { AppStateData, ProviderId, RiskReport } from "../../types";
import { auditConnection, fetchAppState, getSysproxyStatus } from "../../utils/api";
import { toast as t } from "../copy/ru";
import type { ShowToast } from "./useToasts";

interface Options {
  showToast: ShowToast;
  refreshGatewayStats: () => Promise<void>;
}

/**
 * Состояние шлюза: AppStateData, отчёты аудита по трём сервисам, статус системного прокси, задержка проверки.
 * Здесь же стартовый аудит и «самолечение» — повторный опрос, пока не определились IP и провайдер.
 */
export function useGatewayState({ showToast, refreshGatewayStats }: Options) {
  const [state, setState] = useState<AppStateData | null>(null);
  const [reports, setReports] = useState<Partial<Record<ProviderId, RiskReport>>>({});
  const [refreshing, setRefreshing] = useState<boolean>(false);
  const [scanLatencyMs, setScanLatencyMs] = useState<number>(38);
  const [sysproxyActive, setSysproxyActive] = useState<boolean>(true);
  const [autoUpdateEnabled, setAutoUpdateEnabled] = useState<boolean>(true);
  /** Текст последней ошибки аудита; сбрасывается при успешном проходе. Нужен экрану «нет сети». */
  const [auditError, setAuditError] = useState<string | null>(null);

  /** Перечитать AppStateData без аудита (тихо: ошибки только в консоль). */
  const refreshState = useCallback(async () => {
    const st = await fetchAppState();
    setState(st);
    return st;
  }, []);

  const runFullAudit = useCallback(
    async (force: boolean = false, silentError: boolean = false) => {
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

        // Основной отчёт сразу: IP, провайдер и проверки рисуются, не дожидаясь остальных сервисов.
        setReports((prev) => ({
          ...prev,
          anthropic: claudeReport,
        }));
        setAuditError(null);

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
        setAuditError(err?.message || String(err));
        if (!silentError) {
          showToast(t.auditError, err?.message || String(err), "error");
        }
      } finally {
        setRefreshing(false);
        refreshGatewayStats();
      }
    },
    [showToast, refreshGatewayStats],
  );

  // Стартовый аудит.
  useEffect(() => {
    runFullAudit(true, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const activeReport = reports.anthropic || reports.openai || reports.gemini || null;

  // Самолечение на старте: если первый аудит упёрся в холодный старт сети/PAC и вернулся без IP или провайдера,
  // перепроверяем сами, до шести раз с шагом 1,5 с.
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    activeReport?.telemetry?.ip,
    activeReport?.telemetry?.asn,
    activeReport?.telemetry?.isp,
    activeReport?.telemetry?.org,
  ]);

  const currentIp = activeReport?.telemetry.ip || "--";
  const currentCountry = activeReport?.telemetry.country_code || "--";
  const trustScore = activeReport?.trust_score ?? 100;

  return {
    state,
    setState,
    reports,
    refreshing,
    scanLatencyMs,
    sysproxyActive,
    setSysproxyActive,
    autoUpdateEnabled,
    setAutoUpdateEnabled,
    auditError,
    activeReport,
    currentIp,
    currentCountry,
    trustScore,
    refreshState,
    runFullAudit,
  };
}

export type GatewayState = ReturnType<typeof useGatewayState>;
