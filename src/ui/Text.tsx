import type { AnchorHTMLAttributes, ElementType, HTMLAttributes, ReactNode } from "react";
import { Icon } from "./Icon";
import { cx } from "./cx";

/** Моноширинный текст для данных: IP, домены, ключи, версии. */
export function Mono({ children, className, ...rest }: HTMLAttributes<HTMLSpanElement>) {
  return (
    <span {...rest} className={cx("zs-mono", className)}>
      {children}
    </span>
  );
}

export interface LinkProps extends AnchorHTMLAttributes<HTMLAnchorElement> {
  /** Шеврон справа — для перехода к инструкции. */
  chevron?: boolean;
  children: ReactNode;
}

/** Ссылка-действие (info-text). */
export function Link({ chevron, className, children, ...rest }: LinkProps) {
  return (
    <a {...rest} className={cx("zs-link", className)}>
      {children}
      {chevron && <Icon name="chev" />}
    </a>
  );
}

export interface SectionProps {
  /** Метка раздела: единственное место для прописных (стиль overline). */
  title: string;
  children: ReactNode;
  /** Уровень заголовка в разметке. */
  as?: ElementType;
  className?: string;
}

/** Раздел окна: overline-метка над содержимым (карточкой или плитками). */
export function Section({ title, children, as: Heading = "h3", className }: SectionProps) {
  return (
    <div className={cx("zs-section", className)}>
      <Heading className="zs-overline">{title}</Heading>
      {children}
    </div>
  );
}
