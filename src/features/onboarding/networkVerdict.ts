import type { CheckResult, ProviderId, RiskReport } from "../../types";
import type { HeroTone } from "../../ui";
import { countryName } from "../../shared/lib/country";

/** Главная причина, по которой сеть не годится. Каждой соответствует человеческий текст в copy/onboarding.ts. */
export type NetworkReason = "noVpn" | "restricted" | "leak" | "spam" | "shared" | "travel" | "timezone" | "other";

export interface NetworkVerdict {
  /** Сеть годится: можно идти дальше без объяснений. */
  good: boolean;
  tone: HeroTone;
  /** Ключ вердикта в copy.network.verdict. */
  verdict: "ok" | "attention" | "bad";
  passed: number;
  total: number;
  reason: NetworkReason | null;
  /** Страна адреса словом (для текста о стране), пусто если неизвестна. */
  country: string;
}

const PROVIDERS: ProviderId[] = ["anthropic", "openai", "gemini"];

const isOk = (c: CheckResult) => c.status === "ok" || c.status === "green";

function checksOf(report: RiskReport | null | undefined): CheckResult[] {
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

/** Страны, из которых ИИ-сервисы недоступны: для них «нет VPN» понятнее, чем «страна с ограничениями». */
const NO_SERVICE_COUNTRIES = ["RU", "BY"];

/**
 * Вердикт по сети для шага «Проверка» — без жаргона и без побочных эффектов.
 * «Годится» считается так же, как вердикт «Обзора»: ни один сервис не красный и не жёлтый, нет утечки IPv6,
 * часовой пояс совпадает, спам-базы чистые. Проверка закрепления IP не мешает: это рекомендация.
 */
export function evaluateNetwork(
  reports: Partial<Record<ProviderId, RiskReport>>,
  activeReport: RiskReport | null,
): NetworkVerdict {
  const anyRed = PROVIDERS.some((id) => reports[id]?.overall_status === "red");
  const anyYellow = PROVIDERS.some((id) => reports[id]?.overall_status === "yellow");
  const bad = (s?: string) => s === "warning" || s === "critical" || s === "yellow" || s === "red";
  const ipv6Leak = bad(activeReport?.ipv6_result?.status);
  const tzMismatch = bad(activeReport?.timezone_result?.status);
  const dnsblBad = bad(activeReport?.dnsbl_result?.status);

  const checks = checksOf(activeReport);
  const passed = checks.filter(isOk).length;
  const country = activeReport?.telemetry?.country_code ?? "";

  const good = !anyRed && !anyYellow && !ipv6Leak && !tzMismatch && !dnsblBad;
  if (good) {
    return { good, tone: "ok", verdict: "ok", passed, total: checks.length, reason: null, country: countryName(country) };
  }

  // Все не зелёные проверки по всем сервисам: у Gemini и Claude география может различаться.
  const problems: CheckResult[] = [];
  for (const report of [activeReport, ...PROVIDERS.map((id) => reports[id])]) {
    for (const c of checksOf(report)) if (!isOk(c) && !problems.includes(c)) problems.push(c);
  }
  const has = (test: (code: string) => boolean) => problems.some((c) => test(c.code));

  let reason: NetworkReason = "other";
  if (has((c) => c.includes("GEO") || c === "GLOBAL_SANCTIONED_COUNTRY" || c.includes("TRANSIT"))) {
    reason = NO_SERVICE_COUNTRIES.includes(country.toUpperCase()) ? "noVpn" : "restricted";
  } else if (NO_SERVICE_COUNTRIES.includes(country.toUpperCase()) && anyRed) {
    reason = "noVpn";
  } else if (has((c) => c.startsWith("IPV6_LEAK") || c === "DNS_YANDEX_LEAK" || c === "DNS_DOMESTIC_WARNING")) {
    reason = "leak";
  } else if (has((c) => c.startsWith("DNSBL_LISTED"))) {
    reason = "spam";
  } else if (has((c) => c === "HIGH_RISK_MASS_VPN_ASN" || c === "REVERSE_DNS_ANONYMIZER_MARKER")) {
    reason = "shared";
  } else if (has((c) => c.startsWith("IMPOSSIBLE_TRAVEL"))) {
    reason = "travel";
  } else if (has((c) => c.startsWith("TIMEZONE_"))) {
    reason = "timezone";
  }

  return {
    good,
    tone: anyRed ? "danger" : "warn",
    verdict: anyRed ? "bad" : "attention",
    passed,
    total: checks.length,
    reason,
    country: countryName(country),
  };
}
