import { cn } from "../cn";

export function Spinner({ className, label = "Loading" }: { className?: string; label?: string }) {
  return (
    <span
      role="status"
      aria-label={label}
      className={cn(
        "inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-border-strong border-t-accent",
        className,
      )}
    />
  );
}
