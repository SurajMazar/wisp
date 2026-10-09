import type { Message } from "@wisp/contracts";
import { ArrowDownIcon } from "@wisp/icons";
import { cn } from "@wisp/ui";
import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { MessageList } from "./MessageList";

/** Scrolling thread that sticks to the bottom only while you're already there. */
export function ChatThread({
  messages,
  onRetry,
  onRegenerate,
  compact,
  footer,
  className,
}: {
  messages: Message[];
  onRetry?: (q: string) => void;
  onRegenerate?: (id: string) => void;
  compact?: boolean;
  footer?: ReactNode;
  className?: string;
}) {
  const scroller = useRef<HTMLDivElement>(null);
  const [atBottom, setAtBottom] = useState(true);
  const last = messages.at(-1);
  const contentKey = `${messages.length}:${last?.content.length ?? 0}:${last?.reasoning?.length ?? 0}:${last?.stage}:${last?.status}`;

  useLayoutEffect(() => {
    const el = scroller.current;
    if (el && atBottom) el.scrollTop = el.scrollHeight;
  }, [contentKey, atBottom]);

  // New conversation → start at the bottom (render-time adjustment).
  const conversationId = messages[0]?.conversationId;
  const [shown, setShown] = useState(conversationId);
  if (shown !== conversationId) {
    setShown(conversationId);
    setAtBottom(true);
  }

  const onScroll = () => {
    const el = scroller.current;
    if (!el) return;
    setAtBottom(el.scrollHeight - el.scrollTop - el.clientHeight < 40);
  };

  return (
    <div className={cn("relative min-h-0 flex-1", className)}>
      <div ref={scroller} onScroll={onScroll} className="h-full overflow-y-auto">
        <div className={cn("mx-auto w-full", compact ? "px-1 py-2" : "max-w-[720px] px-6 py-8")}>
          <MessageList
            messages={messages}
            compact={compact}
            {...(onRetry && { onRetry })}
            {...(onRegenerate && { onRegenerate })}
          />
          {footer}
        </div>
      </div>
      {!atBottom && (
        <button
          type="button"
          aria-label="Jump to latest"
          onClick={() => {
            const el = scroller.current;
            if (el) el.scrollTo({ top: el.scrollHeight, behavior: "smooth" });
          }}
          className="absolute bottom-3 left-1/2 flex h-8 w-8 -translate-x-1/2 items-center justify-center rounded-full border border-border-strong bg-surface text-text-muted shadow-[var(--shadow-md)] hover:text-text"
        >
          <ArrowDownIcon size={15} />
        </button>
      )}
    </div>
  );
}
