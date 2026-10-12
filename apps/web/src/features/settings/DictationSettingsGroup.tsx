import type { DictationSettings, InvocationSettings, Modifier } from "@wisp/contracts";
import { Segmented, SettingsGroup, SettingsRow, Switch } from "@wisp/ui";
import { formatShortcut } from "../../lib/shortcut";
import { useSettingsStore } from "../../stores/settingsStore";

/** Chords to hold while dictating. Modifier-only, so they never type anything themselves. */
const choices: { value: string; modifiers: Modifier[] }[] = [
  { value: "control+shift", modifiers: ["control", "shift"] },
  { value: "option+shift", modifiers: ["option", "shift"] },
  { value: "control+command", modifiers: ["control", "command"] },
];
const key = (m: Modifier[]) => [...m].sort().join("+");

/** Hold a shortcut and talk: Wisp types what you say where your cursor is (plan Step 5). */
export function DictationSettingsGroup({
  dictation,
  invocation,
}: {
  dictation: DictationSettings;
  invocation: InvocationSettings;
}) {
  const update = useSettingsStore((s) => s.update);
  const save = (patch: Partial<DictationSettings>) =>
    void update({ section: "dictation", value: { ...dictation, ...patch } });
  const current = key(dictation.shortcut.modifiers);
  const clash = !invocation.shortcut.keyCode && key(invocation.shortcut.modifiers) === current;

  return (
    <SettingsGroup
      title="Dictation"
      description="Hold the shortcut and talk; let go and Wisp types what you said where your cursor is — in any app. Transcribed on this Mac. Never types into password fields."
    >
      <SettingsRow
        label="Hold to dictate"
        control={
          <Switch
            label="Hold to dictate"
            checked={dictation.enabled}
            onCheckedChange={(enabled) => save({ enabled })}
          />
        }
      />
      <SettingsRow
        label="Shortcut"
        hint={
          clash ? (
            <span className="text-danger">That's also your assistant shortcut — pick another.</span>
          ) : (
            `Hold ${formatShortcut(dictation.shortcut)} while you speak. Pressing another key cancels.`
          )
        }
        control={
          <Segmented<string>
            label="Dictation shortcut"
            value={choices.find((c) => key(c.modifiers) === current)?.value ?? "control+shift"}
            onValueChange={(v) => {
              const c = choices.find((x) => x.value === v);
              if (c) save({ shortcut: { modifiers: c.modifiers } });
            }}
            options={choices.map((c) => ({
              value: c.value,
              label: formatShortcut({ modifiers: c.modifiers }),
            }))}
          />
        }
      />
      <SettingsRow
        label="Clean up text"
        hint="Fix punctuation and drop “um”s with your model before typing. Adds about half a second."
        control={
          <Switch
            label="Clean up dictated text"
            checked={dictation.cleanup}
            onCheckedChange={(cleanup) => save({ cleanup })}
          />
        }
      />
    </SettingsGroup>
  );
}
