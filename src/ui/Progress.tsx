import { cx } from "./cx";

export interface ProgressProps {
  /** 0–100. Без значения — неопределённый (бегущая полоса). */
  value?: number;
  /** Подпись: что происходит. Показывается над полосой и читается скринридером. */
  label: string;
  /** Показывать процент справа (только для определённого). */
  showValue?: boolean;
  tone?: "brand" | "warn" | "danger";
  className?: string;
}

/** Линейный индикатор: определённый (aria-valuenow) и неопределённый. Процент или подпись всегда рядом. */
export function Progress({ value, label, showValue = true, tone = "brand", className }: ProgressProps) {
  const determinate = value !== undefined;
  const v = determinate ? Math.max(0, Math.min(100, Math.round(value))) : 0;
  return (
    <div className={cx("zs-progress", tone !== "brand" && `zs-progress--${tone}`, !determinate && "zs-progress--indeterminate", className)}>
      <div className="zs-progress__head">
        <span>{label}</span>
        {determinate && showValue && <span className="zs-mono">{v}%</span>}
      </div>
      <div
        className="zs-progress__track"
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={determinate ? v : undefined}
      >
        <div className="zs-progress__bar" style={determinate ? { width: `${v}%` } : undefined} />
      </div>
    </div>
  );
}
