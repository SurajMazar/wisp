import type { AssistantState } from "@wisp/contracts";
import { WispMark } from "@wisp/icons";
import { Button } from "@wisp/ui";
import { useEffect } from "react";
import { useAssistantStore } from "../../stores/assistantStore";

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

/** Content of the floating assistant that expands out of the buddy (spec §91). */
export function AssistantPanel() {
  const { state, dismiss } = useAssistantStore();

  useEffect(() => {
    document.documentElement.dataset.surface = "panel";
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") void dismiss();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dismiss]);

  return (
    <div className="flex h-full flex-col px-4 pb-3 pt-3">
      <header className="flex items-center gap-2">
        <WispMark size={16} />
        <span className="text-base font-semibold">Wisp</span>
        <span className="ml-auto text-sm text-text-faint" role="status" aria-live="polite">
          {stateText[state]}
        </span>
      </header>

      <div className="flex flex-1 flex-col justify-center">
        <p className="text-lg font-medium tracking-tight">What can I help with?</p>
        <p className="mt-1 text-text-muted">
          Asking questions becomes available once an AI model is connected.
        </p>
      </div>

      <footer className="flex items-center justify-between text-sm text-text-faint">
        <span>Esc or click away to close</span>
        <Button size="sm" variant="ghost" onClick={() => void dismiss()}>
          Close
        </Button>
      </footer>
    </div>
  );
}
