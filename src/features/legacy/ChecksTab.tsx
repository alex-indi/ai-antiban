import type { AppStateData, RiskReport } from "../../types";
import { ChecksScreen } from "../checks";

interface ChecksTabProps {
  report: RiskReport | null;
  state: AppStateData | null;
  onNavigateToGuide: (guideId?: string) => void;
  onSimulate: () => void;
  onPinCurrentIp: (ip: string) => void;
  onSyncTimezone: () => void;
  onOpenCrashTest: () => void;
  /** Новое: повторный аудит. Пока оболочка его не передаёт, кнопка «Перепроверить» скрыта. */
  onRecheck?: () => void;
  rechecking?: boolean;
}

/** Вкладка «Проверки»: точка подключения нового экрана features/checks. */
export function ChecksTab(props: ChecksTabProps) {
  return <ChecksScreen {...props} />;
}
