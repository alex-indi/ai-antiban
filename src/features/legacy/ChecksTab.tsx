import type { AppStateData, RiskReport } from "../../types";
import { DeepAuditTab } from "../../components/DeepAuditTab";

interface ChecksTabProps {
  report: RiskReport | null;
  state: AppStateData | null;
  onNavigateToGuide: (guideId?: string) => void;
  onSimulate: () => void;
  onPinCurrentIp: (ip: string) => void;
  onSyncTimezone: () => void;
  onOpenCrashTest: () => void;
}

/** Вкладка «Проверки»: прежний DeepAuditTab без переделки. Перерисует следующая волна. */
export function ChecksTab(props: ChecksTabProps) {
  return <DeepAuditTab {...props} />;
}
