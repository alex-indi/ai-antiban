import { VpnCrashTestModal } from "../../components/VpnCrashTestModal";
import type { ShowToast } from "../../shared/hooks";

interface CrashTestDialogProps {
  open: boolean;
  onClose: () => void;
  onToast: ShowToast;
}

/** Тест обрыва VPN: прежний VpnCrashTestModal как есть. */
export function CrashTestDialog({ open, onClose, onToast }: CrashTestDialogProps) {
  return <VpnCrashTestModal isOpen={open} onClose={onClose} onToast={onToast} />;
}
