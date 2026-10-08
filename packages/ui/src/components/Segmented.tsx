import * as RadioGroup from "@radix-ui/react-radio-group";
import type { ReactNode } from "react";
import { cn } from "../cn";

export interface SegmentedOption<T extends string> {
  value: T;
  label: ReactNode;
  disabled?: boolean;
}

export interface SegmentedProps<T extends string> {
  value: T;
  onValueChange: (value: T) => void;
  options: SegmentedOption<T>[];
  label: string;
  disabled?: boolean;
  className?: string;
}

/** macOS-style segmented control, implemented as an accessible radio group. */
export function Segmented<T extends string>({
  value,
  onValueChange,
  options,
  label,
  disabled,
  className,
}: SegmentedProps<T>) {
  return (
    <RadioGroup.Root
      aria-label={label}
      value={value}
      disabled={disabled}
      onValueChange={(v) => onValueChange(v as T)}
      orientation="horizontal"
      className={cn(
        "inline-flex rounded-[var(--radius-sm)] bg-bg-sunken p-0.5 border border-border",
        className,
      )}
    >
      {options.map((o) => (
        <RadioGroup.Item
          key={o.value}
          value={o.value}
          disabled={o.disabled}
          className="h-6 rounded-[5px] px-2.5 text-sm text-text-muted transition-colors duration-[var(--duration-fast)] hover:text-text data-[state=checked]:bg-surface data-[state=checked]:text-text data-[state=checked]:shadow-[var(--shadow-sm)] disabled:opacity-45"
        >
          {o.label}
        </RadioGroup.Item>
      ))}
    </RadioGroup.Root>
  );
}
