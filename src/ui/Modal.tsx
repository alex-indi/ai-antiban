import { useEffect, useId, useRef } from "react";
import type { KeyboardEvent, ReactNode } from "react";
import { createPortal } from "react-dom";
import { Button } from "./Button";
import { cx } from "./cx";
import { trapTab } from "./dom";

export interface ModalProps {
  open: boolean;
  /** Закрытие: Esc, крестик, клик по затемнению. */
  onClose: () => void;
  title: string;
  /** Прокручиваемое тело. */
  children: ReactNode;
  /** Действия внизу, справа налево по важности: главное — последним. */
  footer?: ReactNode;
  /** Закрывать по клику на затемнение. По умолчанию да. */
  closeOnBackdrop?: boolean;
  className?: string;
}

/** Окно настроек: заголовок, прокручиваемое тело, футер с действиями, фокус-ловушка, Esc. */
export function Modal({ open, onClose, title, children, footer, closeOnBackdrop = true, className }: ModalProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  const titleId = useId();
  const closeRef = useRef(onClose);
  closeRef.current = onClose;

  // Фокус внутрь при открытии и возврат на прежний элемент при закрытии; блокировка прокрутки страницы.
  useEffect(() => {
    if (!open) return;
    const previous = document.activeElement as HTMLElement | null;
    const overflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    panelRef.current?.focus();
    return () => {
      document.body.style.overflow = overflow;
      previous?.focus?.();
    };
  }, [open]);

  if (!open) return null;

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      closeRef.current();
    } else if (panelRef.current) {
      trapTab(e, panelRef.current);
    }
  };

  // Портал вне .zs-root, поэтому обёртка сама несёт zs-root (шрифт, кольцо фокуса).
  return createPortal(
    <div className="zs-root zs-layer">
      <div
        className="zs-modal__backdrop"
        onMouseDown={(e) => {
          if (closeOnBackdrop && e.target === e.currentTarget) closeRef.current();
        }}
      >
        <div
          ref={panelRef}
          role="dialog"
          aria-modal="true"
          aria-labelledby={titleId}
          tabIndex={-1}
          className={cx("zs-modal", className)}
          onKeyDown={onKeyDown}
        >
          <div className="zs-modal__head">
            <h2 className="zs-modal__title" id={titleId}>
              {title}
            </h2>
            <Button variant="quiet" size="sm" iconOnly icon="x" aria-label="Закрыть" onClick={onClose} />
          </div>
          <div className="zs-modal__body">{children}</div>
          {footer && <div className="zs-modal__foot">{footer}</div>}
        </div>
      </div>
    </div>,
    document.body,
  );
}
