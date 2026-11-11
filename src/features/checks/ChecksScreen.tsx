import { useState } from "react";
import type { AppStateData, RiskReport } from "../../types";
import { Badge, Button, Card, ExpandRow, Link, Mono, Row, Rows, Section, Skeleton } from "../../ui";
import type { IconName, Tone } from "../../ui";
import { status as words } from "../../shared/copy/ru";
import { checks as copy } from "../../shared/copy/ru";
import type { CheckId } from "../../shared/copy/ru";
import { buildChecks, isCriticalStatus } from "./checksModel";
import type { CheckItem } from "./checksModel";
import "./checks.css";

export interface ChecksScreenProps {
  report: RiskReport | null;
  state: AppStateData | null;
  onNavigateToGuide: (guideId?: string) => void;
  onSimulate: () => void;
  onPinCurrentIp: (ip: string) => void;
  onSyncTimezone: () => void;
  onOpenCrashTest: () => void;
  /** Повторный аудит сети. Без обработчика кнопка «Перепроверить» не показывается. */
  onRecheck?: () => void;
  /** Идёт повторный аудит. */
  rechecking?: boolean;
}

/** Значок строки: только украшение, статус говорит бейдж. */
const ICONS: Record<CheckId, IconName> = {
  geo_matrix: "globe",
  asn: "pulse",
  dnsbl: "shield",
  reverse_dns: "term",
  pinning: "lock",
  travel: "arrow",
  ipv6: "zap",
  dns: "pulse",
  dns_country: "globe",
  wifi_geo: "shield",
  timezone: "refresh",
  locale_hygiene: "term",
};

const LEAK_CHECKS: CheckId[] = ["ipv6", "dns", "dns_country", "wifi_geo"];

interface StatusView {
  tone: Tone | "neutral";
  word: string;
  icon?: IconName;
}

/** Единый словарь: Чисто / Нужна проверка / Утечка / Выключено. */
function statusOf(item: CheckItem, state: AppStateData | null): StatusView {
  if (item.ok) return { tone: "ok", word: words.clean, icon: "check" };
  if (isCriticalStatus(item.result)) {
    return { tone: "danger", word: LEAK_CHECKS.includes(item.id) ? words.leak : words.review, icon: "alert" };
  }
  if (item.id === "pinning" && !state?.pinned_ip) return { tone: "neutral", word: words.off };
  return { tone: "warn", word: words.review, icon: "alert" };
}

/** Экран «Проверки»: вердикт, 12 проверок по смыслу, тесты защиты. */
export function ChecksScreen({
  report,
  state,
  onNavigateToGuide,
  onSimulate,
  onPinCurrentIp,
  onSyncTimezone,
  onOpenCrashTest,
  onRecheck,
  rechecking,
}: ChecksScreenProps) {
  // Явный выбор пользователя; без него раскрыты только непройденные проверки.
  const [expanded, setExpanded] = useState<Partial<Record<CheckId, boolean>>>({});

  if (!report) {
    return (
      <div className="zs-stack" role="status" aria-live="polite">
        <Card>
          <h2 className="zs-verdict__title">{copy.loadingTitle}</h2>
          <p className="zs-verdict__text">{copy.loadingText}</p>
        </Card>
        <Card>
          <Skeleton variant="text" lines={8} />
        </Card>
      </div>
    );
  }

  const { groups, items } = buildChecks(report, state);
  const passed = items.filter((i) => i.ok).length;
  const failed = items.length - passed;
  const worst: Tone = items.some((i) => !i.ok && isCriticalStatus(i.result)) ? "danger" : failed > 0 ? "warn" : "ok";
  const leak = items.some((i) => !i.ok && LEAK_CHECKS.includes(i.id) && isCriticalStatus(i.result));

  const runAction = (item: CheckItem) => {
    if (item.action === "pin") onPinCurrentIp(report.telemetry.ip);
    else if (item.action === "sync-timezone") onSyncTimezone();
    else if (item.action === "guide") onNavigateToGuide(item.result.guide_ref || undefined);
  };

  return (
    <div className="zs-stack">
      <Card>
        <div className="zs-verdict__head">
          <div className="zs-verdict__body">
            {worst === "ok" ? (
              <Badge tone="ok" icon="check">
                {words.clean}
              </Badge>
            ) : (
              <Badge tone={worst} icon="alert">
                {leak ? words.leak : words.review}
              </Badge>
            )}
            <h2 className="zs-verdict__title">{copy.verdict(passed, items.length)}</h2>
            <p className="zs-verdict__text">{failed === 0 ? copy.verdictOk : copy.verdictIssues(failed)}</p>
          </div>
          {onRecheck && (
            <div className="zs-verdict__action">
              <Button icon="refresh" onClick={onRecheck} disabled={rechecking}>
                {rechecking ? copy.rechecking : copy.recheck}
              </Button>
            </div>
          )}
        </div>
      </Card>

      {groups.map((group) => (
        <Section key={group.id} title={group.title}>
          <Card>
            <Rows>
              {group.items.map((item) => (
                <CheckRow
                  key={item.id}
                  item={item}
                  view={statusOf(item, state)}
                  open={expanded[item.id] ?? !item.ok}
                  onToggle={() => setExpanded((prev) => ({ ...prev, [item.id]: !(prev[item.id] ?? !item.ok) }))}
                  onAction={() => runAction(item)}
                  onGuide={() => onNavigateToGuide(item.result.guide_ref || undefined)}
                />
              ))}
            </Rows>
          </Card>
        </Section>
      ))}

      <Section title={copy.tests.title}>
        <Card>
          <Rows>
            <Row icon="pulse" title={copy.tests.crashTitle} sub={copy.tests.crashSub} onClick={onOpenCrashTest} />
            <Row icon="shield" title={copy.tests.demoTitle} sub={copy.tests.demoSub} onClick={onSimulate} />
          </Rows>
        </Card>
      </Section>
    </div>
  );
}

/* ---------- Строка проверки: ExpandRow с подробностями ---------- */

interface CheckRowProps {
  item: CheckItem;
  view: StatusView;
  open: boolean;
  onToggle: () => void;
  onAction: () => void;
  onGuide: () => void;
}

function CheckRow({ item, view, open, onToggle, onAction, onGuide }: CheckRowProps) {
  const actionLabel =
    item.action === "pin" ? copy.actions.pin : item.action === "sync-timezone" ? copy.actions.syncTimezone : copy.actions.openGuide;
  // Инструкция отдельной ссылкой — только если главное действие другое.
  const showGuideLink = item.result.guide_ref && item.action !== "guide";

  return (
    <ExpandRow
      icon={ICONS[item.id]}
      tone={view.tone === "ok" || view.tone === "warn" || view.tone === "danger" ? view.tone : "neutral"}
      title={item.title}
      sub={item.what}
      trail={
        <Badge tone={view.tone} icon={view.icon}>
          {view.word}
        </Badge>
      }
      open={open}
      onToggle={onToggle}
      detailLabel={item.title}
    >
      <div>
        <p className="zs-check__label">{copy.detail.result}</p>
        <p className="zs-check__message">{item.result.message}</p>
      </div>
      {item.data.length > 0 && (
        <dl className="zs-check__data">
          {item.data.map((d) => (
            <div key={d.label} className="zs-check__pair">
              <dt>{d.label}</dt>
              <dd>{d.mono ? <Mono>{d.value}</Mono> : d.value}</dd>
            </div>
          ))}
        </dl>
      )}
      {(item.action || showGuideLink) && (
        <div className="zs-check__actions">
          {item.action && (
            <Button size="sm" icon={item.action === "guide" ? undefined : item.action === "pin" ? "lock" : "refresh"} onClick={onAction}>
              {actionLabel}
            </Button>
          )}
          {showGuideLink && (
            <Link
              href="#"
              chevron
              onClick={(e) => {
                e.preventDefault();
                onGuide();
              }}
            >
              {copy.actions.openGuide}
            </Link>
          )}
        </div>
      )}
    </ExpandRow>
  );
}
