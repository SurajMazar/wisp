import { useState, type ReactNode } from "react";
import { Button } from "./Button";

/** User-friendly error with expandable technical details (spec §83). */
export function ErrorNotice({
  title,
  message,
  details,
  actions,
}: {
  title: string;
  message?: ReactNode;
  details?: string | undefined;
  actions?: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  return (
    <div role="alert" className="rounded-[var(--radius-md)] border border-danger/30 bg-danger/5 p-3">
      <p className="font-medium text-text">{title}</p>
      {message && <div className="mt-0.5 text-text-muted">{message}</div>}
      {(actions || details) && (
        <div className="mt-2 flex flex-wrap items-center gap-2">
          {actions}
          {details && (
            <Button variant="ghost" size="sm" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
              {open ? "Hide Technical Details" : "Show Technical Details"}
            </Button>
          )}
        </div>
      )}
      {open && details && (
        <pre className="selectable mt-2 overflow-auto rounded-[var(--radius-sm)] bg-bg-sunken p-2 font-mono text-xs text-text-muted">
          {details}
        </pre>
      )}
    </div>
  );
}
