import type { ReactNode } from "react";
import { Icon } from "./Icon";
import type { IconName } from "./Icon";
import { cx } from "./cx";

export type RowTone = "neutral" | "ok" | "warn" | "danger";

/** Список строк (ul.zs-rows) — кладите внутрь Card. */
export function Rows({ children, className }: { children: ReactNode; className?: string }) {
  return <ul className={cx("zs-rows", className)}>{children}</ul>;
}

export interface RowProps {
  /** Иконка в подложке 32px. */
  icon?: IconName;
  /** Тон подложки повторяет статус строки. */
  tone?: RowTone;
  title: ReactNode;
  /** Пояснение человеческим языком; переносится, а не обрезается. */
  sub?: ReactNode;
  /** Хвост: бейдж, тумблер или кнопка sm. Для кликабельной строки хвост — только шеврон (добавляется сам). */
  trail?: ReactNode;
  /** Делает строку кнопкой на всю ширину с шевроном. */
  onClick?: () => void;
  /** Делает строку ссылкой на всю ширину с шевроном. */
  href?: string;
  /** li (по умолчанию, внутри Rows) или div (одиночная строка, например под StatusHero). */
  as?: "li" | "div";
  className?: string;
}

/** Строка списка: иконка, название, пояснение, хвост. Кликабельная получает шеврон. */
export function Row({ icon, tone = "neutral", title, sub, trail, onClick, href, as: Tag = "li", className }: RowProps) {
  const interactive = Boolean(onClick || href);
  const text = (
    <>
      <span className="zs-row__title">{title}</span>
      {sub && <span className="zs-row__sub">{sub}</span>}
    </>
  );
  return (
    <Tag className={cx("zs-row", interactive && "zs-row--action", className)}>
      {icon && (
        <span className={cx("zs-row__icon", tone !== "neutral" && `zs-row__icon--${tone}`)}>
          <Icon name={icon} />
        </span>
      )}
      <div className="zs-row__body">
        {href ? (
          <a className="zs-row__hit" href={href}>
            {text}
          </a>
        ) : onClick ? (
          <button type="button" className="zs-row__hit" onClick={onClick}>
            {text}
          </button>
        ) : (
          <>
            <p className="zs-row__title">{title}</p>
            {sub && <p className="zs-row__sub">{sub}</p>}
          </>
        )}
      </div>
      {(trail || interactive) && (
        <div className="zs-row__trail">
          {trail}
          {interactive && <Icon name="chev" />}
        </div>
      )}
    </Tag>
  );
}
