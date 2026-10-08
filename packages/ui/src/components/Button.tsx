import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import type { ButtonHTMLAttributes } from "react";
import { cn } from "../cn";

const button = cva(
  "inline-flex items-center justify-center gap-1.5 whitespace-nowrap font-medium transition-colors duration-[var(--duration-fast)] disabled:pointer-events-none disabled:opacity-45 select-none",
  {
    variants: {
      variant: {
        primary:
          "bg-accent text-accent-contrast hover:brightness-110 active:brightness-95 shadow-[var(--shadow-sm)]",
        secondary: "bg-surface text-text border border-border-strong hover:bg-surface-hover",
        ghost: "text-text-muted hover:text-text hover:bg-surface-hover",
        danger: "bg-danger text-white hover:brightness-110",
      },
      size: {
        sm: "h-6 px-2 text-sm rounded-[var(--radius-sm)]",
        md: "h-7 px-3 text-base rounded-[var(--radius-sm)]",
        lg: "h-9 px-4 text-md rounded-[var(--radius-md)]",
        icon: "h-7 w-7 rounded-[var(--radius-sm)]",
      },
    },
    defaultVariants: { variant: "secondary", size: "md" },
  },
);

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof button> {
  asChild?: boolean;
}

export function Button({ className, variant, size, asChild, type = "button", ...props }: ButtonProps) {
  const Comp = asChild ? Slot : "button";
  return (
    <Comp type={asChild ? undefined : type} className={cn(button({ variant, size }), className)} {...props} />
  );
}
