import { subscription } from "../../shared/copy/ru";

const b = subscription.card.brands;

/** Платёжная система по первым цифрам BIN. Правила те же, что были в BinCheckerTab. */
export function detectBrand(value: string): string {
  const clean = value.replace(/\D/g, "");
  if (!clean) return b.empty;
  if (clean.startsWith("4")) return b.visa;
  if (
    clean.startsWith("51") ||
    clean.startsWith("52") ||
    clean.startsWith("53") ||
    clean.startsWith("54") ||
    clean.startsWith("55") ||
    (Number(clean.slice(0, 4)) >= 2221 && Number(clean.slice(0, 4)) <= 2720)
  ) {
    return b.mastercard;
  }
  if (
    clean.startsWith("2200") ||
    clean.startsWith("2201") ||
    clean.startsWith("2202") ||
    clean.startsWith("2203") ||
    clean.startsWith("2204")
  ) {
    return b.mir;
  }
  if (clean.startsWith("34") || clean.startsWith("37")) return b.amex;
  if (clean.startsWith("62")) return b.unionpay;
  return b.bank;
}
