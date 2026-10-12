import type {
  AnimationLevel,
  AssistantState,
  CursorSettings,
  CursorShape,
  CursorVisibility,
  GlowMode,
} from "@wisp/contracts";
import { Button, Segmented, SettingsGroup, SettingsRow, Slider, Switch, cn } from "@wisp/ui";
import { useEffect, useState } from "react";
import { ipc } from "../../services/ipc";
import { useSettingsStore } from "../../stores/settingsStore";
import { accents } from "./accents";
import { BuddyPreview } from "./BuddyPreview";
import { glowPresets, shapePaths } from "./buddyShapes";

const shapes: { value: CursorShape; label: string }[] = [
  { value: "wisp", label: "Wisp" },
  { value: "pointer", label: "Pointer" },
  { value: "dot", label: "Dot" },
];

const previewStates: { value: AssistantState; label: string }[] = [
  { value: "idle", label: "Idle" },
  { value: "following", label: "Summoned" },
  { value: "listening", label: "Listening" },
  { value: "thinking", label: "Thinking" },
  { value: "speaking", label: "Speaking" },
  { value: "executing", label: "Working" },
  { value: "waitingForPermission", label: "Waiting" },
  { value: "success", label: "Done" },
  { value: "error", label: "Error" },
];

const onScreenTour: AssistantState[] = [
  "following",
  "listening",
  "thinking",
  "speaking",
  "executing",
  "waitingForPermission",
  "success",
  "idle",
];

function usePrefersReducedMotion() {
  const [reduce, setReduce] = useState(() => window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  useEffect(() => {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const on = () => setReduce(mq.matches);
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, []);
  return reduce;
}

/** Assistant cursor settings with a live preview (spec §29, §81). */
export function CursorSettingsGroup({ cursor }: { cursor: CursorSettings }) {
  const update = useSettingsStore((s) => s.update);
  const saving = useSettingsStore((s) => s.saving === "cursor");
  const [draft, setDraft] = useState(cursor);
  const [previewState, setPreviewState] = useState<AssistantState>("idle");
  const [labelText, setLabelText] = useState(cursor.label);
  const [touring, setTouring] = useState(false);
  const reduceMotion = usePrefersReducedMotion();

  // Re-sync local drafts when saved settings change (render-time adjustment, not an effect).
  const [synced, setSynced] = useState(cursor);
  if (synced !== cursor) {
    setSynced(cursor);
    setDraft(cursor);
    setLabelText(cursor.label);
  }

  const save = (patch: Partial<CursorSettings>) => {
    const next = { ...draft, ...patch };
    setDraft(next);
    void update({ section: "cursor", value: next });
  };

  const showOnScreen = async () => {
    setTouring(true);
    try {
      await ipc().call("host.cursor.preview", { states: onScreenTour, stepMs: 1200 });
    } finally {
      setTimeout(() => setTouring(false), onScreenTour.length * 1200);
    }
  };

  const commitLabel = () => {
    const label = labelText.trim().slice(0, 24);
    if (label && label !== draft.label) save({ label });
    else setLabelText(draft.label);
  };

  return (
    <SettingsGroup
      title="Assistant cursor"
      description="A second cursor that trails yours and shows what Wisp is doing. It never gets in the way of clicks. Tap your shortcut to open Wisp; hold it to talk and point."
    >
      <div className="space-y-2 px-3 py-3">
        <BuddyPreview cursor={draft} state={previewState} reduceMotion={reduceMotion} />
        <div className="flex flex-wrap items-center gap-1" role="group" aria-label="Preview state">
          {previewStates.map((s) => (
            <button
              key={s.value}
              type="button"
              aria-pressed={previewState === s.value}
              onClick={() => setPreviewState(s.value)}
              className={cn(
                "h-6 rounded-[var(--radius-sm)] px-2 text-sm text-text-muted hover:bg-surface-hover hover:text-text",
                previewState === s.value && "bg-surface-hover text-text",
              )}
            >
              {s.label}
            </button>
          ))}
          <Button
            size="sm"
            className="ml-auto"
            disabled={!draft.enabled || touring}
            onClick={() => void showOnScreen()}
          >
            {touring ? "Watch your pointer…" : "Preview on screen"}
          </Button>
        </div>
      </div>

      <SettingsRow
        label="Show status at the notch"
        hint="A small capsule at the top of the screen shows when Wisp is listening, thinking or speaking, with a caption of the answer."
        control={
          <Switch
            label="Show status at the notch"
            checked={draft.showIsland}
            disabled={saving}
            onCheckedChange={(showIsland) => save({ showIsland })}
          />
        }
      />
      <SettingsRow
        label="Show assistant cursor"
        control={
          <Switch
            label="Show assistant cursor"
            checked={draft.enabled}
            disabled={saving}
            onCheckedChange={(enabled) => save({ enabled })}
          />
        }
      />
      <SettingsRow
        label="Visible"
        control={
          <Segmented<CursorVisibility>
            label="When the assistant cursor is visible"
            value={draft.visibility}
            disabled={!draft.enabled}
            onValueChange={(visibility) => save({ visibility })}
            options={[
              { value: "always", label: "Always" },
              { value: "whenSummoned", label: "When summoned" },
            ]}
          />
        }
      />
      <SettingsRow
        label="Shape"
        control={
          <div role="radiogroup" aria-label="Cursor shape" className="flex gap-1.5">
            {shapes.map((sh) => (
              <button
                key={sh.value}
                type="button"
                role="radio"
                aria-checked={draft.shape === sh.value}
                aria-label={`${sh.label} shape`}
                title={sh.label}
                onClick={() => save({ shape: sh.value })}
                className={cn(
                  "flex h-9 w-12 flex-col items-center justify-center gap-0.5 rounded-[var(--radius-sm)] border text-[10px] text-text-muted",
                  draft.shape === sh.value
                    ? "border-accent bg-accent-soft text-text"
                    : "border-border hover:bg-surface-hover",
                )}
              >
                <svg width="16" height="16" viewBox="0 0 20 20" aria-hidden>
                  <path d={shapePaths[sh.value]} fill="currentColor" />
                </svg>
                {sh.label}
              </button>
            ))}
          </div>
        }
      />
      <SettingsRow
        label="Color"
        control={
          <div role="radiogroup" aria-label="Assistant cursor color" className="flex items-center gap-1.5">
            {accents.map((a) => (
              <button
                key={a.value}
                type="button"
                role="radio"
                aria-checked={draft.theme === a.value}
                aria-label={`Cursor ${a.label}`}
                title={a.label}
                onClick={() => save({ theme: a.value })}
                className={cn(
                  "h-5 w-5 rounded-full ring-offset-2 ring-offset-surface",
                  draft.theme === a.value
                    ? "ring-2 ring-text-muted"
                    : "hover:ring-2 hover:ring-border-strong",
                )}
                style={{ background: a.swatch }}
              />
            ))}
            <label
              title="Custom color"
              className={cn(
                "relative h-5 w-5 cursor-pointer overflow-hidden rounded-full ring-offset-2 ring-offset-surface [background:conic-gradient(red,yellow,lime,cyan,blue,magenta,red)]",
                draft.theme === "custom" ? "ring-2 ring-text-muted" : "hover:ring-2 hover:ring-border-strong",
              )}
            >
              <input
                type="color"
                aria-label="Custom cursor color"
                className="absolute inset-0 cursor-pointer opacity-0"
                value={draft.customColor ?? "#7c6cf2"}
                onChange={(e) => setDraft({ ...draft, theme: "custom", customColor: e.target.value })}
                onBlur={(e) => save({ theme: "custom", customColor: e.target.value })}
              />
            </label>
          </div>
        }
      />
      {(
        [
          ["Size", "size", 0.6, 1.6, 0.05],
          ["Opacity", "opacity", 0.3, 1, 0.05],
          ["Glow", "glow", 0, 1, 0.05],
        ] as const
      ).map(([label, key, min, max, step]) => (
        <SettingsRow
          key={key}
          label={label}
          control={
            <Slider
              label={`Cursor ${label.toLowerCase()}`}
              value={draft[key]}
              min={min}
              max={max}
              step={step}
              disabled={!draft.enabled}
              onValueChange={(v) => setDraft({ ...draft, [key]: v })}
              onValueCommit={(v) => save({ [key]: v })}
            />
          }
        />
      ))}
      <SettingsRow
        label="Glow color"
        control={
          <Segmented<GlowMode>
            label="Glow color"
            value={draft.glowMode}
            disabled={!draft.enabled}
            onValueChange={(glowMode) => save({ glowMode })}
            options={[
              { value: "accent", label: "Cursor color" },
              { value: "gradient", label: "Gradient" },
            ]}
          />
        }
      />
      {draft.glowMode === "gradient" && (
        <SettingsRow
          label="Gradient"
          hint={
            glowPresets.find((p) => p.from === draft.glowFrom && p.to === draft.glowTo)?.label ?? "Custom"
          }
          control={
            <div className="flex items-center gap-1.5">
              <div role="radiogroup" aria-label="Glow gradient" className="flex gap-1.5">
                {glowPresets.map((p) => {
                  const on = p.from === draft.glowFrom && p.to === draft.glowTo;
                  return (
                    <button
                      key={p.label}
                      type="button"
                      role="radio"
                      aria-checked={on}
                      aria-label={`${p.label} glow`}
                      title={p.label}
                      onClick={() => save({ glowFrom: p.from, glowTo: p.to })}
                      className={cn(
                        "h-5 w-5 rounded-full ring-offset-2 ring-offset-surface",
                        on ? "ring-2 ring-text-muted" : "hover:ring-2 hover:ring-border-strong",
                      )}
                      style={{ background: `linear-gradient(135deg, ${p.from}, ${p.to})` }}
                    />
                  );
                })}
              </div>
              <input
                type="color"
                aria-label="Glow start color"
                className="h-5 w-6 cursor-pointer rounded border border-border-strong bg-transparent"
                value={draft.glowFrom.toLowerCase()}
                onChange={(e) => setDraft({ ...draft, glowFrom: e.target.value.toUpperCase() })}
                onBlur={(e) => save({ glowFrom: e.target.value.toUpperCase() })}
              />
              <input
                type="color"
                aria-label="Glow end color"
                className="h-5 w-6 cursor-pointer rounded border border-border-strong bg-transparent"
                value={draft.glowTo.toLowerCase()}
                onChange={(e) => setDraft({ ...draft, glowTo: e.target.value.toUpperCase() })}
                onBlur={(e) => save({ glowTo: e.target.value.toUpperCase() })}
              />
            </div>
          }
        />
      )}
      <SettingsRow
        label="Animation"
        hint={reduceMotion ? "Reduce Motion is on in macOS, so the cursor stays still." : undefined}
        control={
          <Segmented<AnimationLevel>
            label="Cursor animation"
            value={draft.animation}
            disabled={!draft.enabled}
            onValueChange={(animation) => save({ animation })}
            options={[
              { value: "minimal", label: "Minimal" },
              { value: "normal", label: "Normal" },
              { value: "expressive", label: "Expressive" },
            ]}
          />
        }
      />
      <SettingsRow
        label="Show label"
        control={
          <div className="flex items-center gap-2">
            <input
              aria-label="Cursor label"
              className="h-6 w-28 rounded-[var(--radius-sm)] border border-border-strong bg-surface px-2 text-sm disabled:opacity-45"
              value={labelText}
              maxLength={24}
              disabled={!draft.enabled || !draft.showLabel}
              onChange={(e) => setLabelText(e.target.value)}
              onBlur={commitLabel}
              onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
            />
            <Switch
              label="Show cursor label"
              checked={draft.showLabel}
              disabled={!draft.enabled}
              onCheckedChange={(showLabel) => save({ showLabel })}
            />
          </div>
        }
      />
    </SettingsGroup>
  );
}
