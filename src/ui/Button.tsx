import { forwardRef } from "react";
import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Icon } from "./Icon";
import type { IconName } from "./Icon";
import { cx } from "./cx";

export type ButtonVariant = "secondary" | "primary" | "quiet" | "danger" | "danger-solid";
export type ButtonSize = "sm" | "md" | "lg";

type ButtonBase = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> & {
  /** primary — одна на экран; danger — тихая опасная; danger-solid — только подтверждение необратимого. */
  variant?: ButtonVariant;
  /** sm 28px, md 36px, lg 44px. */
  size?: ButtonSize;
  /** На всю ширину контейнера. */
  block?: boolean;
  /** Иконка слева. */
  icon?: IconName;
  /** Горячая клавиша справа (zs-kbd), например «1». */
  kbd?: string;
};

export type ButtonProps = ButtonBase &
  (
    | { iconOnly?: false; children: ReactNode }
    | { iconOnly: true; icon: IconName; "aria-label": string; children?: never }
  );

const VARIANT_CLASS: Record<ButtonVariant, string | false> = {
  secondary: false,
  primary: "zs-btn--primary",
  quiet: "zs-btn--quiet",
  danger: "zs-btn--danger",
  "danger-solid": "zs-btn--danger-solid",
};

/** Кнопка действия. Подпись — глагол и объект; иконочная кнопка требует aria-label. */
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(props, ref) {
  const {
    variant = "secondary",
    size = "md",
    block,
    icon,
    kbd,
    iconOnly,
    className,
    type = "button",
    children,
    ...rest
  } = props;
  return (
    <button
      ref={ref}
      type={type}
      aria-keyshortcuts={kbd}
      {...rest}
      className={cx(
        "zs-btn",
        VARIANT_CLASS[variant],
        size === "sm" && "zs-btn--sm",
        size === "lg" && "zs-btn--lg",
        block && "zs-btn--block",
        iconOnly && "zs-btn--icon",
        className,
      )}
    >
      {icon && <Icon name={icon} />}
      {children}
      {kbd && <Kbd>{kbd}</Kbd>}
    </button>
  );
});

/** Подпись горячей клавиши внутри кнопки. */
export function Kbd({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <span className={cx("zs-kbd", className)} aria-hidden="true">
      {children}
    </span>
  );
}
