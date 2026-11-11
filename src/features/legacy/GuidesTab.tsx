import { GuidesScreen } from "../guides";
import type { ShowToast } from "../../shared/hooks";

interface GuidesTabProps {
  initialGuide: string;
  currentIp: string;
  currentTimezone?: string;
  onPinIp: (ip: string) => void;
  onToast: ShowToast;
}

/** Вкладка «Гайды»: экран на дизайн-системе (src/features/guides). */
export function GuidesTab(props: GuidesTabProps) {
  return <GuidesScreen {...props} />;
}
