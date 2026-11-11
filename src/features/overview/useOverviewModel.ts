import { useMemo } from "react";
import type { AppStateData, CheckResult, ProviderId, RiskReport, UpdateCheckResult } from "../../types";
import { checksWord } from "../../shared/lib/format";
import { overview as copy } from "../../shared/copy/ru";
import type { HeroTone } from "../../ui";

export type ServiceStatus = "ok" | "allowed" | "warn" | "critical" | "missing" | "checking";

export interface ServiceModel {
  id: ProviderId;
  status: ServiceStatus;
}

export type BannerKind = "stopped" | "frozen" | "leak" | "timezone" | "review" | "update";
export type PinState = "pinned" | "changed" | "none";

export interface OverviewModel {
  verdict: string;
  tone: HeroTone;
  checksPassed: number;
  checksTotal: number;
  checksLabel: string;
  /** Имя провайдера для бейджа (isp или org); без жаргона вроде ASN. */
  providerName: string | null;
  pinState: PinState;
  banner: BannerKind | null;
  services: ServiceModel[];
}

const PROVIDER_IDS: ProviderId[] = ["anthropic", "openai", "gemini"];

const isOk = (c: CheckResult) => c.status === "ok" || c.status === "green";

function checksOf(report: RiskReport | null): CheckResult[] {
  if (!report) return [];
  if (report.checks && report.checks.length > 0) return report.checks;
  return [
    report.geo_result,
    report.pinning_result,
    report.travel_result,
    report.dnsbl_result,
    report.asn_result,
    report.ipv6_result,
    report.timezone_result,
  ];
}

interface Input {
  reports: Partial<Record<ProviderId, RiskReport>>;
  activeReport: RiskReport | null;
  state: AppStateData | null;
  currentIp: string;
  sysproxyActive: boolean;
  killswitchFrozen: boolean;
  updateInfo: UpdateCheckResult | null;
  /** Идёт аудит: у сервиса без отчёта показываем «Проверяем», а не «Нужна проверка». */
  refreshing: boolean;
}

/** Вывод показателей «Обзора» из сырых данных: вердикт, бейджи, баннер и статусы сервисов. Тут нет побочных эффектов. */
export function useOverviewModel({
  reports,
  activeReport,
  state,
  currentIp,
  sysproxyActive,
  killswitchFrozen,
  updateInfo,
  refreshing,
}: Input): OverviewModel {
  return useMemo(() => {
    const anyRed = PROVIDER_IDS.some((id) => reports[id]?.overall_status === "red");
    const anyYellow = PROVIDER_IDS.some((id) => reports[id]?.overall_status === "yellow");

    const ipv6Leak =
      activeReport?.ipv6_result?.status === "critical" ||
      activeReport?.ipv6_result?.status === "red" ||
      activeReport?.ipv6_result?.status === "warning";
    const tzMismatch =
      activeReport?.timezone_result?.status === "warning" ||
      activeReport?.timezone_result?.status === "critical" ||
      activeReport?.timezone_result?.status === "yellow" ||
      activeReport?.timezone_result?.status === "red";
    const dnsblBad =
      activeReport?.dnsbl_result?.status === "critical" || activeReport?.dnsbl_result?.status === "warning";

    let verdict: string = copy.verdict.ok;
    let tone: HeroTone = "ok";
    if (killswitchFrozen) {
      verdict = copy.verdict.frozen;
      tone = "danger";
    } else if (anyRed) {
      verdict = copy.verdict.stopped;
      tone = "danger";
    } else if (!sysproxyActive) {
      verdict = copy.verdict.off;
      tone = "warn";
    } else if (anyYellow || ipv6Leak || tzMismatch) {
      verdict = copy.verdict.attention;
      tone = "warn";
    }

    let banner: BannerKind | null = null;
    if (killswitchFrozen) banner = "frozen";
    else if (anyRed) banner = "stopped";
    else if (ipv6Leak) banner = "leak";
    else if (tzMismatch) banner = "timezone";
    else if (anyYellow || dnsblBad) banner = "review";
    else if (updateInfo?.update_available) banner = "update";

    const checks = checksOf(activeReport);
    const checksPassed = checks.filter(isOk).length;
    const checksTotal = checks.length;

    const pinned = state?.pinned_ip ?? null;
    const pinState: PinState = pinned ? (pinned === currentIp ? "pinned" : "changed") : "none";

    const providerName = activeReport?.telemetry?.isp || activeReport?.telemetry?.org || null;

    const services: ServiceModel[] = PROVIDER_IDS.map((id) => {
      const report = reports[id];
      const isBlocked = state?.blocked_providers?.includes(id);
      const isAllowed = state?.session_allowed_providers?.includes(id);
      const list = checksOf(report ?? null);
      const hasCritical = list.some((c) => c.status === "critical");
      const hasWarning = list.some((c) => c.status === "warning");

      let status: ServiceStatus = "ok";
      if (isBlocked) status = "critical";
      else if (isAllowed) status = "allowed";
      else if (!report) status = refreshing ? "checking" : "missing";
      else if (report.overall_status === "red" || hasCritical) status = "critical";
      else if (report.overall_status === "yellow" || hasWarning) status = "warn";
      return { id, status };
    });

    return {
      verdict,
      tone,
      checksPassed,
      checksTotal,
      checksLabel: copy.facts.checks(checksPassed, checksTotal, checksWord(checksTotal)),
      providerName,
      pinState,
      banner,
      services,
    };
  }, [reports, activeReport, state, currentIp, sysproxyActive, killswitchFrozen, updateInfo, refreshing]);
}
