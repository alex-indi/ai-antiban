import type { AppStateData, RiskReport } from "../../types";
import { BinCheckerTab } from "../../components/BinCheckerTab";
import type { ShowToast } from "../../shared/hooks";

interface SubscriptionTabProps {
  vpnCountry: string;
  activeReport: RiskReport | null;
  state: AppStateData | null;
  onStateChange: () => void;
  onNavigateToPaymentGuide: () => void;
  onToast: ShowToast;
}

/** Экран «Подписка» (меню «Ещё»): прежний BinCheckerTab без переделки. */
export function SubscriptionTab(props: SubscriptionTabProps) {
  return <BinCheckerTab {...props} />;
}
