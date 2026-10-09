import type { Message } from "@wisp/contracts";
import { CheckIcon, CopyIcon, RegenerateIcon, SpeakIcon, WispMark } from "@wisp/icons";
import { Button, cn } from "@wisp/ui";
import { useState } from "react";
import { Link } from "react-router";
import { copyText } from "../../lib/clipboard";
import { useConversationStore } from "../../stores/conversationStore";
import { useVoiceStore } from "../../stores/voiceStore";
import { ReasoningBlock, StepTrail } from "./AnswerProgress";
import { RichText } from "./RichText";

function Meta({ m }: { m: Message }) {
  if (!m.modelName || m.status === "streaming") return null;
  const parts = [
    m.modelName,
    m.locality === "cloud" ? "cloud" : "on this Mac",
    m.latencyMs ? `${(m.latencyMs / 1000).toFixed(1)} s` : null,
  ].filter(Boolean);
  return <span className="text-xs text-text-faint">{parts.join(" · ")}</span>;
}

function IconAction({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className="rounded-[var(--radius-sm)] p-1.5 text-text-faint hover:bg-surface-hover hover:text-text"
    >
      {children}
    </button>
  );
}

function Actions({ m, onRegenerate }: { m: Message; onRegenerate?: () => void }) {
  const [copied, setCopied] = useState(false);
  const speak = useVoiceStore((s) => s.speak);
  if (m.status === "streaming") return null;
  return (
    <div className="mt-1.5 flex items-center gap-0.5 opacity-0 transition-opacity duration-[var(--duration-fast)] group-hover:opacity-100 focus-within:opacity-100">
      {m.content && (
        <IconAction
          label={copied ? "Copied" : "Copy"}
          onClick={() =>
            void copyText(m.content).then(() => {
              setCopied(true);
              setTimeout(() => setCopied(false), 1500);
            })
          }
        >
          {copied ? <CheckIcon size={14} /> : <CopyIcon size={14} />}
        </IconAction>
      )}
      {onRegenerate && (
        <IconAction label="Regenerate" onClick={onRegenerate}>
          <RegenerateIcon size={14} />
        </IconAction>
      )}
      {m.content && (
        <IconAction label="Read aloud" onClick={() => void speak(m.content)}>
          <SpeakIcon size={14} />
        </IconAction>
      )}
      <span className="ml-1.5">
        <Meta m={m} />
      </span>
    </div>
  );
}

function ErrorBlock({ m, onRetry, compact }: { m: Message; onRetry?: () => void; compact?: boolean }) {
  const [open, setOpen] = useState(false);
  return (
    <div
      role="alert"
      className="rounded-[var(--radius-md)] border border-danger/30 bg-danger/5 p-3 text-base"
    >
      <p>{m.error}</p>
      <div className="mt-2 flex flex-wrap gap-1.5">
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
        <pre className="selectable mt-2 overflow-auto font-mono text-xs text-text-muted">
          {m.errorDetails}
        </pre>
      )}
    </div>
  );
}

/** A conversation's messages, styled like modern assistants: your turns as bubbles, answers as text. */
export function MessageList({
  messages,
  onRetry,
  onRegenerate,
  compact = false,
}: {
  messages: Message[];
  onRetry?: (question: string) => void;
  onRegenerate?: (messageId: string) => void;
  compact?: boolean;
}) {
  const lastAssistant = [...messages].reverse().find((m) => m.role === "assistant");
  const progress = useConversationStore((s) => s.progress);
  return (
    <div className={cn(compact ? "space-y-4 text-base" : "space-y-7 text-md")}>
      {messages.map((m, i) => {
        const previousUser = messages
          .slice(0, i)
          .reverse()
          .find((x) => x.role === "user");
        if (m.role === "user") {
          return (
            <div key={m.id} className="flex flex-col items-end gap-1.5">
              {m.focus && (
                <span className="inline-flex items-center gap-2 rounded-[var(--radius-md)] border border-border bg-surface py-1 pl-1 pr-2 text-xs text-text-muted">
                  {m.focus.imageUrl && (
                    <img src={m.focus.imageUrl} alt="" className="h-7 w-10 rounded-[4px] object-cover" />
                  )}
                  {m.focus.label}
                </span>
              )}
              <div
                className={cn(
                  "selectable max-w-[85%] whitespace-pre-wrap rounded-[20px] bg-surface-hover px-4 py-2.5 text-text",
                  compact && "px-3.5 py-2",
                )}
              >
                {m.content}
              </div>
            </div>
          );
        }
        return (
          <div key={m.id} className="group flex gap-3">
            {!compact && (
              <span className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full border border-border bg-surface">
                <WispMark size={14} />
              </span>
            )}
            <div className="min-w-0 flex-1">
              <StepTrail m={m} steps={progress[m.id] ?? []} />
              <ReasoningBlock m={m} steps={progress[m.id] ?? []} />
              {m.content && <RichText text={m.content} streaming={m.status === "streaming"} />}
              {m.status === "cancelled" && <p className="mt-1 text-xs text-text-faint">Stopped</p>}
              {m.status === "error" && (
                <ErrorBlock
                  m={m}
                  compact={compact}
                  {...(onRetry && previousUser ? { onRetry: () => onRetry(previousUser.content) } : {})}
                />
              )}
              <Actions
                m={m}
                {...(onRegenerate && m.id === lastAssistant?.id
                  ? { onRegenerate: () => onRegenerate(m.id) }
                  : {})}
              />
            </div>
          </div>
        );
      })}
    </div>
  );
}
