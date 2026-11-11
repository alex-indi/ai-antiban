import { useCallback, useState } from "react";
import type { ToastTone } from "../../ui";

/** Тип уведомления в прежнем API: так вызывают showToast старые вкладки. */
export type ToastKind = "success" | "warning" | "error" | "info";

export interface ToastItem {
  id: string;
  tone: ToastTone;
  title: string;
  description?: string;
}

export type ShowToast = (title: string, description?: string, type?: ToastKind) => void;

const TONE: Record<ToastKind, ToastTone> = {
  success: "ok",
  warning: "warn",
  error: "danger",
  info: "info",
};

/** Стопка уведомлений. Подпись showToast не менялась, поэтому старые вкладки получают её как есть. */
export function useToasts() {
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  const dismissToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const showToast = useCallback<ShowToast>((title, description, type = "info") => {
    const id = Date.now().toString() + Math.random().toString(36).substring(2, 6);
    setToasts((prev) => [...prev, { id, title, description, tone: TONE[type] }]);
  }, []);

  return { toasts, showToast, dismissToast };
}
