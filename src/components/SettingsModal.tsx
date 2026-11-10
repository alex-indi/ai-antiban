import React, { useEffect, useState } from "react";
import {
  Settings,
  X,
  Lock,
  Cpu,
  Globe,
  ExternalLink,
  Bell,
  VolumeX,
  ShieldCheck,
  Plus,
  RefreshCw,
  Check,
  Sparkles,
} from "lucide-react";
import { InterceptionBehavior, PopupPosition } from "../types";
import {
  addCustomAiDomain,
  forceCleanOsProxy,
  getCustomAiDomains,
  getInterceptionBehavior,
  getPopupPosition,
  removeCustomAiDomain,
  setInterceptionBehavior,
  setPopupPosition,
} from "../utils/api";
import { openExternal } from "../utils/opener";

export interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  autoUpdateEnabled: boolean;
  onToggleAutoUpdate: (enabled: boolean) => Promise<void>;
  interceptionBehavior?: InterceptionBehavior;
  popupPosition?: PopupPosition;
  customAiDomains?: string[];
  onStateUpdated?: () => void;
  onToast?: (
    title: string,
    msg: string,
    type?: "success" | "warn" | "info"
  ) => void;
}

const BUILTIN_PROTECTED_ECOSYSTEMS = [
  "Claude (Web / Desktop / Code)",
  "ChatGPT & OpenAI",
  "Gemini & NotebookLM",
  "Cursor & Windsurf",
  "VS Code & GitHub Copilot",
  "Word & Excel Copilot (Microsoft 365)",
  "DeepSeek · Grok · Perplexity · OpenRouter",
];

export const SettingsModal: React.FC<SettingsModalProps> = ({
  isOpen,
  onClose,
  autoUpdateEnabled,
  onToggleAutoUpdate,
  interceptionBehavior: propBehavior,
  popupPosition: propPosition,
  customAiDomains: propCustomDomains,
  onStateUpdated,
  onToast,
}) => {
  const [updatingToggle, setUpdatingToggle] = useState(false);

  // Settings State
  const [behavior, setBehavior] = useState<InterceptionBehavior>(
    propBehavior || "ask_popup"
  );
  const [position, setPosition] = useState<PopupPosition>(
    propPosition || "top_right"
  );
  const [customDomains, setCustomDomains] = useState<string[]>(
    propCustomDomains || []
  );
  const [newDomainInput, setNewDomainInput] = useState("");
  const [domainBusy, setDomainBusy] = useState(false);

  // OS Proxy Clean State
  const [cleaningProxy, setCleaningProxy] = useState(false);
  const [proxyCleanStatus, setProxyCleanStatus] = useState<string | null>(null);

  // Sync state when modal opens or props change
  useEffect(() => {
    if (!isOpen) return;

    if (propBehavior) setBehavior(propBehavior);
    if (propPosition) setPosition(propPosition);
    if (propCustomDomains) setCustomDomains(propCustomDomains);

    // Fetch latest persisted settings from backend
    getInterceptionBehavior()
      .then((b) => setBehavior(b))
      .catch(() => {});
    getPopupPosition()
      .then((p) => setPosition(p))
      .catch(() => {});
    getCustomAiDomains()
      .then((d) => setCustomDomains(d))
      .catch(() => {});
    setProxyCleanStatus(null);
  }, [isOpen, propBehavior, propPosition, propCustomDomains]);

  if (!isOpen) return null;

  const handleToggleAutoUpdate = async () => {
    if (updatingToggle) return;
    setUpdatingToggle(true);
    try {
      await onToggleAutoUpdate(!autoUpdateEnabled);
    } finally {
      setUpdatingToggle(false);
    }
  };

  const handleBehaviorChange = async (newBehavior: InterceptionBehavior) => {
    setBehavior(newBehavior);
    try {
      await setInterceptionBehavior(newBehavior);
      onStateUpdated?.();
      onToast?.(
        "Режим защиты обновлен",
        newBehavior === "ask_popup"
          ? "Включены компактные уведомления при обрыве VPN"
          : newBehavior === "auto_lock_silent"
          ? "Включена тихая авто-пауза без всплывающих окон"
          : "Включен авто-пропуск для доверенных стран (NL, DE, US, FI)",
        "success"
      );
    } catch (err: any) {
      onToast?.("Ошибка сохранения", err?.message || String(err), "warn");
    }
  };

  const handlePositionChange = async (newPosition: PopupPosition) => {
    setPosition(newPosition);
    try {
      await setPopupPosition(newPosition);
      onStateUpdated?.();
      onToast?.(
        "Позиция окна сохранена",
        newPosition === "top_right"
          ? "Уведомления будут появляться в правом верхнем углу"
          : "Уведомления будут появляться сверху по центру",
        "info"
      );
    } catch (err: any) {
      onToast?.("Ошибка сохранения", err?.message || String(err), "warn");
    }
  };

  const handleAddDomain = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    let domain = newDomainInput.trim().toLowerCase();
    if (!domain) return;

    // Strip protocols, ports and path slashes if user pasted a URL
    domain = domain
      .replace(/^https?:\/\//i, "")
      .replace(/\/.*$/, "")
      .replace(/:\d+$/, "")
      .trim();

    if (!domain) return;

    setDomainBusy(true);
    try {
      const updated = await addCustomAiDomain(domain);
      setCustomDomains(updated);
      setNewDomainInput("");
      onStateUpdated?.();
      onToast?.("Домен добавлен", `ИИ-сервис ${domain} теперь защищается Заставой`, "success");
    } catch (err: any) {
      onToast?.("Ошибка добавления", err?.message || String(err), "warn");
    } finally {
      setDomainBusy(false);
    }
  };

  const handleRemoveDomain = async (domain: string) => {
    setDomainBusy(true);
    try {
      const updated = await removeCustomAiDomain(domain);
      setCustomDomains(updated);
      onStateUpdated?.();
      onToast?.("Домен удален", `${domain} исключен из пользовательского списка`, "info");
    } catch (err: any) {
      onToast?.("Ошибка удаления", err?.message || String(err), "warn");
    } finally {
      setDomainBusy(false);
    }
  };

  const handleCleanOsProxy = async () => {
    setCleaningProxy(true);
    setProxyCleanStatus(null);
    try {
      const results = await forceCleanOsProxy();
      const statusMsg =
        results.length > 0
          ? results.join(" · ")
          : "Системный прокси, реестр Windows и профили терминала сброшены";
      setProxyCleanStatus("Очищено: " + statusMsg);
      onToast?.(
        "Сетевые настройки сброшены",
        "Системные прокси-хвосты очищены. Доступ в интернет восстановлен.",
        "success"
      );
    } catch (err: any) {
      const errMsg = err?.message || String(err);
      setProxyCleanStatus("Ошибка сброса: " + errMsg);
      onToast?.("Ошибка сброса прокси", errMsg, "warn");
    } finally {
      setCleaningProxy(false);
    }
  };

  const handleOpenGitHub = async () => {
    await openExternal("https://github.com/kingbest1195/ai-antiban");
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="settings-dialog-title"
      className="fixed inset-0 z-50 bg-black/80 backdrop-blur-xs flex items-center justify-center p-3 sm:p-4 animate-in fade-in duration-150"
    >
      <div className="w-full max-w-[560px] bg-[#12141A] border border-white/[0.12] rounded-2xl shadow-2xl flex flex-col max-h-[90vh] select-none font-sans text-zinc-100 overflow-hidden">
        {/* Header */}
        <div className="flex items-start justify-between gap-2 px-5 py-3.5 border-b border-white/[0.08] shrink-0 bg-[#12141A]">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-xl bg-emerald-500/15 border border-emerald-500/30 text-emerald-400 flex items-center justify-center shrink-0">
              <Settings className="w-4 h-4" />
            </div>
            <div>
              <h2
                id="settings-dialog-title"
                className="text-xs font-bold text-white tracking-tight"
              >
                Настройки и параметры защиты
              </h2>
              <p className="text-[10px] text-zinc-400 font-mono">
                Zastava v1.3.0 · Конфигурация шлюза и приватность
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] cursor-pointer transition-colors"
            title="Закрыть"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Scrollable Body */}
        <div className="p-4 sm:p-5 overflow-y-auto space-y-4 max-h-[82vh] text-zinc-100 pr-3">
          {/* ============================================================ */}
          {/* Section 1: Режим защиты при сбое VPN (Паттерн поведения) */}
          {/* ============================================================ */}
          <div className="bg-[#0c0e14] rounded-xl p-3.5 border border-white/[0.06] space-y-3">
            <div className="space-y-0.5">
              <div className="flex items-center justify-between">
                <span className="text-xs font-bold text-white flex items-center gap-1.5">
                  <ShieldCheck className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                  1. Режим защиты при сбое VPN
                </span>
                <span className="text-[9.5px] font-mono text-emerald-400/90 font-semibold bg-emerald-500/10 px-1.5 py-0.5 rounded border border-emerald-500/20">
                  Паттерн поведения
                </span>
              </div>
              <p className="text-[10px] text-zinc-400 leading-relaxed">
                Выберите, как шлюз реагирует, если VPN отключился или трафик пошел через открытую сеть.
              </p>
            </div>

            <div className="grid grid-cols-1 gap-2 pt-1">
              {/* Option 1: ask_popup */}
              <div
                onClick={() => handleBehaviorChange("ask_popup")}
                className={`p-2.5 rounded-xl border transition-all cursor-pointer flex items-start gap-2.5 ${
                  behavior === "ask_popup"
                    ? "bg-emerald-500/10 border-emerald-500/50 shadow-xs shadow-emerald-500/10"
                    : "bg-[#141722]/60 border-white/[0.06] hover:border-white/[0.15]"
                }`}
              >
                <div className="pt-0.5 shrink-0">
                  <div
                    className={`w-4 h-4 rounded-full border flex items-center justify-center transition-all ${
                      behavior === "ask_popup"
                        ? "border-emerald-400 bg-emerald-500"
                        : "border-zinc-600 bg-zinc-900"
                    }`}
                  >
                    {behavior === "ask_popup" && (
                      <div className="w-1.5 h-1.5 rounded-full bg-zinc-950" />
                    )}
                  </div>
                </div>
                <div className="space-y-0.5 min-w-0 flex-1">
                  <div className="flex items-center gap-1.5">
                    <Bell className="w-3.5 h-3.5 text-sky-400 shrink-0" />
                    <span className="text-[11px] font-bold text-white">
                      Уведомление в углу экрана
                    </span>
                    <span className="text-[9px] font-mono font-semibold px-1 py-0.2 rounded bg-emerald-500/20 text-emerald-300 border border-emerald-500/30">
                      Рекомендуется
                    </span>
                  </div>
                  <p className="text-[10px] text-zinc-400 leading-normal">
                    Показать компактное окно в правом верхнем углу без перехвата фокуса клавиатуры
                  </p>

                  {/* Sub-toggle: Position switcher */}
                  {behavior === "ask_popup" && (
                    <div
                      onClick={(e) => e.stopPropagation()}
                      className="pt-2 mt-2 border-t border-emerald-500/20 flex flex-wrap items-center justify-between gap-2"
                    >
                      <span className="text-[9.5px] font-medium text-zinc-300">
                        Позиция окна:
                      </span>
                      <div className="inline-flex rounded-lg bg-zinc-950/80 p-0.5 border border-white/[0.08]">
                        <button
                          type="button"
                          onClick={() => handlePositionChange("top_right")}
                          className={`px-2 py-1 rounded-md text-[9.5px] font-medium transition-all cursor-pointer ${
                            position === "top_right"
                              ? "bg-emerald-500 text-zinc-950 font-bold shadow-xs"
                              : "text-zinc-400 hover:text-zinc-200"
                          }`}
                        >
                          Правый верхний угол
                        </button>
                        <button
                          type="button"
                          onClick={() => handlePositionChange("top_center")}
                          className={`px-2 py-1 rounded-md text-[9.5px] font-medium transition-all cursor-pointer ${
                            position === "top_center"
                              ? "bg-emerald-500 text-zinc-950 font-bold shadow-xs"
                              : "text-zinc-400 hover:text-zinc-200"
                          }`}
                        >
                          Сверху по центру
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              </div>

              {/* Option 2: auto_lock_silent */}
              <div
                onClick={() => handleBehaviorChange("auto_lock_silent")}
                className={`p-2.5 rounded-xl border transition-all cursor-pointer flex items-start gap-2.5 ${
                  behavior === "auto_lock_silent"
                    ? "bg-amber-500/10 border-amber-500/50 shadow-xs shadow-amber-500/10"
                    : "bg-[#141722]/60 border-white/[0.06] hover:border-white/[0.15]"
                }`}
              >
                <div className="pt-0.5 shrink-0">
                  <div
                    className={`w-4 h-4 rounded-full border flex items-center justify-center transition-all ${
                      behavior === "auto_lock_silent"
                        ? "border-amber-400 bg-amber-500"
                        : "border-zinc-600 bg-zinc-900"
                    }`}
                  >
                    {behavior === "auto_lock_silent" && (
                      <div className="w-1.5 h-1.5 rounded-full bg-zinc-950" />
                    )}
                  </div>
                </div>
                <div className="space-y-0.5 min-w-0 flex-1">
                  <div className="flex items-center gap-1.5">
                    <VolumeX className="w-3.5 h-3.5 text-amber-400 shrink-0" />
                    <span className="text-[11px] font-bold text-white">
                      Тихая авто-пауза (Без окон)
                    </span>
                  </div>
                  <p className="text-[10px] text-zinc-400 leading-normal">
                    При обрыве VPN молча ставить запросы к ИИ на паузу, не отвлекая всплывающими окнами
                  </p>
                </div>
              </div>

              {/* Option 3: auto_allow_trusted */}
              <div
                onClick={() => handleBehaviorChange("auto_allow_trusted")}
                className={`p-2.5 rounded-xl border transition-all cursor-pointer flex items-start gap-2.5 ${
                  behavior === "auto_allow_trusted"
                    ? "bg-sky-500/10 border-sky-500/50 shadow-xs shadow-sky-500/10"
                    : "bg-[#141722]/60 border-white/[0.06] hover:border-white/[0.15]"
                }`}
              >
                <div className="pt-0.5 shrink-0">
                  <div
                    className={`w-4 h-4 rounded-full border flex items-center justify-center transition-all ${
                      behavior === "auto_allow_trusted"
                        ? "border-sky-400 bg-sky-500"
                        : "border-zinc-600 bg-zinc-900"
                    }`}
                  >
                    {behavior === "auto_allow_trusted" && (
                      <div className="w-1.5 h-1.5 rounded-full bg-zinc-950" />
                    )}
                  </div>
                </div>
                <div className="space-y-0.5 min-w-0 flex-1">
                  <div className="flex items-center gap-1.5">
                    <ShieldCheck className="w-3.5 h-3.5 text-sky-400 shrink-0" />
                    <span className="text-[11px] font-bold text-white">
                      Авто-пропуск безопасных стран
                    </span>
                  </div>
                  <p className="text-[10px] text-zinc-400 leading-normal">
                    Разрешать работу автоматически, если новый IP находится в безопасной стране (NL, DE, US, FI)
                  </p>
                </div>
              </div>
            </div>
          </div>

          {/* ============================================================ */}
          {/* Section 2: Защищаемые ИИ-сервисы и свои домены */}
          {/* ============================================================ */}
          <div className="bg-[#0c0e14] rounded-xl p-3.5 border border-white/[0.06] space-y-3">
            <div className="space-y-0.5">
              <div className="flex items-center justify-between">
                <span className="text-xs font-bold text-white flex items-center gap-1.5">
                  <Sparkles className="w-3.5 h-3.5 text-sky-400 shrink-0" />
                  2. Защищаемые ИИ-сервисы и свои домены
                </span>
                <span className="text-[9.5px] font-mono text-sky-400 font-semibold bg-sky-500/10 px-1.5 py-0.5 rounded border border-sky-500/20">
                  Авто-матрица
                </span>
              </div>
              <p className="text-[10px] text-zinc-400 leading-relaxed">
                Трафик этих экосистем проверяется шлюзом. Обычные сайты (банки, YouTube, GitHub) идут напрямую на полной скорости.
              </p>
            </div>

            {/* Built-in Protected Ecosystems Badges */}
            <div className="space-y-1.5 pt-1">
              <span className="text-[9.5px] font-mono text-zinc-400 uppercase tracking-wider block">
                Базовая встроенная защита:
              </span>
              <div className="flex flex-wrap gap-1.5">
                {BUILTIN_PROTECTED_ECOSYSTEMS.map((eco) => (
                  <span
                    key={eco}
                    className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md bg-zinc-900/90 border border-white/[0.08] text-[10px] font-medium text-zinc-300"
                  >
                    <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shrink-0" />
                    {eco}
                  </span>
                ))}
              </div>
            </div>

            {/* Custom AI Domains input & removable pills */}
            <div className="space-y-2 pt-2 border-t border-white/[0.06]">
              <div className="flex items-center justify-between">
                <span className="text-[10px] font-mono text-zinc-300 uppercase tracking-wider">
                  Пользовательские домены ИИ:
                </span>
                <span className="text-[9.5px] font-mono text-zinc-400">
                  {customDomains.length > 0 ? `${customDomains.length} доп.` : "По умолчанию"}
                </span>
              </div>

              {/* Input Form */}
              <form onSubmit={handleAddDomain} className="flex items-center gap-2">
                <input
                  type="text"
                  value={newDomainInput}
                  onChange={(e) => setNewDomainInput(e.target.value)}
                  placeholder="Добавить свой ИИ-домен (например, custom-ai.com)"
                  disabled={domainBusy}
                  className="flex-1 bg-zinc-950 border border-white/[0.12] rounded-lg px-2.5 py-1.5 text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-sky-500/60 font-mono transition-colors disabled:opacity-50"
                />
                <button
                  type="submit"
                  disabled={domainBusy || !newDomainInput.trim()}
                  className="px-3 py-1.5 bg-sky-600 hover:bg-sky-500 active:scale-98 text-white rounded-lg text-xs font-semibold flex items-center gap-1 transition-all cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed shrink-0 shadow-xs"
                >
                  <Plus className="w-3.5 h-3.5" />
                  <span>Добавить</span>
                </button>
              </form>

              {/* Custom domains pill list */}
              {customDomains.length > 0 ? (
                <div className="flex flex-wrap gap-1.5 pt-1">
                  {customDomains.map((domain) => (
                    <span
                      key={domain}
                      className="inline-flex items-center gap-1.5 pl-2 pr-1 py-0.5 rounded-md bg-sky-950/50 border border-sky-500/30 text-[10.5px] font-mono text-sky-200"
                    >
                      <span>{domain}</span>
                      <button
                        type="button"
                        onClick={() => handleRemoveDomain(domain)}
                        disabled={domainBusy}
                        className="p-0.5 hover:bg-sky-500/20 rounded text-sky-400 hover:text-sky-100 cursor-pointer transition-colors"
                        title="Удалить домен"
                      >
                        <X className="w-3 h-3" />
                      </button>
                    </span>
                  ))}
                </div>
              ) : (
                <p className="text-[9.5px] text-zinc-400 italic">
                  Дополнительных доменов нет. Встроенного списка достаточно для всех популярных нейросетей.
                </p>
              )}
            </div>
          </div>

          {/* ============================================================ */}
          {/* Section 3: Сброс сетевых настроек (Анти-залипание прокси) */}
          {/* ============================================================ */}
          <div className="bg-[#0c0e14] rounded-xl p-3.5 border border-white/[0.06] space-y-2.5">
            <div className="space-y-0.5">
              <div className="flex items-center justify-between">
                <span className="text-xs font-bold text-white flex items-center gap-1.5">
                  <RefreshCw className="w-3.5 h-3.5 text-amber-400 shrink-0" />
                  3. Сброс сетевых настроек
                </span>
                <span className="text-[9.5px] font-mono text-amber-400/90 font-semibold bg-amber-500/10 px-1.5 py-0.5 rounded border border-amber-500/20">
                  Анти-залипание прокси
                </span>
              </div>
              <p className="text-[10px] text-zinc-400 leading-relaxed">
                Если после закрытия VPN или браузера пропал интернет, системные настройки прокси могли остаться активными. Нажмите кнопку для мгновенного сброса.
              </p>
            </div>

            <div className="pt-1 flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-2.5">
              <button
                type="button"
                onClick={handleCleanOsProxy}
                disabled={cleaningProxy}
                className="px-3 py-1.5 rounded-lg bg-zinc-900 hover:bg-zinc-800 active:scale-98 border border-white/[0.12] hover:border-amber-500/40 text-xs font-semibold text-zinc-200 hover:text-white flex items-center justify-center gap-1.5 transition-all cursor-pointer disabled:opacity-50 shrink-0"
              >
                <RefreshCw
                  className={`w-3.5 h-3.5 text-amber-400 ${
                    cleaningProxy ? "animate-spin" : ""
                  }`}
                />
                <span>Очистить системные прокси-хвосты</span>
              </button>

              {proxyCleanStatus && (
                <div className="text-[10px] font-mono text-emerald-300 bg-emerald-500/10 border border-emerald-500/20 rounded-md px-2 py-1 leading-tight flex items-center gap-1">
                  <Check className="w-3 h-3 text-emerald-400 shrink-0" />
                  <span className="truncate">{proxyCleanStatus}</span>
                </div>
              )}
            </div>
          </div>

          {/* ============================================================ */}
          {/* Section 4: Автообновление и сетевая прозрачность */}
          {/* ============================================================ */}
          <div className="space-y-3">
            {/* Auto-Update Toggle */}
            <div className="bg-[#0c0e14] rounded-xl p-3 border border-white/[0.06] space-y-2">
              <div className="flex items-center justify-between">
                <div className="space-y-0.5 pr-2">
                  <div className="text-xs font-semibold text-white">
                    Автоматическое обновление
                  </div>
                  <div className="text-[10px] text-zinc-400 leading-tight">
                    Фоновая проверка стабильных релизов в GitHub Releases
                  </div>
                </div>

                <button
                  type="button"
                  disabled={updatingToggle}
                  onClick={handleToggleAutoUpdate}
                  className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-hidden disabled:opacity-50 ${
                    autoUpdateEnabled ? "bg-emerald-500" : "bg-zinc-800"
                  }`}
                >
                  <span
                    className={`pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow-md ring-0 transition duration-200 ease-in-out ${
                      autoUpdateEnabled ? "translate-x-4" : "translate-x-0"
                    }`}
                  />
                </button>
              </div>

              <div className="text-[9.5px] font-mono text-zinc-400 flex items-center gap-1.5 pt-1 border-t border-white/[0.04]">
                <span
                  className={`w-1.5 h-1.5 rounded-full ${
                    autoUpdateEnabled ? "bg-emerald-400" : "bg-zinc-600"
                  }`}
                />
                <span>
                  Статус:{" "}
                  <strong
                    className={
                      autoUpdateEnabled
                        ? "text-emerald-300 font-bold"
                        : "text-zinc-500"
                    }
                  >
                    {autoUpdateEnabled ? "ВКЛЮЧЕНО (Рекомендуется)" : "ВЫКЛЮЧЕНО"}
                  </strong>
                </span>
              </div>
            </div>

            {/* 3-Point Network Transparency List */}
            <div className="space-y-1.5">
              <div className="flex items-center justify-between text-[10px] px-1">
                <span className="text-zinc-400 font-semibold uppercase tracking-wider">
                  Гарантии сетевой приватности
                </span>
                <span className="text-emerald-400 font-mono text-[9px] font-bold">
                  100% Локально
                </span>
              </div>

              <div className="bg-[#0c0e14] rounded-xl p-3 border border-white/[0.06] space-y-2.5 text-[10.5px]">
                {/* Point 1 */}
                <div className="flex items-start gap-2">
                  <div className="w-5 h-5 rounded bg-emerald-500/10 text-emerald-400 flex items-center justify-center shrink-0 mt-0.5">
                    <Lock className="w-3 h-3" />
                  </div>
                  <div className="space-y-0.5">
                    <div className="font-semibold text-zinc-200">
                      Сквозное TLS (Чаты зашифрованы)
                    </div>
                    <div className="text-[10px] text-zinc-400 leading-relaxed">
                      Застава не устанавливает корневые сертификаты и не читает переписку с ИИ. Трафик туннелируется в неизменном зашифрованном виде.
                    </div>
                  </div>
                </div>

                {/* Point 2 */}
                <div className="flex items-start gap-2 pt-2 border-t border-white/[0.04]">
                  <div className="w-5 h-5 rounded bg-sky-500/10 text-sky-400 flex items-center justify-center shrink-0 mt-0.5">
                    <Cpu className="w-3 h-3" />
                  </div>
                  <div className="space-y-0.5">
                    <div className="font-semibold text-zinc-200">
                      Куда Застава ходит в сеть
                    </div>
                    <div className="text-[10px] text-zinc-400 leading-relaxed space-y-1">
                      <div>
                        <span className="text-zinc-300">Проверка вашего IP и страны:</span> ipwho.is, ipinfo.io, 1.1.1.1 (Cloudflare), api.ipify.org. Уходит только запрос «какой у меня IP».
                      </div>
                      <div>
                        <span className="text-zinc-300">Спам-базы:</span> DNS-запросы к zen.spamhaus.org и аналогам. Передаётся только IP выхода.
                      </div>
                      <div>
                        <span className="text-zinc-300">Проверка карты (вкладка «Подписка»):</span> только по кнопке, первые 6 цифр BIN уходят на data.handyapi.com.
                      </div>
                      <div>
                        <span className="text-zinc-300">Обновления:</span> api.github.com. Отключаются переключателем выше.
                      </div>
                      <div className="text-zinc-500">
                        Ваши промпты, ключи и переписка никуда не отправляются. Аналитики и телеметрии нет.
                      </div>
                    </div>
                  </div>
                </div>

                {/* Point 3 */}
                <div className="flex items-start gap-2 pt-2 border-t border-white/[0.04]">
                  <div className="w-5 h-5 rounded bg-emerald-500/10 text-emerald-400 flex items-center justify-center shrink-0 mt-0.5">
                    <Globe className="w-3 h-3" />
                  </div>
                  <div className="space-y-0.5">
                    <div className="font-semibold text-zinc-200">
                      Селективный перехват (Только домены ИИ)
                    </div>
                    <div className="text-[10px] text-zinc-400 leading-relaxed">
                      Через шлюз идут только эндпоинты anthropic.com, openai.com, gemini.google.com и добавленные вами домены. Банки, стриминги, GitHub и рабочие сервисы идут напрямую на полной скорости.
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Footer Actions */}
        <div className="flex items-center justify-between gap-2 px-5 py-3 border-t border-white/[0.08] bg-[#12141A] shrink-0">
          <button
            type="button"
            onClick={handleOpenGitHub}
            className="px-2.5 py-1.5 rounded-lg bg-zinc-900 hover:bg-zinc-800 border border-white/[0.08] text-[10.5px] text-zinc-300 hover:text-white flex items-center gap-1.5 cursor-pointer transition-colors"
          >
            <span>GitHub Исходный код</span>
            <ExternalLink className="w-3 h-3 text-zinc-400" />
          </button>

          <button
            type="button"
            onClick={onClose}
            className="px-4 py-1.5 rounded-lg bg-zinc-100 hover:bg-white active:scale-98 text-zinc-950 font-bold text-xs cursor-pointer transition-colors shadow-xs"
          >
            Понятно
          </button>
        </div>
      </div>
    </div>
  );
};
