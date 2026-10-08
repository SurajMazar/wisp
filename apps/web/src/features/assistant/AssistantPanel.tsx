import { WispMark } from "@wisp/icons";
import { Button } from "@wisp/ui";
import { useEffect } from "react";
import { formatShortcut } from "../../lib/shortcut";
import { useAssistantStore } from "../../stores/assistantStore";
import { useSettingsStore } from "../../stores/settingsStore";

const sourceLabel = {
  keyboard: "keyboard shortcut",
  mouse: "mouse button",
  menuBar: "menu bar",
  dock: "Dock",
  commandPalette: "command palette",
} as const;

/** Content of the floating assistant panel (spec §91). */
export function AssistantPanel() {
  const { phase, source, behavior, dismiss } = useAssistantStore();
  const shortcut = useSettingsStore((s) => s.settings?.invocation.shortcut);

  useEffect(() => {
    document.documentElement.dataset.surface = "panel";
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") void dismiss();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dismiss]);

  return (
    <div className="flex h-full flex-col px-4 pb-3 pt-3" aria-live="polite">
      <header className="flex items-center gap-2">
        <WispMark size={16} />
        <span className="text-base font-semibold">Wisp</span>
        <span className="ml-auto text-sm text-text-faint">
          {phase === "active" && source
            ? `via ${sourceLabel[source]}${behavior === "hold" ? " · hold" : ""}`
            : ""}
        </span>
      </header>

      <div className="flex flex-1 flex-col justify-center">
        <p className="text-lg font-medium tracking-tight">What can I help with?</p>
        <p className="mt-1 text-text-muted">
          Asking questions becomes available once an AI model is connected.
        </p>
      </div>

      <footer className="flex items-center justify-between text-sm text-text-faint">
        <span>
          {behavior === "hold" && shortcut ? `Release ${formatShortcut(shortcut)} to close` : "Esc to close"}
        </span>
        <Button size="sm" variant="ghost" onClick={() => void dismiss()}>
          Close
        </Button>
      </footer>
    </div>
  );
}
