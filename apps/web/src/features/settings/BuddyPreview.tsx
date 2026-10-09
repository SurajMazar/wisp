import type { AssistantState, CursorSettings } from "@wisp/contracts";
import { accentColor } from "./accents";

const stateLabel: Partial<Record<AssistantState, string>> = {
  listening: "Listening…",
  selecting: "Point at something",
  drawing: "Drawing…",
  analyzing: "Looking…",
  thinking: "Thinking…",
  speaking: "Speaking…",
  executing: "Working…",
  waitingForPermission: "Waiting for you",
  success: "Done",
  error: "Something went wrong",
};

const stateColor = (s: AssistantState, accent: string) =>
  s === "waitingForPermission" ? "#e2a33b" : s === "success" ? "#3fb86e" : s === "error" ? "#e5484d" : accent;

/** In-settings preview of the buddy. Mirrors BuddyView.swift's look; the real one is native. */
export function BuddyPreview({
  cursor,
  state,
  reduceMotion,
}: {
  cursor: CursorSettings;
  state: AssistantState;
  reduceMotion: boolean;
}) {
  const accent = accentColor(cursor.theme, cursor.customColor);
  const secondary = cursor.theme === "default" ? "#5ec8f2" : accent;
  const ring = stateColor(state, accent);
  const orb = 16 * cursor.size;
  const animate = !reduceMotion && cursor.animation !== "minimal";
  const label = stateLabel[state] ?? (cursor.showLabel ? cursor.label : null);
  const ringAnim =
    !animate || ["idle", "selecting", "drawing", "success", "error"].includes(state)
      ? undefined
      : ["thinking", "analyzing", "executing"].includes(state)
        ? `wisp-spin ${state === "executing" ? 2.4 : 1}s linear infinite`
        : state === "waitingForPermission"
          ? "wisp-blink 1.4s ease-in-out infinite"
          : "wisp-pulse 1.1s ease-out infinite";

  return (
    <div
      aria-hidden
      className="relative flex h-28 items-center justify-center overflow-hidden rounded-[var(--radius-md)] border border-border bg-[repeating-linear-gradient(45deg,var(--color-bg-sunken)_0_10px,var(--color-bg)_10px_20px)]"
    >
      <style>{`
        @keyframes wisp-pulse { from { transform: scale(1); opacity: .9 } to { transform: scale(1.4); opacity: 0 } }
        @keyframes wisp-spin { to { transform: rotate(-360deg) } }
        @keyframes wisp-blink { 50% { opacity: .35 } }
        @keyframes wisp-breathe { 50% { transform: scale(1.05) } }
      `}</style>
      {/* pointer */}
      <svg
        width="18"
        height="18"
        viewBox="0 0 18 18"
        className="absolute"
        style={{ left: "calc(50% - 30px)", top: "calc(50% - 28px)" }}
      >
        <path
          d="M2 1.5 L2 15 L5.6 11.6 L8.1 17 L10.4 16 L7.9 10.8 L12.8 10.8 Z"
          fill="#111"
          stroke="#fff"
          strokeWidth="1.2"
          strokeLinejoin="round"
        />
      </svg>
      <div
        className="relative flex items-center gap-2"
        style={{ opacity: cursor.opacity, marginLeft: 12, marginTop: 14 }}
      >
        <div
          className="relative flex items-center justify-center"
          style={{ width: orb + 14, height: orb + 14 }}
        >
          {state !== "idle" && (
            <span
              className="absolute rounded-full"
              style={{
                inset: 0,
                border: `${1.6 * cursor.size}px ${["selecting", "drawing", "executing"].includes(state) ? "dashed" : "solid"} ${ring}`,
                borderTopColor: ["thinking", "analyzing"].includes(state) && animate ? "transparent" : ring,
                borderRightColor: ["thinking", "analyzing"].includes(state) && animate ? "transparent" : ring,
                opacity: state === "following" ? 0.45 : 0.9,
                animation: ringAnim,
              }}
            />
          )}
          <span
            className="rounded-full"
            style={{
              width: orb,
              height: orb,
              background: `radial-gradient(circle at 42% 38%, #fff 0%, color-mix(in srgb, ${accent} 65%, white) 35%, ${accent} 100%)`,
              boxShadow: `0 0 ${(3 + 9 * cursor.glow) * cursor.size}px ${(1 + 3 * cursor.glow) * cursor.size}px color-mix(in srgb, ${secondary} ${Math.round(25 + 60 * cursor.glow)}%, transparent)`,
              opacity: state === "idle" ? 0.88 : 1,
              animation: state === "idle" && animate ? "wisp-breathe 2.6s ease-in-out infinite" : undefined,
            }}
          />
        </div>
        {label && (
          <span className="flex items-center gap-1.5 rounded-full border border-border bg-surface/90 px-2 py-0.5 text-xs font-medium shadow-[var(--shadow-sm)]">
            <span className="h-1.5 w-1.5 rounded-full" style={{ background: ring }} />
            {label}
          </span>
        )}
      </div>
    </div>
  );
}
