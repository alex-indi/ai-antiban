import type { AppStateData, CheckResult, RiskReport } from "../../types";
import { checks as copy } from "../../shared/copy/ru";
import type { CheckGroupId, CheckId } from "../../shared/copy/ru";

/** Что можно сделать с непройденной проверкой. */
export type CheckAction = "pin" | "sync-timezone" | "guide";

/** Пара «название — значение» второго уровня. mono — только для IP, доменов и кодов. */
export interface CheckDatum {
  label: string;
  value: string;
  mono: boolean;
}

export interface CheckItem {
  id: CheckId;
  group: CheckGroupId;
  title: string;
  what: string;
  result: CheckResult;
  ok: boolean;
  data: CheckDatum[];
  /** Главное действие непройденной проверки. */
  action: CheckAction | null;
}

export interface CheckGroup {
  id: CheckGroupId;
  title: string;
  items: CheckItem[];
}

export const isOkStatus = (c: CheckResult) => c.status === "ok" || c.status === "green";
export const isCriticalStatus = (c: CheckResult) => c.status === "critical" || c.status === "red";

/** Порядок задан шлюзом: report.checks всегда содержит 12 проверок в этом порядке. */
const ORDER: { id: CheckId; group: CheckGroupId; index: number }[] = [
  { id: "geo_matrix", group: "address", index: 0 },
  { id: "asn", group: "address", index: 4 },
  { id: "dnsbl", group: "address", index: 3 },
  { id: "reverse_dns", group: "address", index: 9 },
  { id: "pinning", group: "stability", index: 1 },
  { id: "travel", group: "stability", index: 2 },
  { id: "ipv6", group: "leaks", index: 5 },
  { id: "dns", group: "leaks", index: 6 },
  { id: "dns_country", group: "leaks", index: 10 },
  { id: "wifi_geo", group: "leaks", index: 8 },
  { id: "timezone", group: "environment", index: 7 },
  { id: "locale_hygiene", group: "environment", index: 11 },
];

const GROUP_ORDER: CheckGroupId[] = ["address", "stability", "leaks", "environment"];

/** Результат по месту в report.checks; для первых проверок страхуемся именованными полями отчёта. */
function resultAt(report: RiskReport, id: CheckId, index: number): CheckResult | null {
  const named: Partial<Record<CheckId, CheckResult>> = {
    geo_matrix: report.geo_result,
    pinning: report.pinning_result,
    travel: report.travel_result,
    dnsbl: report.dnsbl_result,
    asn: report.asn_result,
    ipv6: report.ipv6_result,
    timezone: report.timezone_result,
  };
  return named[id] ?? report.checks?.[index] ?? null;
}

function dataFor(id: CheckId, report: RiskReport, state: AppStateData | null): CheckDatum[] {
  const t = report.telemetry;
  const d = copy.data;
  const out: CheckDatum[] = [];
  const add = (label: string, value: string | null | undefined, mono: boolean) => {
    if (value) out.push({ label, value, mono });
  };
  switch (id) {
    case "geo_matrix":
      add(d.country, t.country_code, true);
      add(d.ip, t.ip, true);
      break;
    case "asn":
      add(d.asn, t.asn ? `AS${t.asn}` : null, true);
      add(d.provider, t.isp || t.org, false);
      break;
    case "dnsbl":
      add(d.result, report.dnsbl_clean_count, false);
      add(d.bases, "Spamhaus, SORBS, Barracuda, DroneBL", true);
      break;
    case "reverse_dns":
      add(d.ip, t.ip, true);
      break;
    case "pinning":
      add(d.pinnedIp, state?.pinned_ip ?? null, true);
      if (!state?.pinned_ip) out.push({ label: d.pinnedIp, value: d.notPinned, mono: false });
      add(d.mode, state?.strict_pinning_enabled ? d.strict : d.notify, false);
      break;
    case "travel":
      add(d.country, t.country_code, true);
      break;
    case "timezone":
      add(d.zoneIp, t.timezone, true);
      add(d.zoneAi, state?.synced_ai_timezone, true);
      break;
    case "locale_hygiene":
      add(d.zoneAi, state?.synced_ai_timezone, true);
      break;
    default:
      break;
  }
  return out;
}

function actionFor(id: CheckId, result: CheckResult, ok: boolean): CheckAction | null {
  if (ok) return null;
  if (id === "pinning") return "pin";
  if (id === "timezone" || id === "locale_hygiene") return "sync-timezone";
  return result.guide_ref ? "guide" : null;
}

/** Двенадцать проверок, сгруппированные по смыслу. Без побочных эффектов. */
export function buildChecks(report: RiskReport, state: AppStateData | null): { groups: CheckGroup[]; items: CheckItem[] } {
  const items: CheckItem[] = [];
  for (const spec of ORDER) {
    const result = resultAt(report, spec.id, spec.index);
    if (!result) continue;
    const ok = isOkStatus(result);
    items.push({
      id: spec.id,
      group: spec.group,
      title: copy.items[spec.id].title,
      what: copy.items[spec.id].what,
      result,
      ok,
      data: dataFor(spec.id, report, state),
      action: actionFor(spec.id, result, ok),
    });
  }
  const groups = GROUP_ORDER.map((id) => ({
    id,
    title: copy.groups[id],
    items: items.filter((i) => i.group === id),
  })).filter((g) => g.items.length > 0);
  return { groups, items };
}
