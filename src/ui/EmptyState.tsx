import type { ReactNode } from "react";
import { Icon } from "./Icon";
import type { IconName } from "./Icon";
import { cx } from "./cx";

export interface EmptyStateProps {
  icon: IconName;
  title: string;
  /** Пояснение: почему пусто и что делать. */
  children?: ReactNode;
  /** Одно действие — обычно Button. */
  action?: ReactNode;
  className?: string;
}

/** Пустое состояние: иконка, заголовок, пояснение, одно действие. */
export function EmptyState({ icon, title, children, action, className }: EmptyStateProps) {
  return (
    <div className={cx("zs-empty", className)}>
      <span className="zs-empty__icon">
        <Icon name={icon} size={20} />
      </span>
      <h3 className="zs-empty__title">{title}</h3>
      {children && <p className="zs-empty__text">{children}</p>}
      {action && <div className="zs-empty__action">{action}</div>}
    </div>
  );
}
