import { useCallback, useEffect, useState } from "react";
import type { GatewayStatsSnapshot } from "../../types";
import { getGatewayStats } from "../../utils/api";

/** Счётчики шлюза. Пока активен «Обзор», опрос раз в 2 секунды (как раньше на вкладке «Защита»). */
export function useGatewayStats(active: boolean) {
  const [gatewayStats, setGatewayStats] = useState<GatewayStatsSnapshot | null>(null);

  const refreshGatewayStats = useCallback(async () => {
    try {
      const stats = await getGatewayStats();
      setGatewayStats(stats);
    } catch (err) {
      console.warn("Could not fetch gateway stats:", err);
    }
  }, []);

  useEffect(() => {
    if (!active) return;

    refreshGatewayStats();
    const interval = setInterval(() => {
      refreshGatewayStats();
    }, 2000);

    return () => clearInterval(interval);
  }, [active, refreshGatewayStats]);

  return { gatewayStats, refreshGatewayStats };
}
