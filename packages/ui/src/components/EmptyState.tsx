import type { ReactNode } from "react";

/** Honest empty / unavailable state. Never pretends a feature works. */
export function EmptyState({
  icon,
  title,
  children,
  action,
}: {
  icon?: ReactNode;
  title: string;
  children?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="mx-auto flex max-w-sm flex-col items-center py-16 text-center">
      {icon && <div className="mb-3 text-text-faint">{icon}</div>}
      <h2 className="text-md font-semibold text-text">{title}</h2>
      {children && <div className="mt-1 text-base text-text-muted">{children}</div>}
      {action && <div className="mt-4">{action}</div>}
    </div>
  );
}
