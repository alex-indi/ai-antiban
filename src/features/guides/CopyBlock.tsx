import { Button } from "../../ui";
import { common, guides } from "../../shared/copy/guides-subscription";
import { useGuide } from "./context";

export interface CopyBlockProps {
  id: string;
  /** Что показываем и копируем ровно так, как написано. Несколько строк — многострочный блок. */
  lines: string[];
  /** Название для уведомления и подписи кнопки скринридеру. */
  label: string;
  /** Заголовок уведомления; по умолчанию «Команда скопирована». */
  noticeTitle?: string;
}

/** Блок команды или адреса: моноширинный текст и кнопка «Скопировать» (после клика — «Скопировано»). */
export function CopyBlock({ id, lines, label, noticeTitle = guides.toast.commandCopied }: CopyBlockProps) {
  const { copiedId, copy } = useGuide();
  const done = copiedId === id;
  return (
    <div className="guides-code" role="group" aria-label={label}>
      <pre className="guides-code__text">
        {lines.join("\n")}
      </pre>
      <div className="guides-code__foot">
        <Button
          size="sm"
          icon={done ? "check" : "copy"}
          aria-label={guides.actions.copyCommandAria(label)}
          onClick={() => copy(id, lines.join("\n"), { title: noticeTitle, text: label })}
        >
          {done ? common.copied : common.copy}
        </Button>
        <span className="zs-sr" role="status">
          {done ? common.copied : ""}
        </span>
      </div>
    </div>
  );
}
