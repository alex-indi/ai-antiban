import type { ElementType, ReactNode } from "react";
import { cx } from "./cx";

export interface CardProps {
  /** Заголовок карточки. Если метка раздела стоит над карточкой (Section), заголовок не нужен. */
  title?: ReactNode;
  /** Пометка справа от заголовка. */
  aside?: ReactNode;
  children?: ReactNode;
  as?: ElementType;
  className?: string;
  "aria-label"?: string;
}

/** Контейнер раздела: bg-1, radius-lg, без рамки и тени. Карточки не вкладываются. */
export function Card({ title, aside, children, as: Tag = "section", className, ...rest }: CardProps) {
  return (
    <Tag className={cx("zs-card", className)} {...rest}>
      {(title || aside) && (
        <div className="zs-card__head">
          {title && <h3 className="zs-card__title">{title}</h3>}
          {aside && <span className="zs-card__aside">{aside}</span>}
        </div>
      )}
      {children}
    </Tag>
  );
}
