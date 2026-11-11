import { ClaudeBackupCard } from "../../components/ClaudeBackupCard";
import { ClaudeProfilesCard } from "../../components/ClaudeProfilesCard";
import type { ClaudeBackup, ShowToast } from "../../shared/hooks";

interface BackupTabProps {
  backup: ClaudeBackup;
  currentIp: string;
  currentCountry: string;
  activeUpstreamKey?: string | null;
  onProfileSwitched: (profileName: string, pinnedIp?: string | null) => void;
  onToast: ShowToast;
}

/** Вкладка «Бэкап»: прежняя карточка экспорта и ClaudeProfilesCard без переделки. */
export function BackupTab({ backup, currentIp, currentCountry, activeUpstreamKey, onProfileSwitched, onToast }: BackupTabProps) {
  return (
    <div className="space-y-3.5">
      <ClaudeBackupCard backup={backup} />
      <div id="claude-profiles-section">
        <ClaudeProfilesCard
          currentIp={currentIp}
          currentCountry={currentCountry}
          activeUpstreamKey={activeUpstreamKey}
          onProfileSwitched={onProfileSwitched}
          onToast={onToast}
        />
      </div>
    </div>
  );
}
