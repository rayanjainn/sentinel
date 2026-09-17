import { motion } from "motion/react";
import { useId, useRef, type KeyboardEvent, type ReactNode } from "react";

import { cx } from "../lib/cx";
import { springInteraction } from "../lib/motion";

export interface SegmentedOption<T extends string> {
  value: T;
  label: ReactNode;
  title?: string;
}

export function Segmented<T extends string>({
  options,
  value,
  onChange,
  label,
  size = "md",
  className,
}: {
  options: SegmentedOption<T>[];
  value: T;
  onChange: (value: T) => void;
  label: string;
  size?: "sm" | "md";
  className?: string;
}) {
  const indicatorId = useId();
  const refs = useRef<Array<HTMLButtonElement | null>>([]);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const index = options.findIndex((o) => o.value === value);
    const delta = event.key === "ArrowRight" ? 1 : -1;
    const next = options[(index + delta + options.length) % options.length];
    if (!next) return;
    onChange(next.value);
    refs.current[options.indexOf(next)]?.focus();
  };

  return (
    <div
      role="radiogroup"
      aria-label={label}
      onKeyDown={onKeyDown}
      className={cx("inline-flex rounded-[6px] border border-line bg-sunken p-0.5", className)}
    >
      {options.map((option, i) => {
        const active = option.value === value;
        return (
          <button
            key={option.value}
            ref={(el) => {
              refs.current[i] = el;
            }}
            type="button"
            role="radio"
            aria-checked={active}
            title={option.title}
            tabIndex={active ? 0 : -1}
            onClick={() => onChange(option.value)}
            className={cx(
              "relative inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-[4px] transition-colors",
              size === "sm" ? "h-5 px-2 text-[11px]" : "h-6 px-2.5 text-[12px]",
              active ? "text-fg" : "text-fg-muted hover:text-fg",
            )}
          >
            {active && (
              <motion.span
                layoutId={indicatorId}
                transition={springInteraction}
                className="absolute inset-0 rounded-[4px] bg-raised shadow-[0_0_0_1px_var(--line-strong)]"
              />
            )}
            <span className="relative inline-flex items-center gap-1.5">{option.label}</span>
          </button>
        );
      })}
    </div>
  );
}
