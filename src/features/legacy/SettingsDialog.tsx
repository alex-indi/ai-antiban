import { SettingsModal } from "../settings";
import type { AppStateData } from "../../types";
import type { ShowToast } from "../../shared/hooks";

interface SettingsDialogProps {
  open: boolean;
  onClose: () => void;
  state: AppStateData | null;
  autoUpdateEnabled: boolean;
  onToggleAutoUpdate: (enabled: boolean) => Promise<void>;
  onStateUpdated: () => void;
  onToast: ShowToast;
}

/** Настройки: точка подключения нового окна features/settings. Его onToast говорит «warn», оболочка — «warning». */
export function SettingsDialog({
  open,
  onClose,
  state,
  autoUpdateEnabled,
  onToggleAutoUpdate,
  onStateUpdated,
  onToast,
}: SettingsDialogProps) {
  return (
    <SettingsModal
      open={open}
      onClose={onClose}
      autoUpdateEnabled={autoUpdateEnabled}
      onToggleAutoUpdate={onToggleAutoUpdate}
      interceptionBehavior={state?.interception_behavior}
      popupPosition={state?.popup_position}
      customAiDomains={state?.custom_ai_domains}
      onStateUpdated={onStateUpdated}
      onToast={(title, msg, type) => onToast(title, msg, type === "warn" ? "warning" : type || "info")}
    />
  );
}
