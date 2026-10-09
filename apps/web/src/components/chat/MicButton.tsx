import { Button, cn } from "@wisp/ui";
import { useVoiceStore } from "../../stores/voiceStore";

/** Click to talk, click again to ask. Shows the live microphone level while listening. */
export function MicButton({ disabled }: { disabled?: boolean }) {
  const { listening, level, start, stop } = useVoiceStore();
  return (
    <Button
      size="sm"
      variant={listening ? "primary" : "ghost"}
      disabled={disabled}
      aria-pressed={listening}
      aria-label={listening ? "Stop listening and ask" : "Talk"}
      title={listening ? "Stop and ask" : "Talk (or hold your shortcut)"}
      onClick={() => void (listening ? stop() : start("ask"))}
      className="relative gap-1"
    >
      <svg
        width="14"
        height="14"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        aria-hidden
      >
        <rect x="9" y="3" width="6" height="11" rx="3" />
        <path d="M5 11a7 7 0 0 0 14 0M12 18v3" />
      </svg>
      {listening && (
        <span aria-hidden className="flex h-3 items-end gap-px">
          {[0.6, 1, 0.75].map((k, i) => (
            <span
              key={i}
              className={cn("w-[3px] rounded-full bg-current transition-[height] duration-75")}
              style={{ height: `${Math.max(3, level * k * 12)}px` }}
            />
          ))}
        </span>
      )}
    </Button>
  );
}
