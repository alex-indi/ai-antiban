import type { ReactNode } from "react";
import { Icon } from "./Icon";
import type { IconName } from "./Icon";
import { cx } from "./cx";
import type { Tone } from "./Badge";

export type BannerTone = Tone | "neutral";

const DEFAULT_ICON: Record<BannerTone, IconName> = {
  neutral: "info",
  ok: "shield",
  warn: "alert",
  danger: "alert",
  info: "info",
};

export interface BannerProps {
  tone?: BannerTone;
  /** По умолчанию иконка по тону. */
  icon?: IconName;
  /** Заголовок-факт в одну строку. */
  title: ReactNode;
  /** Одно предложение пояснения: что случилось и что делать. */
  children?: ReactNode;
  /** Не более одной ссылки-действия. */
  action?: ReactNode;
  /** По умолчанию status; для срочного (danger) можно передать alert. */
  role?: "status" | "alert";
  className?: string;
}

/** Сообщение на подложке тона. Без рамки и свечения; один на экран. */
export function Banner({ tone = "neutral", icon, title, children, action, role = "status", className }: BannerProps) {
  return (
    <div role={role} className={cx("zs-banner", tone !== "neutral" && `zs-banner--${tone}`, className)}>
      <Icon name={icon ?? DEFAULT_ICON[tone]} />
      <div className="zs-banner__body">
        <p className="zs-banner__title">{title}</p>
        {children && <p className="zs-banner__text">{children}</p>}
        {action && <div className="zs-banner__action">{action}</div>}
      </div>
    </div>
  );
}
