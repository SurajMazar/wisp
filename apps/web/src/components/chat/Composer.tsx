import { Button, cn } from "@wisp/ui";
import { useState, type KeyboardEvent } from "react";

export function Composer({
  onSend,
  onStop,
  streaming,
  placeholder = "Ask anything…",
  autoFocus,
  attachment,
}: {
  onSend: (text: string) => void;
  onStop?: () => void;
  streaming: boolean;
  placeholder?: string;
  autoFocus?: boolean;
  attachment?: React.ReactNode;
}) {
  const [text, setText] = useState("");
  const submit = () => {
    const t = text.trim();
    if (!t || streaming) return;
    onSend(t);
    setText("");
  };
  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      submit();
    }
  };
  return (
    <div className="rounded-[var(--radius-lg)] border border-border-strong bg-surface p-2 shadow-[var(--shadow-sm)] focus-within:border-accent/60">
      {attachment}
      <textarea
        aria-label="Message"
        rows={Math.min(6, Math.max(1, text.split("\n").length))}
        className="block w-full resize-none bg-transparent px-1 text-base outline-none placeholder:text-text-faint"
        placeholder={placeholder}
        value={text}
        autoFocus={autoFocus}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={onKey}
      />
      <div className="mt-1 flex items-center justify-end gap-2">
        <span className={cn("text-xs text-text-faint", streaming && "invisible")}>
          Enter to send · Shift+Enter for a new line
        </span>
        {streaming ? (
          <Button size="sm" onClick={onStop}>
            Stop
          </Button>
        ) : (
          <Button size="sm" variant="primary" disabled={!text.trim()} onClick={submit}>
            Send
          </Button>
        )}
      </div>
    </div>
  );
}
