import { BackupScreen } from "../backup";
import type { ClaudeBackup, ShowToast } from "../../shared/hooks";

interface BackupTabProps {
  backup: ClaudeBackup;
  currentIp: string;
  currentCountry: string;
  activeUpstreamKey?: string | null;
  onProfileSwitched: (profileName: string, pinnedIp?: string | null) => void;
  onToast: ShowToast;
}

/** Вкладка «Бэкап»: точка подключения нового экрана features/backup. */
export function BackupTab(props: BackupTabProps) {
  return <BackupScreen {...props} />;
}
