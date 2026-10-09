import type { Message } from "@wisp/contracts";
import { Button, cn } from "@wisp/ui";
import { useEffect, useRef, useState } from "react";
import { Link } from "react-router";
import { RichText } from "./RichText";

function Meta({ m }: { m: Message }) {
  if (m.role !== "assistant" || !m.modelName) return null;
  const parts = [
    m.modelName,
    m.locality === "cloud" ? "cloud" : "on this Mac",
    m.latencyMs && m.status !== "streaming" ? `${(m.latencyMs / 1000).toFixed(1)} s` : null,
  ].filter(Boolean);
  return <p className="mt-1 text-xs text-text-faint">{parts.join(" · ")}</p>;
}

function ErrorBlock({ m, onRetry, compact }: { m: Message; onRetry?: () => void; compact?: boolean }) {
  const [open, setOpen] = useState(false);
  return (
    <div
      role="alert"
      className="mt-1 rounded-[var(--radius-sm)] border border-danger/30 bg-danger/5 p-2 text-sm"
    >
      <p>{m.error}</p>
      <div className="mt-1.5 flex flex-wrap gap-1.5">
        {onRetry && (
          <Button size="sm" onClick={onRetry}>
            Try Again
          </Button>
        )}
        {!compact && (
          <Button size="sm" variant="ghost" asChild>
            <Link to="/models">Open Models</Link>
          </Button>
        )}
        {m.errorDetails && (
          <Button size="sm" variant="ghost" onClick={() => setOpen((o) => !o)}>
            {open ? "Hide Technical Details" : "Show Technical Details"}
          </Button>
        )}
      </div>
      {open && (
        <pre className="selectable mt-1.5 overflow-auto font-mono text-xs text-text-muted">
          {m.errorDetails}
        </pre>
      )}
    </div>
  );
}

/** A conversation's messages with live streaming, status and errors. */
export function MessageList({
  messages,
  onRetry,
  compact = false,
}: {
  messages: Message[];
  onRetry?: (question: string) => void;
  compact?: boolean;
}) {
  const end = useRef<HTMLDivElement>(null);
  const last = messages.at(-1);
  useEffect(() => {
    end.current?.scrollIntoView?.({ block: "end" });
  }, [messages.length, last?.content.length]);

  return (
    <div className={cn("space-y-3", compact ? "text-base" : "text-md")} aria-live="polite">
      {messages.map((m, i) => {
        const question = messages
          .slice(0, i)
          .reverse()
          .find((x) => x.role === "user")?.content;
        return m.role === "user" ? (
          <div key={m.id} className="flex flex-col items-end">
            {m.focus && (
              <span className="mb-1 inline-flex items-center gap-1.5 rounded-[var(--radius-sm)] border border-border bg-surface px-1.5 py-1 text-xs text-text-muted">
                {m.focus.imageUrl && (
                  <img src={m.focus.imageUrl} alt="" className="h-6 w-9 rounded-[3px] object-cover" />
                )}
                {m.focus.label}
              </span>
            )}
            <div className="selectable max-w-[85%] whitespace-pre-wrap rounded-[var(--radius-lg)] bg-accent px-3 py-1.5 text-accent-contrast">
              {m.content}
            </div>
          </div>
        ) : (
          <div key={m.id} className="max-w-[95%]">
            {m.content ? (
              <RichText text={m.content} />
            ) : m.status === "streaming" ? (
              <p className="text-text-faint">Thinking…</p>
            ) : null}
            {m.status === "streaming" && m.content && (
              <span
                aria-hidden
                className="ml-0.5 inline-block h-3.5 w-1.5 animate-pulse bg-text-muted align-middle"
              />
            )}
            {m.status === "cancelled" && <p className="mt-1 text-xs text-text-faint">Stopped</p>}
            {m.status === "error" && (
              <ErrorBlock
                m={m}
                compact={compact}
                {...(onRetry && question ? { onRetry: () => onRetry(question) } : {})}
              />
            )}
            <Meta m={m} />
          </div>
        );
      })}
      <div ref={end} />
    </div>
  );
}
