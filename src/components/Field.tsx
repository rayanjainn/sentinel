import { CaretDown, MagnifyingGlass, X } from "@phosphor-icons/react";
import { useEffect, useRef, type ReactNode, type SelectHTMLAttributes } from "react";

import { cx } from "../lib/cx";
import { isModEvent, modKey } from "../lib/platform";

export function SearchField({
  value,
  onChange,
  placeholder,
  label,
  focusShortcut = false,
  className,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  label: string;
  /** Focus with Cmd/Ctrl+F while mounted. */
  focusShortcut?: boolean;
  className?: string;
}) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (!focusShortcut) return;
    const onKey = (event: KeyboardEvent) => {
      if (isModEvent(event) && event.key.toLowerCase() === "f") {
        event.preventDefault();
        ref.current?.focus();
        ref.current?.select();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [focusShortcut]);

  return (
    <div className={cx("relative flex h-7 items-center", className)}>
      <MagnifyingGlass size={13} className="pointer-events-none absolute left-2 text-fg-subtle" />
      <input
        ref={ref}
        type="search"
        aria-label={label}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape" && value) {
            e.stopPropagation();
            onChange("");
          }
        }}
        placeholder={focusShortcut ? `${placeholder}  ${modKey}F` : placeholder}
        spellCheck={false}
        autoComplete="off"
        className="selectable h-full w-full rounded-[5px] border border-line bg-sunken pl-7 pr-7 text-[13px] text-fg placeholder:text-fg-subtle focus:border-line-strong focus:outline-none [&::-webkit-search-cancel-button]:hidden"
      />
      {value && (
        <button
          type="button"
          aria-label="Clear search"
          onClick={() => onChange("")}
          className="absolute right-1.5 rounded-[4px] p-0.5 text-fg-subtle hover:text-fg"
        >
          <X size={12} />
        </button>
      )}
    </div>
  );
}

export function Select({
  label,
  className,
  children,
  ...rest
}: SelectHTMLAttributes<HTMLSelectElement> & { label: string; children: ReactNode }) {
  return (
    <div className={cx("relative inline-flex h-7 items-center", className)}>
      <select
        aria-label={label}
        className="h-full w-full appearance-none rounded-[5px] border border-line bg-sunken pl-2.5 pr-7 text-[12px] text-fg hover:border-line-strong focus:border-line-strong focus:outline-none"
        {...rest}
      >
        {children}
      </select>
      <CaretDown size={11} className="pointer-events-none absolute right-2 text-fg-subtle" />
    </div>
  );
}

export function Checkbox({
  checked,
  indeterminate = false,
  onChange,
  label,
  className,
}: {
  checked: boolean;
  indeterminate?: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  className?: string;
}) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = indeterminate;
  }, [indeterminate]);
  return (
    <input
      ref={ref}
      type="checkbox"
      aria-label={label}
      checked={checked}
      onChange={(e) => onChange(e.target.checked)}
      onClick={(e) => e.stopPropagation()}
      className={cx("size-3.5 shrink-0 cursor-pointer accent-[var(--signal)]", className)}
    />
  );
}
