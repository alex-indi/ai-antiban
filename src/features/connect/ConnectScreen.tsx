import { useState } from "react";
import type { AppStateData } from "../../types";
import { clearUpstreamProxy, setUpstreamProxy, toggleUpstreamProxy } from "../../utils/api";
import { openExternal, openTelegramSupport } from "../../utils/opener";
import { Badge, Button, Card, Field, Mono, Row, Rows, Switch } from "../../ui";
import { connect as copy } from "../../shared/copy/ru";
import { countryName } from "../../shared/lib/country";
import type { ShowToast } from "../../shared/hooks";
import { HOSTINGS } from "./hostings";

export interface ConnectScreenProps {
  state: AppStateData | null;
  currentIp: string;
  currentCountry: string;
  onRefreshState: () => Promise<unknown>;
  onAuditRefresh: () => Promise<void>;
  onOpenGuide: (guideId: string) => void;
  onToast: ShowToast;
}

const VPN_GUIDE = "01-personal-vpn-15min";

/** Экран «Подключение»: свой VPN за 15 минут и «Ключ Заставы» (в тестировании, но работает). */
export function ConnectScreen({
  state,
  currentIp,
  currentCountry,
  onRefreshState,
  onAuditRefresh,
  onOpenGuide,
  onToast,
}: ConnectScreenProps) {
  const [inputKey, setInputKey] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [connecting, setConnecting] = useState<boolean>(false);
  const [toggling, setToggling] = useState<boolean>(false);

  const upstream = state?.upstream_proxy ?? null;
  const isConfigured = Boolean(upstream);
  const isActive = upstream?.enabled ?? false;

  const handleConnect = async () => {
    const key = inputKey.trim();
    if (!key) {
      setError(copy.key.empty);
      return;
    }
    setError(null);
    setConnecting(true);
    try {
      await setUpstreamProxy(key);
      await onRefreshState();
      await onAuditRefresh();
      setInputKey("");
      onToast(copy.key.toast.connected, copy.key.toast.connectedText, "success");
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      onToast(copy.key.toast.connectError, msg, "error");
    } finally {
      setConnecting(false);
    }
  };

  const handleToggle = async () => {
    if (!upstream) return;
    setToggling(true);
    try {
      const next = !upstream.enabled;
      await toggleUpstreamProxy(next);
      await onRefreshState();
      await onAuditRefresh();
      onToast(
        next ? copy.key.toast.on : copy.key.toast.off,
        next ? copy.key.toast.onText : copy.key.toast.offText,
        "info",
      );
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      onToast(copy.key.toast.toggleError, msg, "error");
    } finally {
      setToggling(false);
    }
  };

  const handleDisconnect = async () => {
    try {
      await clearUpstreamProxy();
      await onRefreshState();
      await onAuditRefresh();
      onToast(copy.key.toast.disconnected, copy.key.toast.disconnectedText, "info");
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      onToast(copy.key.toast.disconnectError, msg, "error");
    }
  };

  const exitKnown = currentIp !== "--";

  return (
    <>
      <Card title={copy.vpn.title} aside={<Badge tone="ok">{copy.vpn.badge}</Badge>}>
        <p className="zs-t-body mb-3 text-ink-2">{copy.vpn.text}</p>
        <Rows>
          {HOSTINGS.map((h) => (
            <Row key={h.id} icon="globe" title={h.name} sub={h.sub} onClick={() => openExternal(h.url)} />
          ))}
        </Rows>
        <Button
          variant={isConfigured ? "secondary" : "primary"}
          block
          className="mt-3"
          onClick={() => onOpenGuide(VPN_GUIDE)}
        >
          {copy.vpn.open}
        </Button>
      </Card>

      <Card title={copy.key.title} aside={<Badge tone="info">{copy.key.badge}</Badge>}>
        <p className="zs-t-body mb-3 text-ink-2">{copy.key.text}</p>

        {isConfigured && upstream ? (
          <>
            <Rows>
              <Row
                icon="key"
                tone={isActive ? "ok" : "neutral"}
                title={upstream.label || copy.key.defaultLabel}
                sub={isActive ? copy.key.switchOn : copy.key.switchOff}
                trail={
                  <Switch
                    checked={isActive}
                    disabled={toggling}
                    onChange={handleToggle}
                    aria-label={copy.key.switchLabel}
                  />
                }
              />
              <Row
                icon="globe"
                title={copy.key.exit}
                sub={
                  exitKnown ? (
                    <>
                      <Mono>{currentIp}</Mono>
                      {countryName(currentCountry) ? ` · ${countryName(currentCountry)}` : ""}
                    </>
                  ) : (
                    copy.key.exitUnknown
                  )
                }
              />
            </Rows>
            <Button variant="danger" block className="mt-3" onClick={handleDisconnect}>
              {copy.key.disconnect}
            </Button>
          </>
        ) : (
          <>
            <Field
              label={copy.key.fieldLabel}
              placeholder={copy.key.placeholder}
              value={inputKey}
              onChange={(e) => {
                setInputKey(e.target.value);
                if (error) setError(null);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleConnect();
              }}
              autoComplete="off"
              spellCheck={false}
              help={copy.key.caption}
              error={error ?? undefined}
              action={
                <Button onClick={handleConnect} disabled={connecting}>
                  {connecting ? copy.key.connecting : copy.key.connect}
                </Button>
              }
            />
            <Button variant="quiet" size="sm" className="mt-2 -ml-3" onClick={() => openTelegramSupport()}>
              {copy.key.get}
            </Button>
          </>
        )}
      </Card>
    </>
  );
}
