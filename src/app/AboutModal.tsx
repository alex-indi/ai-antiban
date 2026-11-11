import { Button, Modal, Mono } from "../ui";
import { about as copy } from "../shared/copy/ru";

interface AboutModalProps {
  open: boolean;
  onClose: () => void;
  version: string;
  onSupport: () => void;
}

/** «О программе»: версия, что делает Застава, связь с поддержкой. */
export function AboutModal({ open, onClose, version, onSupport }: AboutModalProps) {
  return (
    <Modal
      open={open}
      onClose={onClose}
      title={copy.title}
      footer={
        <>
          <Button variant="quiet" onClick={onSupport}>
            {copy.support}
          </Button>
          <Button onClick={onClose}>{copy.close}</Button>
        </>
      }
    >
      <p className="zs-modal-text">
        {copy.version} <Mono>v{version}</Mono>
      </p>
      <p className="zs-modal-text">{copy.text}</p>
      <p className="zs-modal-text">{copy.privacy}</p>
    </Modal>
  );
}
