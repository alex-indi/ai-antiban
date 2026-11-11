import { forwardRef, useId } from "react";
import type { InputHTMLAttributes, ReactNode } from "react";
import { cx } from "./cx";

export interface FieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  /** Подпись сверху — всегда видима, плейсхолдер её не заменяет. */
  label: string;
  /** Подсказка снизу. */
  help?: ReactNode;
  /** Текст ошибки вместо подсказки; включает aria-invalid. */
  error?: ReactNode;
  /** Кнопка отправки справа от поля. */
  action?: ReactNode;
  /** Классы для обёртки поля (className идёт на input). */
  fieldClassName?: string;
}

/** Поле ввода: подпись сверху, значение моноширинным, подсказка или ошибка снизу. */
export const Field = forwardRef<HTMLInputElement, FieldProps>(function Field(
  { label, help, error, action, id, className, fieldClassName, ...rest },
  ref,
) {
  const auto = useId();
  const inputId = id ?? auto;
  const noteId = `${inputId}-note`;
  const note = error ?? help;
  return (
    <div className={cx("zs-field", fieldClassName)}>
      <label className="zs-field__label" htmlFor={inputId}>
        {label}
      </label>
      <div className="zs-field__control">
        <input
          ref={ref}
          id={inputId}
          className={cx("zs-input", className)}
          aria-invalid={error ? true : undefined}
          aria-describedby={note ? noteId : undefined}
          {...rest}
        />
        {action}
      </div>
      {note && (
        <span id={noteId} className={cx("zs-field__help", error ? "zs-field__help--error" : false)}>
          {note}
        </span>
      )}
    </div>
  );
});
