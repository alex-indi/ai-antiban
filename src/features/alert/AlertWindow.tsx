import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "@tauri-apps/api/core";
import type { InterceptionAction, RiskInterceptPayload } from "../../types";
import { resolveInterception } from "../../utils/api";
import { Dialog } from "../../ui";
import { alertDialog as copy, providers as providerCopy } from "../../shared/copy/ru";
import { countryName } from "../../shared/lib/country";
import "./alert.css";

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

export interface AlertWindowProps {
  initialPayload?: RiskInterceptPayload;
  onResolved?: (action?: InterceptionAction) => void;
}

/**
 * Диалог обрыва VPN (окно Tauri alert-popup и оверлей в браузере).
 * Клавиши прежние по смыслу: 1 — открыть инструкцию и держать блок, 2 — разрешить на сессию, 3 или Esc — блокировать до смены сети.
 * Теперь порядок кнопок на экране совпадает с номерами.
 */
export function AlertWindow({ initialPayload, onResolved }: AlertWindowProps) {
  const [payload, setPayload] = useState<RiskInterceptPayload>(initialPayload || DEFAULT_PAYLOAD);
  /** Меняется при каждом новом событии: Dialog пересоздаётся и таймер стартует заново. */
  const [epoch, setEpoch] = useState<number>(0);
  const [resolving, setResolving] = useState<boolean>(false);
  const [demoFeedback, setDemoFeedback] = useState<string | null>(null);

  const isDemo = Boolean(payload.is_simulation || payload.id.startsWith("sim-"));
  const ttl = payload.ttl_seconds || 60;

  useEffect(() => {
    let unlistenFn: (() => void) | undefined;
    let cancelled = false;

    if (isTauri()) {
      listen<RiskInterceptPayload>("interceptor://risk-detected", (event) => {
        setPayload(event.payload);
        setEpoch((n) => n + 1);
      }).then((unlisten) => {
        if (cancelled) unlisten();
        else unlistenFn = unlisten;
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
        setEpoch((n) => n + 1);
        setDemoFeedback(null);
      }
    };
    window.addEventListener("simulate-interceptor-event", handleCustomSim);

    return () => {
      cancelled = true;
      if (unlistenFn) unlistenFn();
      window.removeEventListener("simulate-interceptor-event", handleCustomSim);
    };
  }, []);

  const handleAction = useCallback(
    async (action: InterceptionAction, immediate: boolean = false) => {
      if (resolving) return;
      setResolving(true);

      if (isDemo) {
        if (immediate) {
          try {
            await resolveInterception(payload.id, action, payload.provider, "simulation");
          } catch (_) {}
          if (onResolved) onResolved(action);
          setResolving(false);
          return;
        }

        setDemoFeedback(action === "AllowSession" ? copy.feedbackAllow : copy.feedbackDefault);

        setTimeout(async () => {
          try {
            await resolveInterception(payload.id, action, payload.provider, "simulation");
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
        await resolveInterception(payload.id, action, payload.provider, payload.scope);
        if (onResolved) onResolved(action);
      } catch (err) {
        console.error("Failed to resolve interception:", err);
        if (onResolved) onResolved(action);
      } finally {
        setResolving(false);
      }
    },
    [resolving, isDemo, payload, onResolved],
  );

  // Таймер дошёл до нуля: блокируем до смены сети.
  const handleTimeout = useCallback(() => {
    if (isDemo) {
      resolveInterception(payload.id, "BlockUntilNetworkChange", payload.provider, "simulation").catch(() => {});
      if (onResolved) onResolved("BlockUntilNetworkChange");
    } else {
      handleAction("BlockUntilNetworkChange");
    }
  }, [isDemo, payload, onResolved, handleAction]);

  const handleClose = () => {
    handleAction("BlockUntilNetworkChange", true);
    if (onResolved) onResolved("BlockUntilNetworkChange");
  };

  // Esc — то же, что закрытие окна. Клавиши 1, 2, 3 обрабатывает сам Dialog.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (resolving || e.key !== "Escape") return;
      e.preventDefault();
      handleAction("BlockUntilNetworkChange", true);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [resolving, handleAction]);

  const { report, network_change } = payload;
  const previousIp = network_change?.old_ip || (payload as any).previous_ip;
  const currentIp = network_change?.new_ip || report?.telemetry?.ip || copy.unknownIp;
  const currentCountryCode = network_change?.new_country || report?.telemetry?.country_code || "??";
  const service = providerCopy[payload.provider]?.name ?? payload.provider;

  const explanation = isDemo
    ? copy.textDemo
    : previousIp
      ? copy.textDrop
      : currentCountryCode === "RU" || currentCountryCode === "BY"
        ? copy.textRegion
        : copy.textRisk;

  const locked = resolving || Boolean(demoFeedback);

  return (
    <Dialog
      key={`${payload.id}:${epoch}`}
      title={copy.title}
      text={demoFeedback ?? explanation}
      service={service}
      ip={currentIp}
      country={countryName(currentCountryCode) || currentCountryCode}
      demo={isDemo}
      seconds={ttl}
      onTimeout={handleTimeout}
      onClose={handleClose}
      closeLabel={copy.closeLabel}
      actions={[
        { label: copy.openGuide, icon: "zap", disabled: locked, onSelect: () => handleAction("OpenGuideAndBlock") },
        { label: copy.allow, disabled: locked, onSelect: () => handleAction("AllowSession") },
        { label: copy.block, disabled: locked, onSelect: () => handleAction("BlockUntilNetworkChange") },
      ]}
    />
  );
}

/** Страница окна alert-popup: Dialog на прозрачном фоне с полем под тень. */
export function AlertWindowPage() {
  return (
    <div className="zs-root zs-alert-win">
      <AlertWindow />
    </div>
  );
}

/** Оверлей поверх страницы для превью в браузере (без Tauri). */
export function AlertOverlay({ initialPayload, onResolved }: AlertWindowProps) {
  return (
    <div className="zs-root zs-alert-overlay">
      <AlertWindow initialPayload={initialPayload} onResolved={onResolved} />
    </div>
  );
}
