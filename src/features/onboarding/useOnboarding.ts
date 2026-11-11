import { useCallback, useState } from "react";

/** Флаг «знакомство пройдено или пропущено» для версии 2. Старый ключ zastava_hide_onboarding_v13 не учитывается. */
export const ONBOARDING_DONE_KEY = "zastava_onboarding_v2_done";

export const ONBOARDING_STEPS = 4;

function readDone(): boolean {
  try {
    return window.localStorage.getItem(ONBOARDING_DONE_KEY) === "1";
  } catch {
    return false;
  }
}

function writeDone() {
  try {
    window.localStorage.setItem(ONBOARDING_DONE_KEY, "1");
  } catch {
    // Хранилище недоступно (приватное окно, запрет): знакомство просто покажется снова.
  }
}

interface Initial {
  open: boolean;
  /** 1…5; 5 — финальный экран. Параметр ?step= работает только в dev. */
  step: number;
}

/** ?onboarding=1 открывает всегда, ?onboarding=0 подавляет; окно alert-popup — никогда. */
function readInitial(): Initial {
  const params = new URLSearchParams(window.location.search);
  const isAlertWindow =
    params.get("window") === "alert-popup" ||
    params.get("window") === "alert" ||
    window.location.hash === "#alert-popup" ||
    window.location.hash === "#alert";
  const forced = params.get("onboarding");

  let open = !readDone();
  if (forced === "1") open = true;
  if (forced === "0") open = false;
  if (isAlertWindow) open = false;

  let step = 1;
  if (import.meta.env.DEV) {
    const n = Number(params.get("step"));
    if (Number.isInteger(n) && n >= 1 && n <= ONBOARDING_STEPS + 1) step = n;
  }
  return { open, step };
}

/** Показ онбординга: первый запуск новой версии, повторный запуск из меню «Ещё», запись флага. */
export function useOnboarding() {
  const [initial] = useState(readInitial);
  const [open, setOpen] = useState(initial.open);
  /** Меняется при каждом запуске: сценарий монтируется заново и начинается с первого шага. */
  const [run, setRun] = useState(0);

  const start = useCallback(() => {
    setRun((n) => n + 1);
    setOpen(true);
  }, []);

  /** Закрыть и запомнить: и «Пропустить», и обычное завершение. */
  const finish = useCallback(() => {
    writeDone();
    setOpen(false);
  }, []);

  return { open, run, start, finish, initialStep: run === 0 ? initial.step : 1 };
}
