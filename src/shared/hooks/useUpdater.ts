import { useCallback, useEffect, useState } from "react";
import type { UpdateCheckResult, UpdateProgressPayload, UpdateStatusPayload } from "../../types";
import { checkForUpdates, installUpdate, toggleAutoUpdate } from "../../utils/api";
import { openExternal } from "../../utils/opener";
import { toast as t } from "../copy/ru";
import type { ShowToast } from "./useToasts";

interface Options {
  showToast: ShowToast;
  setAutoUpdateEnabled: (value: boolean) => void;
}

/** Проверка, установка и автообновление. События updater:// подключает useTrayEvents через apply*. */
export function useUpdater({ showToast, setAutoUpdateEnabled }: Options) {
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResult | null>(null);
  const [checkingUpdate, setCheckingUpdate] = useState<boolean>(false);
  const [installingUpdate, setInstallingUpdate] = useState<boolean>(false);
  const [updateProgress, setUpdateProgress] = useState<UpdateProgressPayload | null>(null);
  const [updaterStatus, setUpdaterStatus] = useState<UpdateStatusPayload | null>(null);
  const [showUpdateModal, setShowUpdateModal] = useState<boolean>(false);

  const handleCheckUpdates = useCallback(
    async (silent: boolean = false) => {
      setCheckingUpdate(true);
      try {
        const info = await checkForUpdates();
        setUpdateInfo(info);
        if (info.update_available) {
          setShowUpdateModal(true);
          showToast(t.updates.available(info.latest_version), info.release_name || t.updates.availableText, "info");
        } else if (!silent) {
          setShowUpdateModal(true);
          showToast(t.updates.none, t.updates.noneText(info.current_version), "success");
        }
      } catch (err: any) {
        if (!silent) {
          showToast(t.updates.checkError, err?.message || String(err), "error");
        }
      } finally {
        setCheckingUpdate(false);
      }
    },
    [showToast],
  );

  const handleInstallUpdate = useCallback(async () => {
    if (!updateInfo?.download_url || !updateInfo?.asset_name) {
      if (updateInfo?.release_url) {
        await openExternal(updateInfo.release_url);
      }
      return;
    }

    setInstallingUpdate(true);
    setUpdateProgress({
      downloaded_bytes: 0,
      total_bytes: updateInfo.asset_size || 0,
      percent: 0,
      phase: "starting",
    });

    try {
      const res = await installUpdate(updateInfo.download_url, updateInfo.asset_name);
      showToast(res.will_restart ? t.updates.installed : t.updates.ready, res.message, "success");
      if (!res.will_restart) {
        setShowUpdateModal(false);
      }
    } catch (err: any) {
      showToast(t.updates.installError, err?.message || String(err), "error");
    } finally {
      setInstallingUpdate(false);
      setUpdateProgress(null);
    }
  }, [updateInfo, showToast]);

  const handleToggleAutoUpdate = useCallback(
    async (enabled: boolean) => {
      try {
        await toggleAutoUpdate(enabled);
        setAutoUpdateEnabled(enabled);
        showToast(
          enabled ? t.updates.autoOn : t.updates.autoOff,
          enabled ? t.updates.autoOnText : t.updates.autoOffText,
          "info",
        );
      } catch (err: any) {
        showToast(t.updates.autoError, err?.message || String(err), "error");
      }
    },
    [setAutoUpdateEnabled, showToast],
  );

  // Тихая проверка на старте.
  useEffect(() => {
    handleCheckUpdates(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // События updater:// из Rust.
  const applyUpdateProgress = useCallback((payload: UpdateProgressPayload) => {
    setUpdateProgress(payload);
  }, []);
  const applyUpdateAvailable = useCallback((payload: UpdateCheckResult) => {
    setUpdateInfo(payload);
  }, []);
  const applyUpdateStatus = useCallback((payload: UpdateStatusPayload) => {
    setUpdaterStatus(payload);
    if (payload.stage === "installing" || payload.stage === "restarting") {
      setInstallingUpdate(true);
    }
  }, []);

  return {
    updateInfo,
    checkingUpdate,
    installingUpdate,
    updateProgress,
    updaterStatus,
    showUpdateModal,
    setShowUpdateModal,
    handleCheckUpdates,
    handleInstallUpdate,
    handleToggleAutoUpdate,
    applyUpdateProgress,
    applyUpdateAvailable,
    applyUpdateStatus,
  };
}
