import type { AskContext, DrawStyle, FocusSettings } from "@wisp/contracts";
import { Segmented, SettingsGroup, SettingsRow, Slider, Switch } from "@wisp/ui";
import { useState } from "react";
import { useSettingsStore } from "../../stores/settingsStore";
import { useFocusStore } from "../../stores/focusStore";

/** Screen & focus settings (spec §31–33, §81). */
export function FocusSettingsGroup({ focus }: { focus: FocusSettings }) {
  const update = useSettingsStore((s) => s.update);
  const permission = useFocusStore((s) => s.permission);
  const requestPermission = useFocusStore((s) => s.requestPermission);
  const [draft, setDraft] = useState(focus);
  const [synced, setSynced] = useState(focus);
  if (synced !== focus) {
    setSynced(focus);
    setDraft(focus);
  }
  const save = (patch: Partial<FocusSettings>) => {
    const next = { ...draft, ...patch };
    setDraft(next);
    void update({ section: "focus", value: next });
  };

  return (
    <SettingsGroup
      title="Screen & focus"
      description="Wisp looks at your screen only while you hold the shortcut or select something — never continuously. Password managers are never captured unless you draw on them. Captures stay on this Mac."
    >
      <SettingsRow
        label="Screen access"
        hint={permission === "enabled" ? "Allowed" : "Needed to see what you point at"}
        control={
          permission === "enabled" ? (
            <span className="text-sm text-success">Enabled</span>
          ) : (
            <button
              type="button"
              className="text-sm text-accent hover:underline"
              onClick={() => void requestPermission()}
            >
              Allow…
            </button>
          )
        }
      />
      <SettingsRow
        label="When you ask out loud"
        hint="What Wisp looks at if you don't draw anything. Drawing always wins."
        control={
          <Segmented<AskContext>
            label="Screen context for spoken questions"
            value={draft.askContext}
            onValueChange={(askContext) => save({ askContext })}
            options={[
              { value: "window", label: "Window" },
              { value: "display", label: "Whole screen" },
              { value: "off", label: "Nothing" },
            ]}
          />
        }
      />
      <SettingsRow
        label="Draw shape"
        hint="What dragging draws when you pick Area in the panel. Holding the shortcut always draws freehand."
        control={
          <Segmented<DrawStyle>
            label="Focus draw shape"
            value={draft.drawStyle}
            onValueChange={(drawStyle) => save({ drawStyle })}
            options={[
              { value: "rectangle", label: "Rectangle" },
              { value: "lasso", label: "Lasso" },
              { value: "circle", label: "Circle" },
            ]}
          />
        }
      />
      <SettingsRow
        label="Dim surroundings"
        control={
          <Switch
            label="Dim surroundings while selecting"
            checked={draft.dimSurroundings}
            onCheckedChange={(dimSurroundings) => save({ dimSurroundings })}
          />
        }
      />
      <SettingsRow
        label="Context around selection"
        hint={`${draft.margin} pt`}
        control={
          <Slider
            label="Context margin"
            value={draft.margin}
            min={0}
            max={120}
            step={4}
            onValueChange={(margin) => setDraft({ ...draft, margin })}
            onValueCommit={(margin) => save({ margin })}
          />
        }
      />
      <SettingsRow
        label="Point focus area"
        hint={`${draft.pointSize} × ${draft.pointSize} pt around a clicked point`}
        control={
          <Slider
            label="Point focus size"
            value={draft.pointSize}
            min={160}
            max={1200}
            step={40}
            onValueChange={(pointSize) => setDraft({ ...draft, pointSize })}
            onValueCommit={(pointSize) => save({ pointSize })}
          />
        }
      />
    </SettingsGroup>
  );
}
