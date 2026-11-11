// Команды и ссылки гайдов. Перенесены из src/components/GuideViewer.tsx без изменений:
// их копируют в терминал и открывают в браузере, менять символы нельзя.

export const AMNEZIA_DOWNLOAD_URL = "https://amnezia.org/ru/downloads";

export interface GuideCommand {
  id: string;
  /** Команда ровно так, как её копируют в буфер. */
  command: string;
  /** Подпись для уведомления «Команда скопирована». */
  label: string;
}

export const CMD_SSH: GuideCommand = {
  id: "cmd-ssh",
  command: "ssh root@ВАШ_IP_СЕРВЕРА",
  label: "Подключение к VPS по SSH",
};

export const CMD_VLESS_AUTO: GuideCommand = {
  id: "cmd-vless-xhttp-auto",
  command: "bash <(curl -fsSL https://raw.githubusercontent.com/zxcvos/Xray-script/main/install.sh)",
  label: "Авто-установщик VLESS + XHTTP + Reality",
};

export const CMD_3XUI_VLESS: GuideCommand = {
  id: "cmd-3xui-vless",
  command: "bash <(curl -Ls https://raw.githubusercontent.com/mhsanaei/3x-ui/master/install.sh)",
  label: "Установка панели 3X-UI",
};

export const CMD_XRAY_CORE: GuideCommand = {
  id: "cmd-xray-core",
  command: "wget -qO- https://raw.githubusercontent.com/ServerTechnologies/simple-xray-core/refs/heads/main/xray-install | bash",
  label: "Автоустановка голого ядра Xray VLESS Reality",
};

export const CMD_XRAY_CASCADE: GuideCommand = {
  id: "cmd-xray-cascade",
  command: "wget -qO- https://raw.githubusercontent.com/ServerTechnologies/cascade-for-x-ray/refs/heads/main/cascad | bash",
  label: "Скрипт настройки каскадного моста Xray",
};

export const CMD_3XUI_IPV6: GuideCommand = {
  id: "cmd-3xui",
  command: "bash <(curl -Ls https://raw.githubusercontent.com/mhsanaei/3x-ui/master/install.sh)",
  label: "Установка панели 3x-ui",
};

export const CMD_TZ_MAC: GuideCommand = {
  id: "cmd-settimezone-mac",
  command: "sudo systemsetup -settimezone Europe/Amsterdam",
  label: "Команда смены TZ в macOS",
};

/** Строки конфигурации xhttpSettings для способа 2 (CDN). */
export const XHTTP_SETTINGS_LINES: string[] = [
  '"mode": "packet-up", "uplinkHTTPMethod": "GET",',
  '"uplinkDataPlacement": "header", "uplinkDataKey": "x-edge-data", "enableXmux": true',
];

/** Переменные прокси, которые Застава прописывает в ~/.zshrc. */
export const PROXY_EXPORT_LINE = 'export HTTP_PROXY="http://127.0.0.1:8899" HTTPS_PROXY="http://127.0.0.1:8899"';
