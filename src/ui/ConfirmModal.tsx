import type { ReactNode } from "react";
import { Button } from "./Button";
import { Modal } from "./Modal";

export interface ConfirmModalProps {
  open: boolean;
  title: string;
  children: ReactNode;
  /** Подпись подтверждающей кнопки: глагол и объект. */
  confirmLabel: string;
  cancelLabel?: string;
  /** Идёт выполнение: кнопки отключены, подпись заменена. */
  busy?: boolean;
  busyLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Подтверждение необратимого действия вместо window.confirm.
 * Рисуйте рядом с родительским Modal (соседним элементом), а не внутри него: иначе Tab ловят обе ловушки фокуса.
 */
export function ConfirmModal({
  open,
  title,
  children,
  confirmLabel,
  cancelLabel = "Отмена",
  busy,
  busyLabel,
  onConfirm,
  onCancel,
}: ConfirmModalProps) {
  return (
    <Modal
      open={open}
      onClose={busy ? () => {} : onCancel}
      title={title}
      footer={
        <>
          <Button onClick={onCancel} disabled={busy}>
            {cancelLabel}
          </Button>
          <Button variant="danger-solid" onClick={onConfirm} disabled={busy}>
            {busy && busyLabel ? busyLabel : confirmLabel}
          </Button>
        </>
      }
    >
      <p className="zs-modal-text">{children}</p>
    </Modal>
  );
}
