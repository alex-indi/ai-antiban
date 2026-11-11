import { Badge, Button, Row } from "../../ui";
import { common, guides } from "../../shared/copy/guides-subscription";
import { openExternal } from "../../utils/opener";
import { VPS_PROVIDERS } from "./providers";
import type { VpsProvider } from "./providers";
import { useGuide } from "./context";

/** Кнопка «скопировать промокод»: подпись — сам код, после клика — «Скопировано». */
export function PromoButton({ id, code, name }: { id: string; code: string; name: string }) {
  const { copiedId, copy } = useGuide();
  const done = copiedId === id;
  return (
    <>
      <Button
        size="sm"
        icon={done ? "check" : "copy"}
        aria-label={guides.actions.copyPromoAria(code, name)}
        onClick={() =>
          copy(id, code, { title: guides.toast.promoCopied, text: guides.toast.promoCopiedText(code, name) })
        }
      >
        {done ? common.copied : <span className="zs-mono">{code}</span>}
      </Button>
      <span className="zs-sr" role="status">
        {done ? common.copied : ""}
      </span>
    </>
  );
}

/**
 * Хостинги: строка с переходом на сайт (партнёрская ссылка без изменений) и промокод под ней.
 * compact — название, конфигурация, цена; full — ещё описание, бейдж и локации.
 */
export function HostingList({ variant }: { variant: "compact" | "full" }) {
  const { onToast } = useGuide();

  const open = (vps: VpsProvider) => {
    openExternal(vps.url);
    onToast?.(guides.toast.openHosting, guides.toast.openHostingText(vps.name), "info");
  };

  return (
    <ul className="guides-offers">
      {VPS_PROVIDERS.map((vps) => (
        <li key={vps.id} className="guides-offer">
          <Row
            as="div"
            icon="globe"
            title={
              <>
                {vps.name}
                <span className="zs-sr">{common.openSite}</span>
              </>
            }
            sub={variant === "compact" ? `${vps.specs} · ${vps.locations.join(", ")}` : vps.tier}
            onClick={() => open(vps)}
          />
          {variant === "full" && (
            <>
              <p className="guides-offer__text">{vps.description}</p>
              <p className="guides-offer__facts">
                <strong>{vps.price}</strong> · {vps.specs}
              </p>
              <div className="guides-offer__chips">
                {vps.locations.map((loc) => (
                  <Badge key={loc} mono>
                    {loc}
                  </Badge>
                ))}
              </div>
            </>
          )}
          <div className="guides-offer__meta">
            {variant === "compact" ? <Badge tone="info">{vps.price}</Badge> : <Badge tone="info">{vps.badge}</Badge>}
            <PromoButton id={vps.id} code={vps.promoCode} name={vps.name} />
          </div>
        </li>
      ))}
    </ul>
  );
}
