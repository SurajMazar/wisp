import type {
  ChannelState,
  InvocationAction,
  InvocationBehavior,
  InvocationSettings,
  MouseButton,
} from "@wisp/contracts";
import { Button, Segmented, SettingsGroup, SettingsRow, cn } from "@wisp/ui";
import { formatShortcut, speakShortcut } from "../../lib/shortcut";
import { useInvocationStore } from "../../stores/invocationStore";
import { useSettingsStore } from "../../stores/settingsStore";

const stateLabel: Record<ChannelState, { text: string; tone: string }> = {
  active: { text: "Active", tone: "bg-success" },
  needsPermission: { text: "Needs permission", tone: "bg-warning" },
  conflict: { text: "In use by another app", tone: "bg-danger" },
  disabled: { text: "Off", tone: "bg-text-faint" },
  unsupported: { text: "Not supported by macOS", tone: "bg-text-faint" },
};

function ChannelBadge({ state }: { state: ChannelState }) {
  const { text, tone } = stateLabel[state];
  return (
    <span className="inline-flex items-center gap-1.5 text-sm text-text-muted">
      <span aria-hidden className={cn("h-1.5 w-1.5 rounded-full", tone)} />
      {text}
    </span>
  );
}

const behaviorHints: Record<InvocationBehavior, string> = {
  hold: "Wisp appears while you hold the shortcut and finishes when you let go.",
  toggle: "Press once to summon Wisp, press again to put it away.",
  pushToTalk: "Hold to speak, release to send.",
};

export function InvocationSettingsGroup({ invocation }: { invocation: InvocationSettings }) {
  const update = useSettingsStore((s) => s.update);
  const saving = useSettingsStore((s) => s.saving === "invocation");
  const { status, recording, conflicts, message, record, openAccessibility, loadStatus } =
    useInvocationStore();
  const save = (patch: Partial<InvocationSettings>) =>
    void update({ section: "invocation", value: { ...invocation, ...patch } });

  return (
    <SettingsGroup
      title="Invocation"
      description={status?.keyboard === "needsPermission" ? undefined : behaviorHints[invocation.behavior]}
    >
      <SettingsRow
        label="Assistant shortcut"
        hint={status ? <ChannelBadge state={status.keyboard} /> : undefined}
        control={
          <div className="flex items-center gap-2">
            <kbd
              aria-label={speakShortcut(invocation.shortcut)}
              className="min-w-14 rounded-[var(--radius-sm)] border border-border-strong bg-bg-sunken px-2 py-0.5 text-center font-sans text-md tracking-wide"
            >
              {recording ? "…" : formatShortcut(invocation.shortcut)}
            </kbd>
            <Button size="sm" onClick={() => void record()} disabled={recording || saving}>
              {recording ? "Press a shortcut — Esc cancels" : "Record New Shortcut"}
            </Button>
          </div>
        }
      />

      {(message || conflicts.length > 0) && (
        <div className="px-3 py-2 text-sm" role="status">
          {message && <p className="text-text">{message}</p>}
          {conflicts.map((c) => (
            <p key={c.with} className={c.severity === "blocking" ? "text-danger" : "text-warning"}>
              {c.severity === "blocking" ? "Conflicts with" : "Also used for"} {c.with}
            </p>
          ))}
        </div>
      )}

      {status?.keyboard === "needsPermission" && (
        <div className="flex items-center justify-between gap-4 bg-warning/5 px-3 py-2.5">
          <p className="text-sm text-text-muted">
            {status.detail ?? "Wisp needs Accessibility permission to notice this shortcut in other apps."}{" "}
            Until then, use the menu bar.
          </p>
          <div className="flex shrink-0 gap-1.5">
            <Button size="sm" variant="primary" onClick={() => void openAccessibility()}>
              Open System Settings
            </Button>
            <Button size="sm" onClick={() => void loadStatus()}>
              Check Again
            </Button>
          </div>
        </div>
      )}

      <SettingsRow
        label="Behavior"
        control={
          <Segmented<InvocationBehavior>
            label="Invocation behavior"
            value={invocation.behavior}
            disabled={saving}
            onValueChange={(behavior) => save({ behavior })}
            options={[
              { value: "hold", label: "Hold" },
              { value: "toggle", label: "Toggle" },
              { value: "pushToTalk", label: "Push to Talk", disabled: true },
            ]}
          />
        }
        hint={invocation.behavior !== "pushToTalk" ? undefined : "Requires voice, which isn't available yet."}
      />

      <SettingsRow
        label="Mouse button"
        hint={
          status && invocation.mouseButton !== "none" ? (
            <ChannelBadge state={status.mouse} />
          ) : (
            "Use a side button to summon Wisp."
          )
        }
        control={
          <div className="flex gap-2">
            <select
              aria-label="Mouse button"
              className="h-6 rounded-[var(--radius-sm)] border border-border-strong bg-surface px-1.5 text-sm"
              value={invocation.mouseButton}
              disabled={saving}
              onChange={(e) => save({ mouseButton: e.target.value as MouseButton })}
            >
              <option value="none">None</option>
              <option value="back">Back side button</option>
              <option value="forward">Forward side button</option>
              <option value="middle">Middle button</option>
            </select>
            <select
              aria-label="Mouse button action"
              className="h-6 rounded-[var(--radius-sm)] border border-border-strong bg-surface px-1.5 text-sm disabled:opacity-45"
              value={invocation.mouseAction}
              disabled={saving || invocation.mouseButton === "none"}
              onChange={(e) => save({ mouseAction: e.target.value as InvocationAction })}
            >
              <option value="focusCursor">Summon assistant</option>
              <option value="openMainWindow">Open Wisp window</option>
              <option value="disabled">Do nothing</option>
            </select>
          </div>
        }
      />

      <SettingsRow
        label="Trackpad gesture"
        hint="macOS doesn't let apps listen for trackpad gestures system-wide. Use a shortcut or mouse button instead."
        control={<ChannelBadge state="unsupported" />}
      />

      <SettingsRow
        label="Menu bar"
        hint="Choose “Summon Wisp” from the menu bar icon."
        control={<ChannelBadge state="active" />}
      />
    </SettingsGroup>
  );
}
