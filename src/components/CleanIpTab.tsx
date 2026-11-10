import React, { useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import {
  Key,
  Server,
  Radio,
  Play,
  Pause,
  Trash2,
  Send,
  Zap,
  ChevronRight,
  Sparkles,
  CheckCircle2,
  AlertTriangle,
  X,
} from "lucide-react";
import { AppStateData } from "../types";
import { clearUpstreamProxy, toggleUpstreamProxy } from "../utils/api";
import { openTelegramSupport } from "../utils/opener";

interface CleanIpTabProps {
  state: AppStateData | null;
  onRefreshState: () => Promise<void>;
  onAuditRefresh: () => Promise<void>;
  onOpenGuide: (guideId: string) => void;
  onToast: (
    title: string,
    message?: string,
    type?: "success" | "warning" | "error" | "info"
  ) => void;
}

export const CleanIpTab: React.FC<CleanIpTabProps> = ({
  state,
  onRefreshState,
  onAuditRefresh,
  onOpenGuide,
  onToast,
}) => {
  const [inputKey, setInputKey] = useState<string>("");
  const [toggling, setToggling] = useState<boolean>(false);
  const [showSafeguardModal, setShowSafeguardModal] = useState<boolean>(false);

  const upstreamProxy = state?.upstream_proxy;
  const isConfigured = !!upstreamProxy;
  const isActive = upstreamProxy?.enabled ?? false;

  const handleConnect = (_keyToUse?: string) => {
    setShowSafeguardModal(true);
    onToast(
      "Функция «Чистый IP» будет доступна в следующем патче",
      "Сейчас мы завершаем разработку официального бота автоматической выдачи чистых выделенных IP-адресов. Ручная привязка сторонних прокси временно отключена, чтобы некорректный конфиг не нарушил работу сетевого шлюза Заставы. Пока используйте свой личный VPN по инструкции во вкладке «Свой VPN».",
      "warning"
    );
  };

  const handleToggle = async () => {
    if (!upstreamProxy) return;
    setToggling(true);
    try {
      const nextState = !upstreamProxy.enabled;
      await toggleUpstreamProxy(nextState);
      await onRefreshState();
      await onAuditRefresh();
      onToast(
        nextState ? "Шлюз включен" : "Шлюз на паузе",
        nextState
          ? "Трафик ИИ снова идет через выделенный чистый IP"
          : "Трафик идет напрямую через вашу сеть",
        "info"
      );
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      onToast("Ошибка переключения", msg, "error");
    } finally {
      setToggling(false);
    }
  };

  const handleDisconnect = async () => {
    try {
      await clearUpstreamProxy();
      await onRefreshState();
      await onAuditRefresh();
      onToast("Ключ отключен", "Настройки шлюза очищены. Все запросы идут напрямую.", "info");
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      onToast("Ошибка отключения", msg, "error");
    }
  };

  const handleUseDemoKey = () => {
    const demo = "socks5://u20473:demo_pass@185.10.10.1:1080#Zastava 2-Hop (NL)";
    setInputKey(demo);
    handleConnect(demo);
  };

  return (
    <div className="space-y-3.5 animate-in fade-in duration-150">
      {/* Safeguard Alert Banner: Clean IP Bot in development */}
      <div className="bg-amber-500/10 border border-amber-500/30 rounded-xl p-3.5 space-y-2">
        <div className="flex items-center gap-2 text-amber-300 font-bold text-xs">
          <AlertTriangle className="w-4 h-4 text-amber-400 shrink-0" />
          <span>Функция «Чистый IP» будет доступна в следующем патче</span>
        </div>
        <p className="text-[11px] text-zinc-300 leading-relaxed font-sans">
          Сейчас мы завершаем разработку официального бота автоматической выдачи чистых выделенных IP-адресов. Ручная привязка сторонних прокси временно отключена, чтобы некорректный конфиг не нарушил работу сетевого шлюза Заставы. Пока используйте свой личный VPN по инструкции во вкладке «Свой VPN».
        </p>
        <div className="pt-0.5">
          <button
            type="button"
            onClick={() => onOpenGuide("01-personal-vpn-15min")}
            className="px-2.5 py-1.5 rounded-lg bg-amber-500/20 hover:bg-amber-500/30 border border-amber-500/40 text-[10.5px] font-semibold text-amber-200 flex items-center gap-1.5 cursor-pointer transition-colors"
          >
            <Zap className="w-3 h-3 text-amber-400" />
            <span>Инструкция: Свой VPN за 15 минут</span>
          </button>
        </div>
      </div>

      {/* Block 1: Zastava Gateway Key */}
      <section className="bg-[#12141A] rounded-xl border border-white/[0.08] p-3.5 space-y-3 shadow-sm">
        <div className="flex items-center justify-between pb-2 border-b border-white/[0.05]">
          <div className="flex items-center gap-2">
            <div className="w-6 h-6 rounded-lg bg-emerald-500/10 border border-emerald-500/25 flex items-center justify-center text-emerald-400">
              <Key className="w-3.5 h-3.5" strokeWidth={1.8} />
            </div>
            <div>
              <h2 className="text-xs font-bold text-white tracking-tight">
                Ключ Заставы (2-Hop Gateway)
              </h2>
              <span className="text-[10px] text-zinc-400 font-mono">
                Чистый статический IP только для ИИ
              </span>
            </div>
          </div>

          {isConfigured && (
            <span
              className={`px-2 py-0.5 rounded-full text-[9.5px] font-mono font-bold border tracking-tight flex items-center gap-1 ${
                isActive
                  ? "bg-emerald-500/15 border-emerald-500/30 text-emerald-300"
                  : "bg-amber-500/15 border-amber-500/30 text-amber-300"
              }`}
            >
              <Radio className={`w-2.5 h-2.5 ${isActive ? "animate-pulse" : ""}`} />
              <span>{isActive ? "АКТИВЕН" : "НА ПАУЗЕ"}</span>
            </span>
          )}
        </div>

        {/* Plain Russian explanation for developers and vibe-coders */}
        <p className="text-[11px] text-zinc-300 leading-relaxed font-sans">
          Направляет трафик <strong className="text-zinc-100 font-semibold">Claude, ChatGPT, Gemini и Cursor</strong> через
          чистый выделенный IP (Нидерланды / Германия / США). Весь остальной интернет на компьютере (банки РФ, рабочие
          сайты, загрузки) работает напрямую на максимальной скорости.
        </p>

        {isConfigured && upstreamProxy ? (
          /* Active Gateway Status Card */
          <div className="bg-zinc-950/70 rounded-lg border border-white/[0.06] p-3 space-y-2.5">
            <div className="flex items-start justify-between gap-2">
              <div className="space-y-0.5 min-w-0">
                <div className="flex items-center gap-1.5">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
                  <span className="text-xs font-bold text-white truncate">
                    {upstreamProxy.label || "Выделенный шлюз Заставы"}
                  </span>
                </div>
                <div className="text-[10px] font-mono text-zinc-400 flex items-center gap-2 pt-0.5">
                  <span className="px-1.5 py-0.2 rounded bg-zinc-900 border border-white/[0.07] text-zinc-300 font-semibold uppercase">
                    {upstreamProxy.protocol}
                  </span>
                  <span className="truncate">
                    {upstreamProxy.host}:{upstreamProxy.port}
                  </span>
                </div>
              </div>

              <div className="flex items-center gap-1.5 shrink-0">
                <button
                  type="button"
                  disabled={toggling}
                  onClick={handleToggle}
                  className={`flex items-center gap-1 px-2.5 py-1 rounded-md text-[10.5px] font-semibold border transition-all cursor-pointer ${
                    isActive
                      ? "bg-amber-500/15 hover:bg-amber-500/25 border-amber-500/30 text-amber-300"
                      : "bg-emerald-500/15 hover:bg-emerald-500/25 border-emerald-500/30 text-emerald-300"
                  }`}
                  title={isActive ? "Поставить шлюз на паузу" : "Включить шлюз"}
                >
                  {isActive ? (
                    <>
                      <Pause className="w-3 h-3" />
                      <span>Пауза</span>
                    </>
                  ) : (
                    <>
                      <Play className="w-3 h-3" />
                      <span>Включить</span>
                    </>
                  )}
                </button>

                <button
                  type="button"
                  onClick={handleDisconnect}
                  className="p-1 rounded-md bg-zinc-900 hover:bg-zinc-800 border border-white/[0.08] text-zinc-400 hover:text-rose-400 cursor-pointer transition-colors"
                  title="Отключить ключ и очистить настройки"
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>

            <div className="pt-2 border-t border-white/[0.04] flex items-center justify-between text-[10px] text-zinc-400 font-mono">
              <span className="flex items-center gap-1 text-emerald-400">
                <CheckCircle2 className="w-3 h-3" />
                <span>Защита сессии активна</span>
              </span>
              <span className="text-zinc-500">2-Hop · без чтения чатов</span>
            </div>
          </div>
        ) : (
          /* Input Field for connecting new key */
          <div className="space-y-2">
            <div className="flex items-center gap-1.5">
              <div className="relative flex-1">
                <input
                  type="text"
                  value={inputKey}
                  onChange={(e) => setInputKey(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && handleConnect()}
                  placeholder="zastava://... или socks5://user:pass@host:port#Label"
                  className="w-full pl-3 pr-2 py-2 bg-zinc-950/80 rounded-lg border border-white/[0.08] focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/30 text-[11px] font-mono text-zinc-100 placeholder:text-zinc-600 outline-none transition-all"
                />
              </div>

              <button
                type="button"
                onClick={() => handleConnect()}
                className="px-3.5 py-2 rounded-lg bg-amber-500 hover:bg-amber-400 text-zinc-950 font-bold text-xs flex items-center gap-1.5 cursor-pointer transition-colors shrink-0 shadow-sm"
              >
                <Key className="w-3 h-3" />
                <span>Подключить</span>
              </button>
            </div>

            {!isTauri() && (
              <div className="flex items-center justify-between pt-0.5 text-[10px]">
                <span className="text-zinc-500">Режим браузерного превью:</span>
                <button
                  type="button"
                  onClick={handleUseDemoKey}
                  className="text-amber-400 hover:text-amber-300 font-mono font-medium hover:underline cursor-pointer flex items-center gap-1"
                >
                  <Sparkles className="w-2.5 h-2.5" />
                  <span>Тестовый ключ (NL 2-Hop)</span>
                </button>
              </div>
            )}
          </div>
        )}

        {/* Clean Telegram Helper Card */}
        <div className="rounded-lg bg-sky-500/5 border border-sky-500/20 p-2.5 flex items-center justify-between gap-2.5">
          <div className="space-y-0.5 min-w-0">
            <div className="text-[11px] font-semibold text-sky-200">
              Нужен готовый чистый IP для Claude и ChatGPT?
            </div>
            <div className="text-[10px] text-zinc-400 leading-snug">
              Статический сервер в Нидерландах без спам-баз и с гарантией разблокировки.
            </div>
          </div>
          <button
            type="button"
            onClick={() => openTelegramSupport()}
            className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg bg-sky-500 hover:bg-sky-400 text-zinc-950 font-bold text-[10.5px] cursor-pointer transition-colors shrink-0"
          >
            <Send className="w-3 h-3 text-zinc-950" />
            <span>Получить ключ</span>
          </button>
        </div>
      </section>

      {/* Block 2: Personal VPS for Free */}
      <section className="bg-[#12141A] rounded-xl border border-white/[0.08] p-3.5 space-y-2.5 shadow-sm">
        <div className="flex items-center gap-2">
          <div className="w-6 h-6 rounded-lg bg-zinc-800 border border-white/[0.08] flex items-center justify-center text-zinc-300">
            <Server className="w-3.5 h-3.5" strokeWidth={1.8} />
          </div>
          <div>
            <h2 className="text-xs font-bold text-white tracking-tight">
              Свой личный VPN за 15 минут (Бесплатно)
            </h2>
            <span className="text-[10px] text-zinc-400 font-mono">
              100% контроль · Оплата картой РФ / СБП
            </span>
          </div>
        </div>

        <p className="text-[11px] text-zinc-300 leading-relaxed font-sans">
          Застава полностью бесплатна и поддерживает подключение к вашему собственному серверу (VPS). Вы получаете личный
          чистый IP, который не делит никто другой.
        </p>

        <div className="grid grid-cols-3 gap-1.5 pt-0.5 text-center text-[10px] font-mono">
          <div className="p-1.5 rounded-md bg-zinc-900/60 border border-white/[0.05] text-zinc-300">
            <div className="text-emerald-400 font-bold">HostVDS</div>
            <div className="text-[9px] text-zinc-500">от ~190 ₽/мес</div>
          </div>
          <div className="p-1.5 rounded-md bg-zinc-900/60 border border-white/[0.05] text-zinc-300">
            <div className="text-sky-400 font-bold">is*hosting</div>
            <div className="text-[9px] text-zinc-500">IPv4/IPv6 NL</div>
          </div>
          <div className="p-1.5 rounded-md bg-zinc-900/60 border border-white/[0.05] text-zinc-300">
            <div className="text-indigo-400 font-bold">Timeweb</div>
            <div className="text-[9px] text-zinc-500">Оплата СБП / Мир</div>
          </div>
        </div>

        <button
          type="button"
          onClick={() => onOpenGuide("01-personal-vpn-15min")}
          className="w-full py-2 px-3 rounded-lg bg-zinc-900 hover:bg-zinc-800 border border-white/[0.08] hover:border-white/[0.15] text-zinc-200 hover:text-white text-xs font-semibold flex items-center justify-between cursor-pointer transition-all group"
        >
          <div className="flex items-center gap-1.5">
            <Zap className="w-3.5 h-3.5 text-emerald-400" />
            <span>Открыть пошаговую инструкцию по настройке VPS</span>
          </div>
          <ChevronRight className="w-3.5 h-3.5 text-zinc-400 group-hover:translate-x-0.5 transition-transform" />
        </button>
      </section>

      {/* Safeguard Modal */}
      {showSafeguardModal && (
        <div className="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="max-w-md w-full bg-[#12141A] border border-amber-500/40 rounded-xl p-4 space-y-3.5 shadow-2xl animate-in fade-in zoom-in-95 duration-150">
            <div className="flex items-start justify-between gap-2 border-b border-white/[0.08] pb-3">
              <div className="flex items-center gap-2">
                <div className="w-7 h-7 rounded-lg bg-amber-500/20 text-amber-400 flex items-center justify-center shrink-0">
                  <AlertTriangle className="w-4 h-4" />
                </div>
                <h3 className="text-xs font-bold text-white leading-tight">
                  Функция «Чистый IP» будет доступна в следующем патче
                </h3>
              </div>
              <button
                type="button"
                onClick={() => setShowSafeguardModal(false)}
                className="p-1 rounded text-zinc-400 hover:text-white cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <p className="text-[11.5px] text-zinc-300 leading-relaxed font-sans">
              Сейчас мы завершаем разработку официального бота автоматической выдачи чистых выделенных IP-адресов. Ручная привязка сторонних прокси временно отключена, чтобы некорректный конфиг не нарушил работу сетевого шлюза Заставы. Пока используйте свой личный VPN по инструкции во вкладке «Свой VPN».
            </p>

            <div className="flex items-center justify-end gap-2 pt-1 border-t border-white/[0.06]">
              <button
                type="button"
                onClick={() => {
                  setShowSafeguardModal(false);
                  onOpenGuide("01-personal-vpn-15min");
                }}
                className="px-3 py-1.5 rounded-lg bg-emerald-500 hover:bg-emerald-400 text-zinc-950 font-bold text-xs flex items-center gap-1.5 cursor-pointer transition-colors"
              >
                <Zap className="w-3 h-3 text-zinc-950" />
                <span>Инструкция: Свой VPN</span>
              </button>
              <button
                type="button"
                onClick={() => setShowSafeguardModal(false)}
                className="px-3 py-1.5 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-300 hover:text-white font-medium text-xs cursor-pointer transition-colors"
              >
                Понятно
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
