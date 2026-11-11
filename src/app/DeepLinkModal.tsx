import { Badge, Button, Card, Field, Modal } from "../ui";

export interface DeepLinkModalProps {
  deepLinkKey: string | null;
  onClose: () => void;
  onConfirm: () => void;
  submitting?: boolean;
}

/** Модальное окно подтверждения подключения ключа из ссылки zastava://... */
export function DeepLinkModal({
  deepLinkKey,
  onClose,
  onConfirm,
  submitting = false,
}: DeepLinkModalProps) {
  if (!deepLinkKey) return null;

  const maskedKey =
    deepLinkKey.length > 18
      ? `${deepLinkKey.slice(0, 18)}…`
      : deepLinkKey;

  return (
    <Modal
      open={Boolean(deepLinkKey)}
      onClose={onClose}
      title="Подключить ключ Заставы?"
      footer={
        <>
          <Button variant="quiet" onClick={onClose} disabled={submitting}>
            Отмена
          </Button>
          <Button
            variant="primary"
            onClick={onConfirm}
            disabled={submitting}
          >
            {submitting ? "Подключение…" : "Подключить ключ"}
          </Button>
        </>
      }
    >
      <div className="zs-stack">
        <p className="zs-modal-text">
          Обнаружена ссылка для быстрого подключения ключа. Проверить и добавить этот маршрут?
        </p>

        <Card>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              marginBottom: "var(--space-2)",
            }}
          >
            <span className="zs-t-caption" style={{ color: "var(--ink-2)" }}>
              Ссылка быстрого подключения
            </span>
            <Badge tone="ok" icon="key">
              zastava://
            </Badge>
          </div>
          <Field
            label="Предпросмотр ключа"
            readOnly
            value={maskedKey}
            help="Маршрут будет проверен и назначен основным для ИИ-сервисов."
          />
        </Card>
      </div>
    </Modal>
  );
}
