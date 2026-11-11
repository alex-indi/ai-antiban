import { useState } from "react";
import type { AppStateData, RiskReport } from "../../types";
import { Segmented } from "../../ui";
import { subscription as copy } from "../../shared/copy/guides-subscription";
import type { ShowToast } from "../../shared/hooks";
import { AddressCard } from "./AddressCard";
import { CardCheck } from "./CardCheck";
import { Warmup, warmupProgress } from "./Warmup";
import "./subscription.css";

export interface SubscriptionScreenProps {
  vpnCountry?: string;
  activeReport?: RiskReport | null;
  state?: AppStateData | null;
  onStateChange?: () => void;
  onNavigateToPaymentGuide?: () => void;
  onToast?: ShowToast;
}

type SubTab = "card" | "warmup";

/** Экран «Подписка» (меню «Ещё»): проверка карты и адрес для оплаты, прогрев аккаунта и чеклист. */
export function SubscriptionScreen({
  vpnCountry = "NL",
  activeReport,
  state,
  onStateChange,
  onNavigateToPaymentGuide,
  onToast,
}: SubscriptionScreenProps) {
  const [tab, setTab] = useState<SubTab>("card");

  const country =
    vpnCountry && vpnCountry !== "--" ? vpnCountry : activeReport?.telemetry.country_code || "NL";

  const warm = warmupProgress(state?.warmup_started_at);
  const warmupLabel = warm.started
    ? warm.done
      ? copy.tabs.warmupDone
      : copy.tabs.warmupRunning
    : copy.tabs.warmup;

  return (
    <div className="zs-stack">
      <Segmented<SubTab>
        aria-label={copy.tabsLabel}
        value={tab}
        onChange={setTab}
        options={[
          { value: "card", label: copy.tabs.card },
          { value: "warmup", label: warmupLabel },
        ]}
      />

      {tab === "card" ? (
        <>
          <CardCheck country={country} onToast={onToast} onNavigateToPaymentGuide={onNavigateToPaymentGuide} />
          <AddressCard onToast={onToast} />
        </>
      ) : (
        <Warmup
          state={state}
          onStateChange={onStateChange}
          onNavigateToPaymentGuide={onNavigateToPaymentGuide}
          onToast={onToast}
        />
      )}
    </div>
  );
}
