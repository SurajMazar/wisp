import type { AssistantState } from "@wisp/contracts";
import { WispMark } from "@wisp/icons";
import { Button, cn } from "@wisp/ui";
import { useEffect, useState } from "react";
import { Composer } from "../../components/chat/Composer";
import { MicButton } from "../../components/chat/MicButton";
import { MessageList } from "../../components/chat/MessageList";
import { useAssistantStore } from "../../stores/assistantStore";
import { streamingMessage, useConversationStore } from "../../stores/conversationStore";
import { useFocusStore } from "../../stores/focusStore";
import { FocusActions } from "./FocusActions";
import { FocusCard } from "./FocusCard";

const stateText: Record<AssistantState, string> = {
  idle: "Ready",
  following: "Ready",
  listening: "Listening…",
  selecting: "Point at something",
  drawing: "Drawing…",
  analyzing: "Looking at your screen…",
  thinking: "Thinking…",
  speaking: "Speaking…",
  executing: "Working…",
  waitingForPermission: "Waiting for your permission",
  success: "Done",
  error: "Something went wrong",
};

/** The floating assistant (spec §91): ask about what you're pointing at, answers stream in. */
export function AssistantPanel() {
  const { state, dismiss } = useAssistantStore();
  const region = useFocusStore((s) => s.region);
  const { panelId, conversations, send, cancel, setPanel, error } = useConversationStore();
  const conversation = panelId ? conversations[panelId] : undefined;
  const streaming = streamingMessage(conversation);
  const [attachFocus, setAttachFocus] = useState(true);
  const [focusSeen, setFocusSeen] = useState(region?.id);
  if (region?.id !== focusSeen) {
    // A new selection re-enables attaching.
    setFocusSeen(region?.id);
    setAttachFocus(true);
  }

  useEffect(() => {
    document.documentElement.dataset.surface = "panel";
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") void dismiss();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dismiss]);

  const ask = async (text: string) => {
    const id = await send(text, { conversationId: panelId, includeFocus: !!region && attachFocus });
    if (id) {
      setPanel(id);
      setAttachFocus(false); // attach a selection once; select again to include a new one
    }
  };

  const status = streaming ? (streaming.content ? "Answering…" : "Thinking…") : stateText[state];

  return (
    <div className="flex h-full flex-col px-3 pb-3 pt-2.5">
      <header className="flex items-center gap-2 px-1 pb-2">
        <WispMark size={16} />
        <span className="text-base font-semibold">Wisp</span>
        <span className="text-sm text-text-faint" role="status" aria-live="polite">
          · {status}
        </span>
        <span className="ml-auto flex gap-1">
          {conversation && (
            <Button size="sm" variant="ghost" onClick={() => setPanel(null)} title="Start a new conversation">
              New
            </Button>
          )}
          <Button size="sm" variant="ghost" onClick={() => void dismiss()} aria-label="Close">
            Close
          </Button>
        </span>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto px-1">
        {conversation && conversation.messages.length > 0 ? (
          <MessageList messages={conversation.messages} compact onRetry={(q) => void ask(q)} />
        ) : (
          <div className="flex h-full flex-col justify-center gap-3">
            <div>
              <p className="text-lg font-medium tracking-tight">What can I help with?</p>
              <p className="mt-0.5 text-sm text-text-muted">
                Hold your shortcut and drag around something, or choose what to focus on.
              </p>
            </div>
            <FocusActions />
          </div>
        )}
      </div>

      <div className="mt-2 space-y-1.5">
        {error && (
          <p className="px-1 text-xs text-danger" role="alert">
            {error}
          </p>
        )}
        {region && conversation && <FocusActions />}
        <Composer
          autoFocus
          streaming={!!streaming}
          onStop={() => streaming && void cancel(streaming.id)}
          onSend={(t) => void ask(t)}
          accessory={<MicButton disabled={!!streaming} />}
          placeholder={region && attachFocus ? "Ask about what you selected…" : "Ask anything…"}
          attachment={
            region ? (
              <div className={cn("mb-1.5", !attachFocus && "opacity-50")}>
                <FocusCard region={region} />
                <label className="mt-1 flex items-center gap-1.5 px-1 text-xs text-text-muted">
                  <input
                    type="checkbox"
                    checked={attachFocus}
                    onChange={(e) => setAttachFocus(e.target.checked)}
                  />
                  Include this in my next question
                </label>
              </div>
            ) : null
          }
        />
      </div>
    </div>
  );
}
