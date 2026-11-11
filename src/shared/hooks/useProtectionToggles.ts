import { useCallback, useEffect, useState } from "react";
import {
  checkCliProtectionStatus,
  disableCliProtection,
  enableCliProtection,
  getNetworkKillswitchStatus,
  toggleNetworkKillswitch,
  toggleSysproxy,
} from "../../utils/api";
import { toast as t } from "../copy/ru";
import type { ShowToast } from "./useToasts";

interface Options {
  sysproxyActive: boolean;
  setSysproxyActive: (value: boolean) => void;
  showToast: ShowToast;
}

/** Тумблеры защиты: системный прокси (PAC), защита редакторов и CLI, kill-switch «Остановить сеть». */
export function useProtectionToggles({ sysproxyActive, setSysproxyActive, showToast }: Options) {
  const [cliProtected, setCliProtected] = useState<boolean>(false);
  const [killswitchFrozen, setKillswitchFrozen] = useState<boolean>(false);

  // Стартовое чтение: kill-switch и статус защиты CLI.
  useEffect(() => {
    getNetworkKillswitchStatus()
      .then((frozen) => setKillswitchFrozen(frozen))
      .catch(() => {});
    checkCliProtectionStatus()
      .then((active) => {
        if (active) setCliProtected(true);
      })
      .catch(() => {});
  }, []);

  const handleToggleSysproxy = useCallback(async () => {
    try {
      const next = !sysproxyActive;
      const res = await toggleSysproxy(next);
      setSysproxyActive(res);
      showToast(res ? t.guard.onTitle : t.guard.offTitle, res ? t.guard.onText : t.guard.offText, res ? "success" : "info");
    } catch (err: any) {
      showToast(t.guard.error, err?.message || String(err), "error");
    }
  }, [sysproxyActive, setSysproxyActive, showToast]);

  const handleToggleCliProtection = useCallback(async () => {
    try {
      if (cliProtected) {
        await disableCliProtection();
        setCliProtected(false);
        showToast(t.cli.offTitle, t.cli.offText, "info");
      } else {
        const report = await enableCliProtection();
        try {
          await navigator.clipboard.writeText(report.shell_command);
        } catch {
          // буфер обмена необязателен
        }
        setCliProtected(true);
        const targets =
          report.configured_targets.length > 0 ? report.configured_targets.join(", ") : t.cli.onFallbackTargets;
        showToast(t.cli.onTitle, t.cli.onText(targets), "success");
      }
    } catch (err: any) {
      showToast(t.cli.error, err?.message || String(err), "error");
    }
  }, [cliProtected, showToast]);

  const handleToggleKillswitch = useCallback(async () => {
    try {
      const next = !killswitchFrozen;
      const isFrozen = await toggleNetworkKillswitch(next);
      setKillswitchFrozen(isFrozen);
      if (isFrozen) {
        showToast(t.killswitch.onTitle, t.killswitch.onText, "warning");
      } else {
        showToast(t.killswitch.offTitle, t.killswitch.offText, "success");
      }
    } catch (err: any) {
      showToast(t.killswitch.error, err?.message || String(err), "error");
    }
  }, [killswitchFrozen, showToast]);

  return {
    cliProtected,
    killswitchFrozen,
    setKillswitchFrozen,
    handleToggleSysproxy,
    handleToggleCliProtection,
    handleToggleKillswitch,
  };
}
