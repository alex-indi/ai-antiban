import { Button } from "./Button";

export interface CopyBlockProps {
  /** Что показываем и копируем ровно так, как написано. Несколько строк — многострочный блок. */
  lines: string[];
  /** Название блока: подпись группы и кнопки для скринридера. */
  label: string;
  /** Текст уже скопирован: кнопка показывает «Скопировано». Состояние держит вызывающий (так проще сбрасывать по таймеру). */
  copied?: boolean;
  onCopy: (text: string) => void;
  copyLabel?: string;
  copiedLabel?: string;
  /** Подпись кнопки для скринридера; по умолчанию «Скопировать: название». */
  copyAriaLabel?: string;
  className?: string;
}

/** Блок команды или адреса: моноширинный текст на слое bg-2 и кнопка «Скопировать». */
export function CopyBlock({
  lines,
  label,
  copied = false,
  onCopy,
  copyLabel = "Скопировать",
  copiedLabel = "Скопировано",
  copyAriaLabel,
  className,
}: CopyBlockProps) {
  const text = lines.join("\n");
  return (
    <div className={className ? `zs-code ${className}` : "zs-code"} role="group" aria-label={label}>
      <pre className="zs-code__text">{text}</pre>
      <div className="zs-code__foot">
        <Button size="sm" icon={copied ? "check" : "copy"} aria-label={copyAriaLabel ?? `${copyLabel}: ${label}`} onClick={() => onCopy(text)}>
          {copied ? copiedLabel : copyLabel}
        </Button>
        <span className="zs-sr" role="status">
          {copied ? copiedLabel : ""}
        </span>
      </div>
    </div>
  );
}
