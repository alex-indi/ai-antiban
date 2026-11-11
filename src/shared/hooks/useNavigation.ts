import { useCallback, useState } from "react";

export type TabId = "overview" | "connect" | "checks" | "backup" | "guides";

export const TAB_ORDER: TabId[] = ["overview", "connect", "checks", "backup", "guides"];

/** Гайд, который открывается, если вызвавший не указал другой. */
export const DEFAULT_GUIDE = "01-personal-vpn-15min";

interface Initial {
  tab: TabId;
  subscription: boolean;
}

/** ?tab= понимает и старые id вкладок (pulse, clean-ip, claude_vault, bin, audit, guide), и новые. */
function readInitial(): Initial {
  const raw = new URLSearchParams(window.location.search).get("tab");
  switch (raw) {
    case "overview":
    case "pulse":
      return { tab: "overview", subscription: false };
    case "connect":
    case "clean-ip":
      return { tab: "connect", subscription: false };
    case "checks":
    case "audit":
      return { tab: "checks", subscription: false };
    case "backup":
    case "claude_vault":
      return { tab: "backup", subscription: false };
    case "guides":
    case "guide":
      return { tab: "guides", subscription: false };
    case "bin":
    case "subscription":
      return { tab: "overview", subscription: true };
    default:
      return { tab: "overview", subscription: false };
  }
}

/** Навигация оболочки: вкладка, экран «Подписка» из меню «Ещё» и цель гайда. */
export function useNavigation() {
  const [initial] = useState(readInitial);
  const [tab, setTab] = useState<TabId>(initial.tab);
  const [subscriptionOpen, setSubscriptionOpen] = useState<boolean>(initial.subscription);
  const [guideTarget, setGuideTarget] = useState<string>(DEFAULT_GUIDE);

  const goTab = useCallback((next: TabId) => {
    setSubscriptionOpen(false);
    setTab(next);
  }, []);

  /** Открывает «Гайды»; без id остаётся прежняя цель. */
  const openGuide = useCallback((guideId?: string) => {
    if (guideId) setGuideTarget(guideId);
    setSubscriptionOpen(false);
    setTab("guides");
  }, []);

  const openSubscription = useCallback(() => setSubscriptionOpen(true), []);
  const closeSubscription = useCallback(() => setSubscriptionOpen(false), []);

  return { tab, goTab, subscriptionOpen, openSubscription, closeSubscription, guideTarget, openGuide };
}
