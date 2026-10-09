import { cn } from "@wisp/ui";
import { useEffect, useRef, useState, type ReactNode } from "react";

/** Minimal click-to-open popover that closes on outside click or Escape. */
export function Popover({
  trigger,
  children,
  align = "start",
  side = "top",
  label,
}: {
  trigger: (open: boolean, toggle: () => void) => ReactNode;
  children: (close: () => void) => ReactNode;
  align?: "start" | "end";
  side?: "top" | "bottom";
  label: string;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey, true);
    };
  }, [open]);
  return (
    <div ref={ref} className="relative">
      {trigger(open, () => setOpen((o) => !o))}
      {open && (
        <div
          role="menu"
          aria-label={label}
          className={cn(
            "absolute z-[var(--z-popover)] min-w-52 rounded-[var(--radius-lg)] border border-border-strong bg-surface p-1 shadow-[var(--shadow-lg)]",
            side === "top" ? "bottom-full mb-2" : "top-full mt-2",
            align === "start" ? "left-0" : "right-0",
          )}
        >
          {children(() => setOpen(false))}
        </div>
      )}
    </div>
  );
}

export function MenuItem({
  icon,
  label,
  hint,
  onSelect,
  active,
}: {
  icon?: ReactNode;
  label: string;
  hint?: string;
  onSelect: () => void;
  active?: boolean;
}) {
  return (
    <button
      type="button"
      role="menuitem"
      onClick={onSelect}
      className={cn(
        "flex w-full items-center gap-2.5 rounded-[var(--radius-md)] px-2.5 py-1.5 text-left hover:bg-surface-hover",
        active && "bg-surface-hover",
      )}
    >
      {icon && <span className="shrink-0 text-text-muted">{icon}</span>}
      <span className="min-w-0 flex-1">
        <span className="block truncate text-base text-text">{label}</span>
        {hint && <span className="block truncate text-xs text-text-faint">{hint}</span>}
      </span>
    </button>
  );
}
