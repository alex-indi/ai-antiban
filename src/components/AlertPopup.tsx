import React, { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "@tauri-apps/api/core";
import {
  Shield,
  ShieldX,
  Zap,
  CheckCircle2,
  ArrowRight,
  Clock,
  X,
} from "lucide-react";
import { InterceptionAction, RiskInterceptPayload } from "../types";
import { resolveInterception } from "../utils/api";

const DEFAULT_PAYLOAD: RiskInterceptPayload = {
  id: "sim-preview",
  is_simulation: true,
  provider: "anthropic",
  host: "api.anthropic.com:443",
  ttl_seconds: 60,
  network_change: {
    old_ip: "185.212.102.40",
    old_country: "NL",
    new_ip: "185.220.101.5",
    new_country: "RU",
  },
  report: {
    provider: "anthropic",
    overall_status: "red",
    trust_score: 28,
    telemetry: {
      ip: "185.220.101.5",
      country_code: "RU",
      asn: 9009,
      isp: "M247 Europe SRL",
      org: "M247 Europe SRL",
      timezone: "Europe/Moscow",
      resolved_at: Math.floor(Date.now() / 1000),
    },
    geo_result: {
      status: "critical",
      code: "GEO_HIGH_RISK_RU",
      message: "Локация RU не поддерживается сервисом ИИ.",
      guide_ref: "01-personal-vpn-15min",
    },
    pinning_result: {
      status: "warning",
      code: "PINNED_IP_NOT_CONFIGURED",
      message: "Трафик идет НЕ через закрепленный VPS.",
      guide_ref: "01-personal-vpn-15min",
    },
    travel_result: {
      status: "ok",
      code: "TRAVEL_NORMAL",
      message: "Конфликтов перемещения за 12ч нет.",
      guide_ref: null,
    },
    dnsbl_result: {
      status: "ok",
      code: "DNSBL_CLEAN",
      message: "В спам-базах не числится.",
      guide_ref: null,
    },
    dnsbl_clean_count: "4/4 Чисто",
    asn_result: {
      status: "ok",
      code: "ASN_CLEAN",
      message: "Чистая автономная система.",
      guide_ref: null,
    },
    ipv6_result: {
      status: "ok",
      code: "IPV6_SAFE",
      message: "Утечек IPv6 нет.",
      guide_ref: null,
    },
    timezone_result: {
      status: "ok",
      code: "TIMEZONE_OK",
      message: "Часовой пояс согласован.",
      guide_ref: null,
    },
    checks: [],
  },
};

interface AlertPopupProps {
  initialPayload?: RiskInterceptPayload;
  onResolved?: (action?: InterceptionAction) => void;
}

export const AlertPopup: React.FC<AlertPopupProps> = ({
  initialPayload,
  onResolved,
}) => {
  const [payload, setPayload] = useState<RiskInterceptPayload>(
    initialPayload || DEFAULT_PAYLOAD
  );
  const [secondsRemaining, setSecondsRemaining] = useState<number>(
    initialPayload?.ttl_seconds || 60
  );
  const [resolving, setResolving] = useState<boolean>(false);
  const [demoFeedback, setDemoFeedback] = useState<string | null>(null);

  const isDemo = Boolean(payload.is_simulation || payload.id.startsWith("sim-"));

  useEffect(() => {
    let unlistenFn: (() => void) | undefined;

    if (isTauri()) {
      listen<RiskInterceptPayload>("interceptor://risk-detected", (event) => {
        setPayload(event.payload);
        setSecondsRemaining(event.payload.ttl_seconds || 60);
      }).then((unlisten) => {
        unlistenFn = unlisten;
      });
    }

    const handleCustomSim = (e: any) => {
      if (e.detail) {
        setPayload({
          ...DEFAULT_PAYLOAD,
          id: e.detail.id || "sim-" + Date.now(),
          is_simulation: true,
          provider: e.detail.provider || "anthropic",
          host: e.detail.host || "api.anthropic.com:443",
          network_change: {
            old_ip: "185.212.102.40",
            old_country: "NL",
            new_ip: "185.220.101.5",
            new_country: "RU",
          },
        });
        setSecondsRemaining(60);
        setDemoFeedback(null);
      }
    };
    window.addEventListener("simulate-interceptor-event", handleCustomSim);

    return () => {
      if (unlistenFn) unlistenFn();
      window.removeEventListener("simulate-interceptor-event", handleCustomSim);
    };
  }, []);

  useEffect(() => {
    if (secondsRemaining <= 0) {
      if (isDemo) {
        resolveInterception(
          payload.id,
          "BlockUntilNetworkChange",
          payload.provider,
          "simulation"
        ).catch(() => {});
        if (onResolved) onResolved("BlockUntilNetworkChange");
      } else {
        handleAction("BlockUntilNetworkChange");
      }
      return;
    }

    const timer = setInterval(() => {
      setSecondsRemaining((prev) => (prev > 0 ? prev - 1 : 0));
    }, 1000);

    return () => clearInterval(timer);
  }, [secondsRemaining, isDemo]);

  const handleAction = async (action: InterceptionAction, immediate: boolean = false) => {
    if (resolving) return;
    setResolving(true);

    if (isDemo) {
      if (immediate) {
        try {
          await resolveInterception(
            payload.id,
            action,
            payload.provider,
            "simulation"
          );
        } catch (_) {}
        if (onResolved) onResolved(action);
        setResolving(false);
        return;
      }

      let feedbackText = "Демо-режим: в реальной ситуации шлюз остановил бы трафик до включения VPN";
      if (action === "AllowSession") {
        feedbackText = "Демо-режим: сессия была бы разрешена";
      } else if (action === "OpenGuideAndBlock") {
        feedbackText = "Демо-режим: в реальной ситуации шлюз остановил бы трафик до включения VPN";
      }

      setDemoFeedback(feedbackText);

      setTimeout(async () => {
        try {
          await resolveInterception(
            payload.id,
            action,
            payload.provider,
            "simulation"
          );
        } catch (err) {
          console.error("Failed to resolve demo interception:", err);
        } finally {
          if (onResolved) onResolved(action);
          setResolving(false);
        }
      }, 900);
      return;
    }

    try {
      await resolveInterception(
        payload.id,
        action,
        payload.provider,
        payload.scope
      );
      if (onResolved) onResolved(action);
    } catch (err) {
      console.error("Failed to resolve interception:", err);
      if (onResolved) onResolved(action);
    } finally {
      setResolving(false);
    }
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (resolving) return;
      if (e.key === "1") {
        e.preventDefault();
        handleAction("OpenGuideAndBlock");
      } else if (e.key === "2") {
        e.preventDefault();
        handleAction("AllowSession");
      } else if (e.key === "3" || e.key === "Escape") {
        e.preventDefault();
        handleAction("BlockUntilNetworkChange", e.key === "Escape");
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [payload, resolving, demoFeedback, isDemo]);

  const { report, network_change } = payload;
  const previousIp = network_change?.old_ip || (payload as any).previous_ip;
  const previousCountry =
    network_change?.old_country || (payload as any).previous_country || "VPN";
  const currentIp =
    network_change?.new_ip || report?.telemetry?.ip || "Неизвестный IP";
  const currentCountry =
    network_change?.new_country || report?.telemetry?.country_code || "??";
  const targetHost = (payload.host || "api.anthropic.com").split(":")[0];

  const explanation = previousIp
    ? "Обнаружен обрыв VPN или резкая смена страны. Застава поставила запросы к ИИ на паузу, чтобы ваш аккаунт не получил бан."
    : currentCountry === "RU" || currentCountry === "BY"
    ? "Обнаружен неподдерживаемый регион подключения. Запрос к ИИ остановлен, чтобы аккаунт не был заблокирован."
    : "Обнаружен риск для безопасности сессии ИИ. Застава заморозила запрос до возврата в доверенную сеть.";

  return (
    <div
      className={`w-[432px] mx-auto rounded-b-[24px] bg-[#07080d]/95 backdrop-blur-2xl border-x border-b shadow-[0_20px_50px_rgba(0,0,0,0.85)] px-4 pt-2.5 pb-3.5 animate-notch-drop select-none overflow-hidden space-y-2.5 text-zinc-100 font-sans ${
        isDemo ? "border-sky-500/40" : "border-rose-500/40"
      }`}
    >
      {/* 1. Top Notch Pill Bar */}
      <div className="flex items-center justify-between pb-2 border-b border-white/[0.08]">
        {isDemo ? (
          <div className="flex items-center gap-1.5 min-w-0">
            <span className="w-1.5 h-1.5 rounded-full bg-amber-400 shrink-0" />
            <Shield className="w-3.5 h-3.5 text-sky-400 shrink-0" strokeWidth={2.2} />
            <span className="text-[10px] font-bold uppercase tracking-wider text-sky-200 truncate font-mono">
              ДЕМОНСТРАЦИЯ · СЕТЬ НЕ ЗАБЛОКИРОВАНА
            </span>
          </div>
        ) : (
          <div className="flex items-center gap-1.5 min-w-0">
            <span className="relative flex h-2 w-2 shrink-0">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-rose-400 opacity-75" />
              <span className="relative inline-flex rounded-full h-2 w-2 bg-rose-500" />
            </span>
            <Shield className="w-3.5 h-3.5 text-rose-400 shrink-0" strokeWidth={2.2} />
            <span className="text-[10px] font-bold uppercase tracking-wider text-rose-300 truncate">
              ТРАФИК ИИ ЗАМОРОЖЕН · УТЕЧКА ОСТАНОВЛЕНА
            </span>
          </div>
        )}

        <div className="flex items-center gap-1.5 shrink-0 ml-2">
          <div
            className={`flex items-center gap-1 px-2 py-0.5 rounded-full border text-[10px] font-mono font-bold tabular-nums ${
              isDemo
                ? "bg-sky-500/15 border-sky-500/30 text-sky-300"
                : "bg-rose-500/15 border-rose-500/30 text-rose-300"
            }`}
          >
            <Clock className="w-2.5 h-2.5 shrink-0" />
            <span>{secondsRemaining}с</span>
          </div>

          <button
            type="button"
            onClick={() => {
              handleAction("BlockUntilNetworkChange", true);
              if (onResolved) onResolved("BlockUntilNetworkChange");
            }}
            disabled={resolving}
            title="Закрыть окно (ИИ останется на безопасной паузе до включения VPN)"
            className="p-1 rounded-md text-zinc-400 hover:text-white hover:bg-white/[0.08] active:scale-95 transition-all cursor-pointer disabled:opacity-40"
            aria-label="Закрыть окно"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* 2. Visual Route Change Pill */}
      <div className="space-y-1.5">
        <div className="bg-[#11131c]/90 rounded-xl px-3 py-1.5 border border-white/[0.08] flex items-center justify-between text-[11px] font-mono shadow-inner">
          {previousIp ? (
            <>
              <div className="flex items-center gap-1.5 min-w-0">
                <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shrink-0" />
                <span className="text-zinc-400 text-[10px] shrink-0 font-mono">Был:</span>
                <span className="text-emerald-300 font-bold truncate">
                  {previousIp} ({previousCountry})
                </span>
              </div>
              <ArrowRight className="w-3.5 h-3.5 text-zinc-500 shrink-0 mx-2" />
              <div className="flex items-center gap-1.5 min-w-0">
                <span className="w-1.5 h-1.5 rounded-full bg-rose-500 shrink-0 animate-pulse" />
                <span className="text-zinc-400 text-[10px] shrink-0 font-mono">Стал:</span>
                <span className="text-rose-300 font-bold truncate">
                  {currentIp} ({currentCountry})
                </span>
              </div>
            </>
          ) : (
            <>
              <div className="flex items-center gap-1.5 min-w-0">
                <span className="text-zinc-400 text-[10px] shrink-0 font-mono">Цель:</span>
                <span className="text-zinc-200 font-semibold truncate font-mono">
                  {targetHost}
                </span>
              </div>
              <ArrowRight className="w-3.5 h-3.5 text-zinc-500 shrink-0 mx-2" />
              <div className="flex items-center gap-1.5 min-w-0">
                <span className="w-1.5 h-1.5 rounded-full bg-rose-500 shrink-0 animate-pulse" />
                <span className="text-zinc-400 text-[10px] shrink-0 font-mono">Выход:</span>
                <span className="text-rose-300 font-bold truncate font-mono">
                  {currentIp} ({currentCountry})
                </span>
              </div>
            </>
          )}
        </div>

        {demoFeedback ? (
          <div className="p-2 rounded-xl bg-sky-950/70 border border-sky-500/40 text-center animate-in fade-in duration-150">
            <div className="text-[10.5px] font-medium text-sky-200 leading-tight">
              {demoFeedback}
            </div>
          </div>
        ) : (
          <p className="text-[10.5px] text-zinc-300 font-sans leading-tight text-center px-1">
            {explanation}
          </p>
        )}
      </div>

      {/* 3. Action Buttons */}
      <div className="space-y-1.5 pt-0.5">
        <button
          type="button"
          disabled={resolving || Boolean(demoFeedback)}
          onClick={() => handleAction("OpenGuideAndBlock")}
          className="w-full py-2 px-3 rounded-xl bg-emerald-500 hover:bg-emerald-400 active:scale-[0.99] text-zinc-950 font-sans font-bold text-xs flex items-center justify-between transition-all shadow-md shadow-emerald-500/20 cursor-pointer disabled:opacity-50"
        >
          <div className="flex items-center gap-1.5 min-w-0">
            <Zap className="w-3.5 h-3.5 fill-zinc-950 shrink-0" strokeWidth={2} />
            <span className="truncate">1. Открыть инструкцию: Свой VPN</span>
          </div>
          <span className="text-[9.5px] font-mono bg-zinc-950/20 px-1.5 py-0.5 rounded text-zinc-900 shrink-0 font-bold">
            [1]
          </span>
        </button>

        <div className="grid grid-cols-2 gap-2">
          <button
            type="button"
            disabled={resolving || Boolean(demoFeedback)}
            onClick={() => handleAction("BlockUntilNetworkChange")}
            className="py-1.5 px-2 rounded-xl bg-[#12141c] hover:bg-[#181b24] active:scale-[0.99] border border-rose-500/35 hover:border-rose-500/55 text-rose-300 text-[10.5px] font-sans font-semibold flex items-center justify-center gap-1.5 transition-all cursor-pointer disabled:opacity-50 truncate"
          >
            <ShieldX className="w-3 h-3 shrink-0 text-rose-400" strokeWidth={2} />
            <span className="truncate">3. Блокировать до VPN</span>
          </button>

          <button
            type="button"
            disabled={resolving || Boolean(demoFeedback)}
            onClick={() => handleAction("AllowSession")}
            className="py-1.5 px-2 rounded-xl bg-[#12141c] hover:bg-[#181b24] active:scale-[0.99] border border-white/[0.1] hover:border-white/[0.2] text-zinc-300 hover:text-white text-[10.5px] font-sans font-medium flex items-center justify-center gap-1.5 transition-all cursor-pointer disabled:opacity-50 truncate"
          >
            <CheckCircle2 className="w-3 h-3 shrink-0 text-zinc-400" strokeWidth={2} />
            <span className="truncate">2. Разрешить сессию</span>
          </button>
        </div>
      </div>
    </div>
  );
};
