import { useEffect, useRef } from "react";
import type { ReactNode } from "react";
import { Button } from "../../ui";
import type { ButtonProps } from "../../ui";
import { onboarding as copy } from "../../shared/copy/ru";

/** Каркас шага: заголовок (на него переходит фокус), короткое пояснение, тело и подвал с действиями. */
export function StepFrame({
  title,
  lead,
  children,
  footer,
}: {
  title: string;
  lead?: ReactNode;
  children?: ReactNode;
  footer: ReactNode;
}) {
  const headingRef = useRef<HTMLHeadingElement>(null);

  // Шаг монтируется заново при каждом переходе: фокус на заголовок, чтобы читалка начала с него.
  useEffect(() => {
    headingRef.current?.focus({ preventScroll: true });
  }, []);

  return (
    <>
      <div className="zs-onb__body">
        <div className="zs-onb__intro">
          <h1 ref={headingRef} tabIndex={-1} className="zs-t-title zs-onb__title">
            {title}
          </h1>
          {lead && <p className="zs-onb__lead">{lead}</p>}
        </div>
        {children}
      </div>
      {footer}
    </>
  );
}

type ActionProps = Omit<ButtonProps, "variant" | "size" | "block" | "iconOnly"> & { children: ReactNode };

/**
 * Подвал шага: главное действие на всю ширину (единственная кнопка brand), под ним «Назад» слева и тихое действие справа.
 * Главная кнопка помечена data-onb-primary: по Enter на неё нажимает сценарий.
 */
export function StepFooter({
  primary,
  back,
  secondary,
}: {
  primary: ActionProps;
  back?: () => void;
  secondary?: ActionProps;
}) {
  // Без тихого действия «Назад» стоит в одной строке с главной кнопкой: так шаг выше на 44px.
  const inline = Boolean(back) && !secondary;
  return (
    <div className={inline ? "zs-onb__foot zs-onb__foot--inline" : "zs-onb__foot"}>
      {inline && (
        <Button variant="quiet" size="lg" onClick={back}>
          {copy.back}
        </Button>
      )}
      <Button variant="primary" size="lg" block={!inline} className="zs-onb__primary" data-onb-primary="" {...primary} />
      {!inline && (back || secondary) && (
        <div className="zs-onb__foot-row">
          {back ? (
            <Button variant="quiet" onClick={back}>
              {copy.back}
            </Button>
          ) : (
            <span />
          )}
          {secondary && <Button variant="quiet" {...secondary} />}
        </div>
      )}
    </div>
  );
}
