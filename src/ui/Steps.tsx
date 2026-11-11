import { useState } from "react";
import type { ReactNode } from "react";
import { Button } from "./Button";
import { Icon } from "./Icon";
import { cx } from "./cx";

export type StepStatus = "done" | "current" | "todo";

export interface StepItem {
  /** Заголовок-действие в повелительном наклонении. */
  title: ReactNode;
  /** Одно-два предложения; у пройденного шага — результат. */
  text?: ReactNode;
  /** Статус задаёт вызывающий. В режиме collapsible вычисляется сам. */
  status?: StepStatus;
  /** Кнопка или баннер у текущего шага. */
  action?: ReactNode;
  /** Только collapsible: содержимое шага (команды, карточки, кнопки), видно у раскрытого шага. */
  body?: ReactNode;
}

export interface StepsProps {
  items: StepItem[];
  /**
   * Раскрыт один шаг, остальные свёрнуты до заголовка. Заголовок — кнопка с aria-expanded,
   * к любому шагу можно вернуться; «Далее» отмечает шаг пройденным. Содержимое не сокращается, оно лишь свёрнуто.
   */
  collapsible?: boolean;
  nextLabel?: string;
  className?: string;
}

/** Пошаговая инструкция: пройденные сворачиваются до заголовка и результата. Не больше пяти шагов. */
export function Steps({ items, collapsible = false, nextLabel = "Далее", className }: StepsProps) {
  const [current, setCurrent] = useState(0);
  const [done, setDone] = useState<number[]>([]);
  const last = items.length - 1;

  const advance = (i: number) => {
    setDone((prev) => (prev.includes(i) ? prev : [...prev, i]));
    setCurrent(Math.min(i + 1, last));
  };

  return (
    <ol className={cx("zs-steps", className)}>
      {items.map((item, i) => {
        const open = i === current;
        const status: StepStatus = collapsible ? (open ? "current" : done.includes(i) ? "done" : "todo") : (item.status ?? "todo");
        return (
          <li
            key={i}
            className={cx("zs-step", status !== "todo" && `zs-step--${status}`)}
            aria-current={status === "current" ? "step" : undefined}
          >
            <span className="zs-step__num">{status === "done" ? <Icon name="check" label="Выполнено" /> : i + 1}</span>
            <div className="zs-step__body">
              {collapsible ? (
                <p className="zs-step__title">
                  <button type="button" className="zs-step__toggle" aria-expanded={open} onClick={() => setCurrent(i)}>
                    {item.title}
                  </button>
                </p>
              ) : (
                <p className="zs-step__title">{item.title}</p>
              )}
              {item.text && (!collapsible || open) && <p className="zs-step__text">{item.text}</p>}
              {collapsible ? (
                open && (
                  <div className="zs-step__panel">
                    {item.body}
                    {i < last && (
                      <Button size="sm" onClick={() => advance(i)}>
                        {nextLabel}
                      </Button>
                    )}
                  </div>
                )
              ) : (
                item.action
              )}
            </div>
          </li>
        );
      })}
    </ol>
  );
}
