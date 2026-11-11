import type { CSSProperties } from "react";
import { cx } from "./cx";

export interface SkeletonProps {
  variant?: "text" | "block" | "circle";
  /** Ширина: число — px, строка — любое CSS-значение («60%»). */
  width?: number | string;
  /** Высота: для text по умолчанию 16px (строка 14px), для block 64px, для circle равна ширине. */
  height?: number | string;
  /** Для text: число строк; последняя короче. */
  lines?: number;
  className?: string;
}

/** Заглушка на время загрузки: пульсация без градиентов, скрыта от скринридера. */
export function Skeleton({ variant = "text", width, height, lines = 1, className }: SkeletonProps) {
  if (variant === "text" && lines > 1) {
    return (
      <div className={cx("zs-skeleton-group", className)} aria-hidden="true">
        {Array.from({ length: lines }, (_, i) => (
          <Skeleton key={i} variant="text" width={i === lines - 1 ? "60%" : width} height={height} />
        ))}
      </div>
    );
  }
  const style: CSSProperties = { width };
  if (variant === "circle") style.height = height ?? width ?? 32;
  else if (height !== undefined) style.height = height;
  if (variant === "circle" && width === undefined) style.width = 32;
  return <span aria-hidden="true" className={cx("zs-skeleton", `zs-skeleton--${variant}`, className)} style={style} />;
}
