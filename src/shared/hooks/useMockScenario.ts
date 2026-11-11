import { useCallback, useState } from "react";
import { getMockScenario, setMockScenario } from "../../utils/api";
import type { MockScenario } from "../../utils/api";

/** Сценарий моков для превью в браузере без Tauri (?scenario=clean|turkey|ru). */
export function useMockScenario(runFullAudit: (force?: boolean, silentError?: boolean) => Promise<void>) {
  const [scenario, setScenario] = useState<MockScenario>(() => {
    const s = new URLSearchParams(window.location.search).get("scenario") as MockScenario | null;
    if (s === "clean" || s === "turkey" || s === "ru") {
      setMockScenario(s);
      return s;
    }
    return getMockScenario();
  });

  const handleSwitchScenario = useCallback(
    (next: MockScenario) => {
      setMockScenario(next);
      setScenario(next);
      runFullAudit(true);
    },
    [runFullAudit],
  );

  return { scenario, handleSwitchScenario };
}
