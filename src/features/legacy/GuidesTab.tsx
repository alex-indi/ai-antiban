import { GuideViewer } from "../../components/GuideViewer";
import type { ShowToast } from "../../shared/hooks";

interface GuidesTabProps {
  initialGuide: string;
  currentIp: string;
  currentTimezone?: string;
  onPinIp: (ip: string) => void;
  onToast: ShowToast;
}

/** Вкладка «Гайды»: прежний GuideViewer без переделки. */
export function GuidesTab(props: GuidesTabProps) {
  return <GuideViewer {...props} />;
}
