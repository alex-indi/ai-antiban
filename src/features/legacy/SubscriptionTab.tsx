import type { AppStateData, RiskReport } from "../../types";
import { SubscriptionScreen } from "../subscription";
import type { ShowToast } from "../../shared/hooks";

interface SubscriptionTabProps {
  vpnCountry: string;
  activeReport: RiskReport | null;
  state: AppStateData | null;
  onStateChange: () => void;
  onNavigateToPaymentGuide: () => void;
  onToast: ShowToast;
}

/** Экран «Подписка» (меню «Ещё»): экран на дизайн-системе (src/features/subscription). */
export function SubscriptionTab(props: SubscriptionTabProps) {
  return <SubscriptionScreen {...props} />;
}
