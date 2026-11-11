import { useRef } from "react";
import type { KeyboardEvent } from "react";
import { Icon } from "./Icon";
import type { IconName } from "./Icon";
import { cx } from "./cx";

export interface SegmentedOption<T extends string = string> {
  value: T;
  label: string;
  icon?: IconName;
  disabled?: boolean;
}

export interface SegmentedProps<T extends string = string> {
  /** 2–4 варианта. */
  options: SegmentedOption<T>[];
  value: T;
  onChange: (value: T) => void;
  /** Название группы для скринридера. */
  "aria-label": string;
  className?: string;
}

/** Выбор одного из 2–4 вариантов (radiogroup): стрелки переключают, Tab уходит из группы. */
export function Segmented<T extends string = string>({
  options,
  value,
  onChange,
  className,
  ...rest
}: SegmentedProps<T>) {
  const ref = useRef<HTMLDivElement>(null);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const enabled = options.filter((o) => !o.disabled);
    const index = enabled.findIndex((o) => o.value === value);
    let next = -1;
    if (e.key === "ArrowRight" || e.key === "ArrowDown") next = (index + 1) % enabled.length;
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp") next = (index - 1 + enabled.length) % enabled.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = enabled.length - 1;
    if (next < 0) return;
    e.preventDefault();
    onChange(enabled[next].value);
    const buttons = ref.current?.querySelectorAll<HTMLButtonElement>('[role="radio"]:not(:disabled)');
    buttons?.[next]?.focus();
  };

  return (
    <div ref={ref} role="radiogroup" aria-label={rest["aria-label"]} className={cx("zs-seg", className)} onKeyDown={onKeyDown}>
      {options.map((o) => {
        const selected = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={selected}
            tabIndex={selected ? 0 : -1}
            disabled={o.disabled}
            className="zs-seg__opt"
            onClick={() => onChange(o.value)}
          >
            {o.icon && <Icon name={o.icon} />}
            {o.label}
          </button>
        );
      })}
    </div>
  );
}
