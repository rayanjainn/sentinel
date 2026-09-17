import type { ButtonHTMLAttributes, ReactNode, Ref } from "react";

import { cx } from "../lib/cx";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger" | "dangerQuiet";
export type ButtonSize = "sm" | "md";

const VARIANTS: Record<ButtonVariant, string> = {
  primary: "bg-signal text-signal-ink hover:brightness-110 font-medium",
  secondary: "bg-raised text-fg border border-line hover:border-line-strong",
  ghost: "text-fg-muted hover:bg-raised hover:text-fg",
  danger: "bg-danger text-danger-ink hover:brightness-110 font-medium",
  dangerQuiet: "text-danger hover:bg-danger/10",
};

const SIZES: Record<ButtonSize, string> = {
  sm: "h-6 px-2 text-[12px] gap-1",
  md: "h-7 px-2.5 text-[13px] gap-1.5",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  icon?: ReactNode;
  ref?: Ref<HTMLButtonElement>;
}

export function Button({
  variant = "secondary",
  size = "md",
  icon,
  className,
  children,
  type = "button",
  ref,
  ...rest
}: ButtonProps) {
  return (
    <button
      ref={ref}
      type={type}
      className={cx(
        "inline-flex shrink-0 items-center justify-center whitespace-nowrap rounded-[5px] transition-[background-color,border-color,color,filter,transform] duration-150 active:translate-y-px disabled:pointer-events-none disabled:opacity-45",
        VARIANTS[variant],
        SIZES[size],
        className,
      )}
      {...rest}
    >
      {icon}
      {children}
    </button>
  );
}

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  active?: boolean;
  size?: ButtonSize;
  ref?: Ref<HTMLButtonElement>;
}

export function IconButton({ label, active, size = "md", className, children, type = "button", ref, ...rest }: IconButtonProps) {
  return (
    <button
      ref={ref}
      type={type}
      aria-label={label}
      title={label}
      aria-pressed={active}
      className={cx(
        "inline-flex shrink-0 items-center justify-center rounded-[5px] transition-colors duration-150 active:translate-y-px disabled:pointer-events-none disabled:opacity-45",
        size === "sm" ? "size-6" : "size-7",
        active ? "bg-raised text-fg" : "text-fg-muted hover:bg-raised hover:text-fg",
        className,
      )}
      {...rest}
    >
      {children}
    </button>
  );
}
