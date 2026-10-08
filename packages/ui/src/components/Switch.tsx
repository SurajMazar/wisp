import * as RadixSwitch from "@radix-ui/react-switch";
import { cn } from "../cn";

export interface SwitchProps extends Omit<RadixSwitch.SwitchProps, "onChange"> {
  label: string;
}

/** Native-feeling toggle. `label` is required for VoiceOver. */
export function Switch({ className, label, ...props }: SwitchProps) {
  return (
    <RadixSwitch.Root
      aria-label={label}
      className={cn(
        "relative inline-flex h-[18px] w-[30px] shrink-0 items-center rounded-full border border-border-strong bg-bg-sunken transition-colors duration-[var(--duration-base)] data-[state=checked]:border-transparent data-[state=checked]:bg-accent disabled:opacity-45",
        className,
      )}
      {...props}
    >
      <RadixSwitch.Thumb className="block h-[14px] w-[14px] translate-x-[1px] rounded-full bg-white shadow-[var(--shadow-sm)] transition-transform duration-[var(--duration-base)] ease-[var(--ease-out-soft)] data-[state=checked]:translate-x-[13px]" />
    </RadixSwitch.Root>
  );
}
