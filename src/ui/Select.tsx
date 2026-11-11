import { forwardRef, useId } from "react";
import type { ReactNode, SelectHTMLAttributes } from "react";
import { cx } from "./cx";

export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}

export interface SelectProps extends Omit<SelectHTMLAttributes<HTMLSelectElement>, "size"> {
  label?: string;
  help?: ReactNode;
  error?: ReactNode;
  options?: SelectOption[];
  fieldClassName?: string;
}

/** Выпадающий список выбора на базе дизайн-токенов Заставы. */
export const Select = forwardRef<HTMLSelectElement, SelectProps>(function Select(
  { label, help, error, options, children, id, className, fieldClassName, ...rest },
  ref,
) {
  const auto = useId();
  const selectId = id ?? auto;
  const noteId = `${selectId}-note`;
  const note = error ?? help;

  return (
    <div className={cx("zs-field", fieldClassName)}>
      {label && (
        <label className="zs-field__label" htmlFor={selectId}>
          {label}
        </label>
      )}
      <div className="zs-field__control">
        <select
          ref={ref}
          id={selectId}
          className={cx("zs-input", "zs-select", className)}
          aria-invalid={error ? true : undefined}
          aria-describedby={note ? noteId : undefined}
          {...rest}
        >
          {options
            ? options.map((opt) => (
                <option key={opt.value} value={opt.value} disabled={opt.disabled}>
                  {opt.label}
                </option>
              ))
            : children}
        </select>
      </div>
      {note && (
        <span id={noteId} className={cx("zs-field__help", error ? "zs-field__help--error" : false)}>
          {note}
        </span>
      )}
    </div>
  );
});
