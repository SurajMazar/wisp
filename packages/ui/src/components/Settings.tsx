import type { ReactNode } from "react";
import { cn } from "../cn";

/** Grouped settings list in the style of macOS System Settings — rows, not cards. */
export function SettingsGroup({
  title,
  description,
  children,
}: {
  title?: string;
  description?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="mb-6">
      {title && <h3 className="mb-1.5 px-1 text-sm font-semibold text-text-muted">{title}</h3>}
      <div className="divide-y divide-border overflow-hidden rounded-[var(--radius-md)] border border-border bg-surface">
        {children}
      </div>
      {description && <p className="mt-1.5 px-1 text-sm text-text-faint">{description}</p>}
    </section>
  );
}

export function SettingsRow({
  label,
  hint,
  control,
  className,
  htmlFor,
}: {
  label: ReactNode;
  hint?: ReactNode;
  control: ReactNode;
  className?: string;
  htmlFor?: string;
}) {
  return (
    <div className={cn("flex min-h-11 items-center justify-between gap-6 px-3 py-2", className)}>
      <div className="min-w-0">
        <label htmlFor={htmlFor} className="block text-base text-text">
          {label}
        </label>
        {hint && <p className="text-sm text-text-muted">{hint}</p>}
      </div>
      <div className="shrink-0">{control}</div>
    </div>
  );
}
