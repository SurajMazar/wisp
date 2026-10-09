import { WispMark } from "@wisp/icons";
import { useRef, useState } from "react";
import { useNavigate } from "react-router";
import { Composer } from "../../components/chat/Composer";
import { useComposerFocus } from "../../components/chat/useComposerFocus";
import { useConversationStore } from "../../stores/conversationStore";
import { useFocusStore } from "../../stores/focusStore";

function greeting(date = new Date()): string {
  const h = date.getHours();
  if (h < 5) return "Up late?";
  if (h < 12) return "Good morning";
  if (h < 18) return "Good afternoon";
  return "Good evening";
}

const suggestions: { label: string; prompt?: string; focus?: "rectangle" | "window" }[] = [
  { label: "Explain something on my screen", focus: "rectangle" },
  { label: "Summarize this window", focus: "window" },
  { label: "Draft a reply", prompt: "Help me draft a short, friendly reply to: " },
  {
    label: "Teach me a shortcut",
    prompt: "Teach me a useful macOS keyboard shortcut I probably don't know.",
  },
];

export function HomePage() {
  const { send, error } = useConversationStore();
  const startFocus = useFocusStore((s) => s.start);
  const navigate = useNavigate();
  const { focus, includeFocus, consumed } = useComposerFocus();
  const [draft, setDraft] = useState("");
  const box = useRef<HTMLDivElement>(null);

  const ask = (t: string, opts?: { teach?: boolean }) =>
    void send(t, { includeFocus, ...opts }).then((id) => {
      if (!id) return;
      consumed();
      navigate(`/conversations/${id}`);
    });

  return (
    <div className="flex flex-1 flex-col items-center justify-center overflow-y-auto px-8 pb-[12vh] pt-10">
      <div className="w-full max-w-[680px]">
        <div className="mb-7 flex items-center justify-center gap-3">
          <WispMark size={30} />
          <h1 className="text-[28px] font-medium tracking-tight">{greeting()}</h1>
        </div>
        <div ref={box}>
          <Composer
            autoFocus
            streaming={false}
            focus={focus}
            placeholder={includeFocus ? "Ask about what you selected" : "How can I help?"}
            onSend={ask}
            text={draft}
            onTextChange={setDraft}
          />
        </div>
        {error && (
          <p role="alert" className="mt-2 px-2 text-sm text-danger">
            {error}
          </p>
        )}
        <div className="mt-4 flex flex-wrap justify-center gap-2">
          {suggestions.map((s) => (
            <button
              key={s.label}
              type="button"
              onClick={() => {
                if (s.focus) void startFocus(s.focus);
                else if (s.prompt) {
                  setDraft(s.prompt);
                  box.current?.querySelector("textarea")?.focus();
                }
              }}
              className="rounded-full border border-border px-3 py-1.5 text-sm text-text-muted transition-colors hover:border-border-strong hover:bg-surface hover:text-text"
            >
              {s.label}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
