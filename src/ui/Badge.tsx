import type { HTMLAttributes, ReactNode } from "react";
import { Icon } from "./Icon";
import type { IconName } from "./Icon";
import { cx } from "./cx";

export type Tone = "ok" | "warn" | "danger" | "info";
export type BadgeTone = Tone | "neutral";

export interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  tone?: BadgeTone;
  /** Иконка слева: для warn и danger обязательна по правилам ДС — статус не только цветом. */
  icon?: IconName;
  /** Моноширинный шрифт для технических значений (ASN, код страны). */
  mono?: boolean;
  children: ReactNode;
}

/** Короткий статус: тон + слово из единого словаря («Защищено», «Утечка», «Выключено»…). */
export function Badge({ tone = "neutral", icon, mono, className, children, ...rest }: BadgeProps) {
  return (
    <span
      {...rest}
      className={cx("zs-badge", tone !== "neutral" && `zs-badge--${tone}`, mono && "zs-badge--mono", className)}
    >
      {icon && <Icon name={icon} />}
      {children}
    </span>
  );
}
