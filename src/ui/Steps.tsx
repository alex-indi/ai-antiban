import type { ReactNode } from "react";
import { Icon } from "./Icon";
import { cx } from "./cx";

export type StepStatus = "done" | "current" | "todo";

export interface StepItem {
  /** Заголовок-действие в повелительном наклонении. */
  title: ReactNode;
  /** Одно-два предложения; у пройденного шага — результат. */
  text?: ReactNode;
  status: StepStatus;
  /** Кнопка или баннер у текущего шага. */
  action?: ReactNode;
}

/** Пошаговая инструкция: пройденные сворачиваются до заголовка и результата. Не больше пяти шагов. */
export function Steps({ items, className }: { items: StepItem[]; className?: string }) {
  return (
    <ol className={cx("zs-steps", className)}>
      {items.map((item, i) => (
        <li
          key={i}
          className={cx("zs-step", item.status !== "todo" && `zs-step--${item.status}`)}
          aria-current={item.status === "current" ? "step" : undefined}
        >
          <span className="zs-step__num">
            {item.status === "done" ? <Icon name="check" label="Выполнено" /> : i + 1}
          </span>
          <div className="zs-step__body">
            <p className="zs-step__title">{item.title}</p>
            {item.text && <p className="zs-step__text">{item.text}</p>}
            {item.action}
          </div>
        </li>
      ))}
    </ol>
  );
}
