import { CrashTestModal } from "../crash-test";
import type { ShowToast } from "../../shared/hooks";

interface CrashTestDialogProps {
  open: boolean;
  onClose: () => void;
  onToast: ShowToast;
}

/** Тест обрыва VPN: точка подключения нового окна features/crash-test. */
export function CrashTestDialog({ open, onClose, onToast }: CrashTestDialogProps) {
  return <CrashTestModal open={open} onClose={onClose} onToast={onToast} />;
}
