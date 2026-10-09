import { MicIcon } from "@wisp/icons";
import { cn } from "@wisp/ui";
import { useVoiceStore } from "../../stores/voiceStore";

/** Click to talk, click again to ask. Shows the live microphone level while listening. */
export function MicButton({ disabled }: { disabled?: boolean }) {
  const { listening, level, start, stop } = useVoiceStore();
  return (
    <button
      type="button"
      disabled={disabled}
      aria-pressed={listening}
      aria-label={listening ? "Stop listening and ask" : "Talk"}
      title={listening ? "Stop and ask" : "Talk (or hold your shortcut)"}
      onClick={() => void (listening ? stop() : start("ask"))}
      className={cn(
        "flex h-7 items-center gap-1 rounded-full px-2 text-text-muted transition-colors hover:bg-surface-hover hover:text-text disabled:opacity-40",
        listening && "bg-accent px-2.5 text-accent-contrast hover:bg-accent hover:text-accent-contrast",
      )}
    >
      <MicIcon size={15} />
      {listening && (
        <span aria-hidden className="flex h-3 items-end gap-px">
          {[0.6, 1, 0.75, 0.9].map((k, i) => (
            <span
              key={i}
              className="w-[3px] rounded-full bg-current transition-[height] duration-75"
              style={{ height: `${Math.max(3, level * k * 12)}px` }}
            />
          ))}
        </span>
      )}
    </button>
  );
}
