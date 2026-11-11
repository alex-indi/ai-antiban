import { forwardRef } from "react";
import type { ButtonHTMLAttributes, ReactNode } from "react";
import { cx } from "./cx";

export interface SwitchProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "onChange" | "role" | "aria-checked" | "children"> {
  checked: boolean;
  onChange?: (checked: boolean) => void;
  /** Видимая подпись справа; кликабельна. Без неё обязателен aria-label. */
  children?: ReactNode;
  /** Классы для обёртки-подписи (только когда есть children). */
  labelClassName?: string;
}

/** Тумблер для настроек, которые применяются сразу. Для действий с подтверждением — кнопка. */
export const Switch = forwardRef<HTMLButtonElement, SwitchProps>(function Switch(
  { checked, onChange, children, className, labelClassName, disabled, ...rest },
  ref,
) {
  const control = (
    <button
      ref={ref}
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      {...rest}
      className={cx("zs-switch", className)}
      onClick={(e) => {
        rest.onClick?.(e);
        if (!e.defaultPrevented) onChange?.(!checked);
      }}
    />
  );
  if (!children) return control;
  return (
    <label className={cx("zs-field-row", labelClassName)}>
      {control}
      {children}
    </label>
  );
});
