import type { PrivacyMode, ThemePreference } from "@wisp/contracts";
import { Button, ErrorNotice, Segmented, SettingsGroup, SettingsRow, Spinner, Switch, cn } from "@wisp/ui";
import { useConnectionStore } from "../../stores/connectionStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { accents } from "./accents";
import { CursorSettingsGroup } from "./CursorSettingsGroup";
import { ComputerSettingsGroup } from "./ComputerSettingsGroup";
import { DataSettingsGroup } from "./DataSettingsGroup";
import { DictationSettingsGroup } from "./DictationSettingsGroup";
import { FocusSettingsGroup } from "./FocusSettingsGroup";
import { InvocationSettingsGroup } from "./InvocationSettingsGroup";

const privacyHints: Record<PrivacyMode, string> = {
  localOnly: "Nothing leaves your Mac. Cloud models are never used — not even as a fallback.",
  localPreferred: "Wisp runs on your Mac whenever it can and asks before each use of a cloud model.",
  cloudAllowed:
    "Cloud models you've configured may be used. Screenshots, audio and files are still never sent silently.",
};

export function SettingsPage() {
  const { settings, status, error, saving, update, load } = useSettingsStore();
  const info = useConnectionStore((s) => s.info);

  if (status === "loading" || status === "idle") {
    return (
      <div className="flex items-center gap-2 pt-16 text-text-muted">
        <Spinner /> Loading settings…
      </div>
    );
  }
  if (!settings) {
    return (
      <ErrorNotice
        title="Settings couldn't be loaded."
        message={error?.message}
        details={error?.details}
        actions={
          <Button size="sm" onClick={() => void load()}>
            Try Again
          </Button>
        }
      />
    );
  }

  const { general, appearance, privacy, invocation, cursor, focus, computer, dictation } = settings;

  return (
    <>
      <h1 className="mb-6 text-xl font-semibold tracking-tight">Settings</h1>

      {error && (
        <div className="mb-4">
          <ErrorNotice title="That change wasn't saved." message={error.message} details={error.details} />
        </div>
      )}

      <SettingsGroup title="General">
        <SettingsRow
          label="Show menu bar icon"
          hint="Quick access to Wisp from the menu bar."
          control={
            <Switch
              label="Show menu bar icon"
              checked={general.showMenuBarIcon}
              disabled={saving === "general"}
              onCheckedChange={(v) =>
                void update({ section: "general", value: { ...general, showMenuBarIcon: v } })
              }
            />
          }
        />
        <SettingsRow
          label="Offer tips"
          hint="After you've spent a while in an app, Wisp offers a quick tour of it — once per app. Uses only which app is in front, never your screen."
          control={
            <Switch
              label="Offer tips"
              checked={general.proactiveTips}
              disabled={saving === "general"}
              onCheckedChange={(v) =>
                void update({ section: "general", value: { ...general, proactiveTips: v } })
              }
            />
          }
        />
      </SettingsGroup>

      <InvocationSettingsGroup invocation={invocation} />

      <DictationSettingsGroup dictation={dictation} invocation={invocation} />

      <CursorSettingsGroup cursor={cursor} />

      <FocusSettingsGroup focus={focus} />

      <ComputerSettingsGroup computer={computer} />

      <DataSettingsGroup />

      <SettingsGroup title="Appearance">
        <SettingsRow
          label="Theme"
          control={
            <Segmented<ThemePreference>
              label="Theme"
              value={appearance.theme}
              disabled={saving === "appearance"}
              onValueChange={(theme) =>
                void update({ section: "appearance", value: { ...appearance, theme } })
              }
              options={[
                { value: "system", label: "System" },
                { value: "light", label: "Light" },
                { value: "dark", label: "Dark" },
              ]}
            />
          }
        />
        <SettingsRow
          label="Accent color"
          hint={accents.find((a) => a.value === appearance.accent)?.label ?? "Custom"}
          control={
            <div role="radiogroup" aria-label="Accent color" className="flex gap-1.5">
              {accents.map((a) => {
                const selected = appearance.accent === a.value;
                return (
                  <button
                    key={a.value}
                    type="button"
                    role="radio"
                    aria-checked={selected}
                    aria-label={a.label}
                    title={a.label}
                    disabled={saving === "appearance"}
                    onClick={() =>
                      void update({ section: "appearance", value: { ...appearance, accent: a.value } })
                    }
                    className={cn(
                      "h-5 w-5 rounded-full ring-offset-2 ring-offset-surface transition-shadow duration-[var(--duration-fast)]",
                      selected ? "ring-2 ring-text-muted" : "hover:ring-2 hover:ring-border-strong",
                    )}
                    style={{ background: a.swatch }}
                  />
                );
              })}
            </div>
          }
        />
      </SettingsGroup>

      <SettingsGroup title="Privacy" description={privacyHints[privacy.mode]}>
        <SettingsRow
          label="Processing"
          control={
            <Segmented<PrivacyMode>
              label="Privacy mode"
              value={privacy.mode}
              disabled={saving === "privacy"}
              onValueChange={(mode) => void update({ section: "privacy", value: { mode } })}
              options={[
                { value: "localOnly", label: "Local Only" },
                { value: "localPreferred", label: "Local Preferred" },
                { value: "cloudAllowed", label: "Cloud Allowed" },
              ]}
            />
          }
        />
      </SettingsGroup>

      <SettingsGroup title="About">
        <SettingsRow
          label="Engine version"
          control={<span className="text-text-muted">{info?.coreVersion ?? "—"}</span>}
        />
        <SettingsRow
          label="Data location"
          control={
            <span className="selectable max-w-[320px] truncate font-mono text-sm text-text-muted">
              {info?.dataDir ?? "—"}
            </span>
          }
        />
      </SettingsGroup>
    </>
  );
}
