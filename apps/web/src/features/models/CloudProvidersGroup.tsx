import type { CloudPreset, EndpointConfig, ModelSettings } from "@wisp/contracts";
import { Button, SettingsGroup, SettingsRow } from "@wisp/ui";
import { useState } from "react";
import { ipc } from "../../services/ipc";
import { useSettingsStore } from "../../stores/settingsStore";

const providers: { preset: CloudPreset; name: string; baseUrl: string; keyHint: string; keysUrl: string }[] =
  [
    {
      preset: "openai",
      name: "OpenAI",
      baseUrl: "https://api.openai.com/v1",
      keyHint: "sk-…",
      keysUrl: "https://platform.openai.com/api-keys",
    },
    {
      preset: "anthropic",
      name: "Anthropic",
      baseUrl: "https://api.anthropic.com/v1",
      keyHint: "sk-ant-…",
      keysUrl: "https://console.anthropic.com/settings/keys",
    },
    {
      preset: "gemini",
      name: "Google Gemini",
      baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai",
      keyHint: "AIza…",
      keysUrl: "https://aistudio.google.com/apikey",
    },
  ];

/** OpenAI, Anthropic and Gemini with your own API key (stored in the Keychain, never in settings). */
export function CloudProvidersGroup({
  settings,
  onSave,
}: {
  settings: ModelSettings;
  onSave: (p: Partial<ModelSettings>) => void;
}) {
  const privacy = useSettingsStore((s) => s.settings?.privacy);
  const update = useSettingsStore((s) => s.update);
  const [editing, setEditing] = useState<CloudPreset | null>(null);
  const [key, setKey] = useState("");
  const [error, setError] = useState<string | null>(null);
  const connected = (p: CloudPreset) => settings.endpoints.find((e) => e.preset === p);
  const anyConnected = providers.some((p) => connected(p.preset));

  const save = async (p: (typeof providers)[number]) => {
    setError(null);
    const account = `endpoint:${p.preset}`;
    try {
      await ipc().call("host.keychain.set", { account, secret: key.trim() });
      const e: EndpointConfig = {
        id: p.preset,
        name: p.name,
        baseUrl: p.baseUrl,
        keychainAccount: account,
        preset: p.preset,
      };
      onSave({ endpoints: [...settings.endpoints.filter((x) => x.id !== p.preset), e] });
      setEditing(null);
      setKey("");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Couldn't save the key.");
    }
  };

  const remove = async (e: EndpointConfig) => {
    if (e.keychainAccount)
      await ipc()
        .call("host.keychain.delete", { account: e.keychainAccount })
        .catch(() => {});
    onSave({ endpoints: settings.endpoints.filter((x) => x.id !== e.id) });
  };

  return (
    <SettingsGroup
      title="Cloud models"
      description="Use OpenAI, Anthropic or Gemini models with your own API key. Keys are stored in your Mac's Keychain. Questions sent to these models (and any screen area you attach) leave this Mac."
    >
      {anyConnected && privacy?.mode === "localOnly" && (
        <div
          role="status"
          className="mx-3 my-2 rounded-[var(--radius-md)] border border-warning/30 bg-warning/10 p-2.5 text-sm"
        >
          <p className="text-text">Your privacy setting is Local only, so cloud models won't be used.</p>
          <Button
            size="sm"
            variant="primary"
            className="mt-2"
            onClick={() => void update({ section: "privacy", value: { mode: "cloudAllowed" } })}
          >
            Allow cloud models
          </Button>
        </div>
      )}
      {providers.map((p) => {
        const c = connected(p.preset);
        if (editing === p.preset) {
          return (
            <form
              key={p.preset}
              className="flex flex-wrap items-center gap-2 px-3 py-2.5"
              onSubmit={(e) => {
                e.preventDefault();
                if (key.trim()) void save(p);
              }}
            >
              <label htmlFor={`key-${p.preset}`} className="w-28 text-sm text-text">
                {p.name}
              </label>
              <input
                id={`key-${p.preset}`}
                type="password"
                autoComplete="off"
                autoFocus
                className="h-7 min-w-56 flex-1 rounded-[var(--radius-sm)] border border-border-strong bg-surface px-2 font-mono text-sm"
                placeholder={`API key (${p.keyHint})`}
                value={key}
                onChange={(e) => setKey(e.target.value)}
              />
              <Button type="submit" size="sm" variant="primary" disabled={!key.trim()}>
                Save
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  setEditing(null);
                  setKey("");
                }}
              >
                Cancel
              </Button>
              <a
                href={p.keysUrl}
                target="_blank"
                rel="noreferrer"
                className="w-full pl-[7.5rem] text-xs text-accent"
              >
                Get a key from {p.name}
              </a>
              {error && <span className="w-full pl-[7.5rem] text-xs text-danger">{error}</span>}
            </form>
          );
        }
        return (
          <SettingsRow
            key={p.preset}
            label={p.name}
            hint={c ? "Connected · key in Keychain" : "Not connected"}
            control={
              <span className="flex gap-1">
                <Button size="sm" onClick={() => setEditing(p.preset)}>
                  {c ? "Change Key" : "Add Key"}
                </Button>
                {c && (
                  <Button size="sm" variant="ghost" onClick={() => void remove(c)}>
                    Remove
                  </Button>
                )}
              </span>
            }
          />
        );
      })}
    </SettingsGroup>
  );
}
