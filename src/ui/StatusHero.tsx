import type { ReactNode } from "react";
import { cx } from "./cx";
import { Card } from "./Card";

export type HeroTone = "ok" | "warn" | "danger";

/** Длина окружности кольца: r = 27 в viewBox 64. */
const RING_LENGTH = 2 * Math.PI * 27;

/** Тон по доверию: пороги прежнего TrustRing (меньше 36 — опасно, меньше 86 — внимание). */
export function toneForScore(score: number): HeroTone {
  if (score < 36) return "danger";
  if (score < 86) return "warn";
  return "ok";
}

export interface TrustRingProps {
  /** 0–100. */
  score: number;
  tone?: HeroTone;
  className?: string;
}

/** Кольцо доверия 64px: трек bg-3, дуга brand / warn / danger, число моноширинным. */
export function TrustRing({ score, tone, className }: TrustRingProps) {
  const value = Math.max(0, Math.min(100, Math.round(score)));
  const t = tone ?? toneForScore(value);
  const dash = ((value / 100) * RING_LENGTH).toFixed(1);
  return (
    <div
      className={cx("zs-ring", `zs-ring--${t}`, className)}
      role="img"
      aria-label={`Доверие: ${value} из 100`}
    >
      <svg viewBox="0 0 64 64" aria-hidden="true">
        <circle className="zs-ring__track" cx="32" cy="32" r="27" />
        <circle
          className="zs-ring__value"
          cx="32"
          cy="32"
          r="27"
          strokeDasharray={`${dash} ${RING_LENGTH.toFixed(1)}`}
        />
      </svg>
      <span className="zs-ring__num" aria-hidden="true">
        {value}
      </span>
    </div>
  );
}

export interface StatusHeroProps {
  /** Доверие 0–100. */
  score: number;
  /** По умолчанию определяется по score. */
  tone?: HeroTone;
  /** Заголовок-вердикт: «Вы защищены», «Доступ к ИИ остановлен». */
  title: ReactNode;
  /** IP — выводится моноширинным. */
  ip?: string;
  /** Страна словом. */
  country?: string;
  /** До трёх фактов бейджами (Badge). */
  facts?: ReactNode;
  /** Ниже фактов внутри той же карточки: например, строка с тумблером защиты. */
  children?: ReactNode;
  className?: string;
}

/** Главный вердикт окна: кольцо доверия, заголовок, IP и страна, факты. Один на окно. */
export function StatusHero({ score, tone, title, ip, country, facts, children, className }: StatusHeroProps) {
  const t = tone ?? toneForScore(score);
  return (
    <Card className={className}>
      <div className={cx("zs-hero", t !== "ok" && `zs-hero--${t}`)}>
        <TrustRing score={score} tone={t} />
        <div className="zs-hero__body">
          <h2 className="zs-hero__title">{title}</h2>
          {(ip || country) && (
            <p className="zs-hero__meta">
              {ip && <span className="zs-mono">{ip}</span>}
              {ip && country && " · "}
              {country}
            </p>
          )}
        </div>
      </div>
      {facts && <div className="zs-hero__facts">{facts}</div>}
      {children}
    </Card>
  );
}
