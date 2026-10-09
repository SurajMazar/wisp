import type { AssistantState } from "@wisp/contracts";
import { CloseIcon, PlusIcon, WispMark } from "@wisp/icons";
import { useEffect } from "react";
import { ChatThread } from "../../components/chat/ChatThread";
import { Composer } from "../../components/chat/Composer";
import { useComposerFocus } from "../../components/chat/useComposerFocus";
import { useAssistantStore } from "../../stores/assistantStore";
import { streamingMessage, useConversationStore } from "../../stores/conversationStore";
import { useFocusStore } from "../../stores/focusStore";
import { FocusActions } from "./FocusActions";

const stateText: Record<AssistantState, string> = {
  idle: "",
  following: "",
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
  const { panelId, conversations, send, cancel, setPanel, regenerate, error } = useConversationStore();
  const conversation = panelId ? conversations[panelId] : undefined;
  const streaming = streamingMessage(conversation);
  const { focus, includeFocus, consumed } = useComposerFocus();

  useEffect(() => {
    document.documentElement.dataset.surface = "panel";
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") void dismiss();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dismiss]);

  const ask = async (text: string) => {
    const id = await send(text, { conversationId: panelId, includeFocus });
    if (id) {
      setPanel(id);
      consumed(); // attach a selection once; select again to include a new one
    }
  };

  const status = streaming ? "" : stateText[state];
  const hasMessages = !!conversation && conversation.messages.length > 0;

  return (
    <div className="flex h-full flex-col px-3 pb-3 pt-2.5">
      <header className="flex items-center gap-2 px-1 pb-1.5">
        <WispMark size={16} />
        <span className="text-base font-semibold">Wisp</span>
        {status && (
          <span className="text-sm text-text-faint" role="status" aria-live="polite">
            {status}
          </span>
        )}
        <span className="ml-auto flex gap-0.5">
          {conversation && (
            <button
              type="button"
              aria-label="New chat"
              title="New chat"
              onClick={() => setPanel(null)}
              className="rounded-[var(--radius-sm)] p-1.5 text-text-muted hover:bg-surface-hover hover:text-text"
            >
              <PlusIcon size={15} />
            </button>
          )}
          <button
            type="button"
            aria-label="Close"
            title="Close (Esc)"
            onClick={() => void dismiss()}
            className="rounded-[var(--radius-sm)] p-1.5 text-text-muted hover:bg-surface-hover hover:text-text"
          >
            <CloseIcon size={15} />
          </button>
        </span>
      </header>

      {hasMessages ? (
        <ChatThread
          compact
          messages={conversation.messages}
          onRetry={(q) => void ask(q)}
          onRegenerate={(id) => void regenerate(id)}
        />
      ) : (
        <div className="flex flex-1 flex-col justify-center gap-3 px-1">
          <div>
            <p className="text-lg font-medium tracking-tight">What can I help with?</p>
            <p className="mt-0.5 text-sm text-text-muted">
              Hold your shortcut and drag around something, or pick what to look at.
            </p>
          </div>
          {!region && <FocusActions />}
        </div>
      )}

      <div className="mt-2">
        {error && (
          <p className="mb-1 px-1 text-xs text-danger" role="alert">
            {error}
          </p>
        )}
        <Composer
          compact
          autoFocus
          focus={focus}
          streaming={!!streaming}
          onStop={() => streaming && void cancel(streaming.id)}
          onSend={(t) => void ask(t)}
          placeholder={includeFocus ? "Ask about what you selected" : "Ask anything"}
        />
      </div>
    </div>
  );
}
