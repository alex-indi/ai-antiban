import type { ReactNode } from "react";
import { cx } from "./cx";

export interface StatProps {
  /** Число стилем metric. */
  value: ReactNode;
  /** Подпись до четырёх слов; переносится на вторую строку. */
  label: ReactNode;
  /** Красится только когда значение требует внимания: «0 утечек» остаётся нейтральным. */
  tone?: "neutral" | "ok" | "danger";
  /** Слой подложки: 2 (bg-2) внутри карточки, 1 (bg-1) прямо на фоне окна. */
  layer?: 1 | 2;
  className?: string;
}

/** Плитка метрики. */
export function Stat({ value, label, tone = "neutral", layer = 2, className }: StatProps) {
  return (
    <div className={cx("zs-stat", tone !== "neutral" && `zs-stat--${tone}`, layer === 1 && "zs-stat--l1", className)}>
      <span className="zs-stat__value">{value}</span>
      <span className="zs-stat__label">{label}</span>
    </div>
  );
}

/** Ряд из трёх плиток. */
export function Stats({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={cx("zs-stats", className)}>{children}</div>;
}
