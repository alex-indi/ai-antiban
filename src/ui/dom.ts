import { useEffect, useRef } from "react";

const FOCUSABLE =
  'a[href], button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';

/** Фокусируемые элементы внутри контейнера, в порядке документа. */
export function getFocusable(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => !el.hasAttribute("hidden") && el.getAttribute("aria-hidden") !== "true",
  );
}

/** Замыкает Tab / Shift+Tab внутри контейнера (фокус-ловушка). */
export function trapTab(event: KeyboardEvent | React.KeyboardEvent, root: HTMLElement) {
  if (event.key !== "Tab") return;
  const items = getFocusable(root);
  if (items.length === 0) {
    event.preventDefault();
    root.focus();
    return;
  }
  const first = items[0];
  const last = items[items.length - 1];
  const active = document.activeElement;
  if (event.shiftKey && (active === first || active === root)) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && active === last) {
    event.preventDefault();
    first.focus();
  }
}

/** Вызывает handler при нажатии мыши/пальца вне элемента, пока enabled. */
export function useClickOutside(
  ref: React.RefObject<HTMLElement | null>,
  enabled: boolean,
  handler: () => void,
) {
  const latest = useRef(handler);
  latest.current = handler;
  useEffect(() => {
    if (!enabled) return;
    const onDown = (e: PointerEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) latest.current();
    };
    document.addEventListener("pointerdown", onDown);
    return () => document.removeEventListener("pointerdown", onDown);
  }, [ref, enabled]);
}
