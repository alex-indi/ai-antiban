import { useState } from "react";
import type { ReactNode } from "react";
import { Button, Steps } from "../../ui";
import type { StepItem } from "../../ui";
import { common } from "../../shared/copy/guides-subscription";

export interface GuideStep {
  title: ReactNode;
  /** Одно-два предложения под заголовком; видны у раскрытого шага. */
  text?: ReactNode;
  /** Содержимое шага: команды, карточки, кнопки. Видно только у раскрытого шага. */
  body?: ReactNode;
}

/**
 * Пошаговая инструкция на примитиве Steps: раскрыт один шаг, остальные свёрнуты до заголовка.
 * Заголовок шага — кнопка (aria-expanded): можно вернуться к любому шагу. «Далее» отмечает шаг пройденным.
 * Содержимое шагов не сокращается, оно лишь свёрнуто.
 */
export function GuideSteps({ steps }: { steps: GuideStep[] }) {
  const [current, setCurrent] = useState(0);
  const [done, setDone] = useState<number[]>([]);
  const last = steps.length - 1;

  const advance = (i: number) => {
    setDone((prev) => (prev.includes(i) ? prev : [...prev, i]));
    setCurrent(Math.min(i + 1, last));
  };

  const items: StepItem[] = steps.map((step, i) => {
    const open = i === current;
    return {
      status: open ? "current" : done.includes(i) ? "done" : "todo",
      title: (
        <button type="button" className="guides-step__toggle" aria-expanded={open} onClick={() => setCurrent(i)}>
          {step.title}
        </button>
      ),
      text: open ? step.text : undefined,
      action: open ? (
        <div className="guides-step__panel">
          {step.body}
          {i < last && (
            <Button size="sm" onClick={() => advance(i)}>
              {common.next}
            </Button>
          )}
        </div>
      ) : undefined,
    };
  });

  return <Steps items={items} />;
}
