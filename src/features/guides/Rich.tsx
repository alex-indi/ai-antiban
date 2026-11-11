import { Fragment } from "react";
import { Mono } from "../../ui";

const TOKEN = /(\*\*[^*]+\*\*|`[^`]+`)/;

/** Текст с мини-разметкой: `код` — моноширинный фрагмент, **жирный** — акцент. Только строки из copy. */
export function Rich({ text }: { text: string }) {
  return (
    <>
      {text.split(TOKEN).map((part, i) => {
        if (part.startsWith("**") && part.endsWith("**") && part.length > 4) {
          return <strong key={i}>{part.slice(2, -2)}</strong>;
        }
        if (part.startsWith("`") && part.endsWith("`") && part.length > 2) {
          return <Mono key={i}>{part.slice(1, -1)}</Mono>;
        }
        return <Fragment key={i}>{part}</Fragment>;
      })}
    </>
  );
}
