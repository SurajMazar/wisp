import * as RadixSlider from "@radix-ui/react-slider";
import { cn } from "../cn";

export interface SliderProps {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onValueChange: (value: number) => void;
  /** Fires once when the user lets go — use for persisting. */
  onValueCommit?: (value: number) => void;
  disabled?: boolean;
  className?: string;
}

export function Slider({
  label,
  value,
  min,
  max,
  step,
  onValueChange,
  onValueCommit,
  disabled,
  className,
}: SliderProps) {
  return (
    <RadixSlider.Root
      aria-label={label}
      className={cn("relative flex h-5 w-40 touch-none select-none items-center", className)}
      value={[value]}
      min={min}
      max={max}
      step={step}
      disabled={disabled}
      onValueChange={([v]) => v !== undefined && onValueChange(v)}
      onValueCommit={([v]) => v !== undefined && onValueCommit?.(v)}
    >
      <RadixSlider.Track className="relative h-[3px] grow rounded-full bg-border-strong">
        <RadixSlider.Range className="absolute h-full rounded-full bg-accent" />
      </RadixSlider.Track>
      <RadixSlider.Thumb
        aria-label={label}
        className="block h-3.5 w-3.5 rounded-full border border-border-strong bg-white shadow-[var(--shadow-sm)] transition-transform duration-[var(--duration-fast)] hover:scale-110"
      />
    </RadixSlider.Root>
  );
}
