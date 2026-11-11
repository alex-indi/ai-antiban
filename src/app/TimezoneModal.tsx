import { Button, Modal } from "../ui";
import { timezone as copy } from "../shared/copy/ru";

interface TimezoneModalProps {
  open: boolean;
  onClose: () => void;
  syncing: boolean;
  synced: boolean;
  onSync: () => Promise<void>;
  /** Открыть настройки времени ОС и перейти к гайду. */
  onOpenOsClock: () => void;
}

/** «Настроить часовой пояс»: синхронизация для ИИ-инструментов и переход к часам системы. */
export function TimezoneModal({ open, onClose, syncing, synced, onSync, onOpenOsClock }: TimezoneModalProps) {
  return (
    <Modal
      open={open}
      onClose={onClose}
      title={copy.title}
      footer={
        <>
          <Button variant="quiet" onClick={onOpenOsClock}>
            {copy.osClock}
          </Button>
          <Button
            variant="primary"
            disabled={syncing}
            onClick={async () => {
              await onSync();
              onClose();
            }}
          >
            {syncing ? copy.syncing : copy.sync}
          </Button>
        </>
      }
    >
      <p className="zs-modal-text">{copy.text}</p>
      {synced && <p className="zs-modal-text">{copy.done}</p>}
    </Modal>
  );
}
