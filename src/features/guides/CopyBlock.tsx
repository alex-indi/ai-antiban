import { CopyBlock as CopyBlockView } from "../../ui";
import { common, guides } from "../../shared/copy/ru";
import { useGuide } from "./context";

export interface GuideCopyBlockProps {
  id: string;
  /** Что показываем и копируем ровно так, как написано. Несколько строк — многострочный блок. */
  lines: string[];
  /** Название для уведомления и подписи кнопки скринридеру. */
  label: string;
  /** Заголовок уведомления; по умолчанию «Команда скопирована». */
  noticeTitle?: string;
}

/** Блок команды из ридера гайдов: ui/CopyBlock, привязанный к состоянию «скопировано» и уведомлению ридера. */
export function CopyBlock({ id, lines, label, noticeTitle = guides.toast.commandCopied }: GuideCopyBlockProps) {
  const { copiedId, copy } = useGuide();
  return (
    <CopyBlockView
      lines={lines}
      label={label}
      copied={copiedId === id}
      copyLabel={common.copy}
      copiedLabel={common.copied}
      copyAriaLabel={guides.actions.copyCommandAria(label)}
      onCopy={(text) => copy(id, text, { title: noticeTitle, text: label })}
    />
  );
}
