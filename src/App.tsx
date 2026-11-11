import { useEffect, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { isTauri } from "@tauri-apps/api/core";
import { AppShell } from "./app/AppShell";
import { AlertWindowPage } from "./features/alert";
import "./App.css";

/** Окно alert-popup по URL известно сразу: главное окно в нём даже не монтируем. */
function labelFromUrl(): string {
  const winParam = new URLSearchParams(window.location.search).get("window");
  const isAlert =
    winParam === "alert-popup" ||
    winParam === "alert" ||
    window.location.hash === "#alert-popup" ||
    window.location.hash === "#alert";
  return isAlert ? "alert-popup" : "main";
}

function App() {
  const [windowLabel, setWindowLabel] = useState<string>(labelFromUrl);

  useEffect(() => {
    // 1. Параметры URL / hash (например, ?window=alert-popup или #alert, #alert-popup)
    const params = new URLSearchParams(window.location.search);
    const winParam = params.get("window");
    if (
      winParam === "alert-popup" ||
      winParam === "alert" ||
      window.location.hash === "#alert-popup" ||
      window.location.hash === "#alert"
    ) {
      setWindowLabel("alert-popup");
      return;
    }

    // 2. В Tauri смотрим метку окна
    if (isTauri()) {
      try {
        const win = getCurrentWebviewWindow();
        setWindowLabel(win.label);
      } catch (err) {
        console.warn("Could not determine Tauri window label, defaulting to main:", err);
        setWindowLabel("main");
      }
    } else {
      setWindowLabel("main");
    }
  }, []);

  useEffect(() => {
    const isAlert =
      windowLabel === "alert-popup" ||
      window.location.hash === "#alert" ||
      window.location.hash === "#alert-popup";
    if (isAlert) {
      document.documentElement.classList.add("is-alert-popup");
      document.body.classList.add("is-alert-popup");
    } else {
      document.documentElement.classList.remove("is-alert-popup");
      document.body.classList.remove("is-alert-popup");
    }
  }, [windowLabel]);

  // Окно alert-popup: диалог обрыва VPN на прозрачном фоне
  if (windowLabel === "alert-popup") {
    return <AlertWindowPage />;
  }

  return <AppShell />;
}

export default App;
