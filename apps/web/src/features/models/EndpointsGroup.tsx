import type { EndpointConfig, ModelSettings } from "@wisp/contracts";
import { Button, SettingsGroup, SettingsRow } from "@wisp/ui";
import { useState } from "react";
import { ipc } from "../../services/ipc";
import type { SettingsPatch } from "../../stores/settingsStore";

const isLocal = (url: string) => /^http:\/\/(127\.0\.0\.1|localhost|\[::1\])(:|\/|$)/.test(url);

/** OpenAI-compatible endpoints (LM Studio, llama.cpp server, or a cloud API). Keys go to the Keychain. */
export function EndpointsGroup({
  settings,
  onSave,
}: {
  settings: ModelSettings;
  onSave: (change: SettingsPatch<"models">) => void;
}) {
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState("");
  const [url, setUrl] = useState("http://127.0.0.1:1234/v1");
  const [key, setKey] = useState("");
  const [error, setError] = useState<string | null>(null);

  const valid = name.trim() && (url.startsWith("https://") || isLocal(url));

  const add = async () => {
    setError(null);
    const id =
      name
        .trim()
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-|-$/g, "")
        .slice(0, 40) || "endpoint";
    const account = key ? `endpoint:${id}` : undefined;
    try {
      if (account) await ipc().call("host.keychain.set", { account, secret: key });
      const e: EndpointConfig = {
        id,
        name: name.trim(),
        baseUrl: url.trim(),
        ...(account && { keychainAccount: account }),
      };
      onSave((m) => ({ endpoints: [...m.endpoints.filter((x) => x.id !== id), e] }));
      setAdding(false);
      setName("");
      setKey("");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Couldn't save the endpoint.");
    }
  };

  const removeEndpoint = async (e: EndpointConfig) => {
    if (e.keychainAccount)
      await ipc()
        .call("host.keychain.delete", { account: e.keychainAccount })
        .catch(() => {});
    onSave((m) => ({ endpoints: m.endpoints.filter((x) => x.id !== e.id) }));
  };

  return (
    <SettingsGroup
      title="Other model servers"
      description="Connect LM Studio, a llama.cpp server or any OpenAI-compatible API. Keys are stored in your Mac's Keychain. Cloud servers are only used when your privacy setting allows."
    >
      {settings.endpoints
        .filter((e) => !e.preset)
        .map((e) => (
          <SettingsRow
            key={e.id}
            label={e.name}
            hint={`${e.baseUrl} · ${isLocal(e.baseUrl) ? "on this Mac" : "cloud"}${e.keychainAccount ? " · key in Keychain" : ""}`}
            control={
              <Button size="sm" variant="ghost" onClick={() => void removeEndpoint(e)}>
                Remove
              </Button>
            }
          />
        ))}
      {adding ? (
        <form
          className="grid grid-cols-[110px_1fr] items-center gap-2 px-3 py-2.5"
          onSubmit={(e) => {
            e.preventDefault();
            if (valid) void add();
          }}
        >
          <label htmlFor="ep-name" className="text-sm text-text-muted">
            Name
          </label>
          <input
            id="ep-name"
            className="h-7 rounded-[var(--radius-sm)] border border-border-strong bg-surface px-2"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="LM Studio"
          />
          <label htmlFor="ep-url" className="text-sm text-text-muted">
            Address
          </label>
          <input
            id="ep-url"
            className="h-7 rounded-[var(--radius-sm)] border border-border-strong bg-surface px-2 font-mono text-sm"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
          />
          <label htmlFor="ep-key" className="text-sm text-text-muted">
            API key
          </label>
          <input
            id="ep-key"
            type="password"
            autoComplete="off"
            className="h-7 rounded-[var(--radius-sm)] border border-border-strong bg-surface px-2"
            value={key}
            onChange={(e) => setKey(e.target.value)}
            placeholder="Optional"
          />
          <span />
          <div className="flex items-center gap-2">
            <Button type="submit" variant="primary" size="sm" disabled={!valid}>
              Add
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setAdding(false)}>
              Cancel
            </Button>
            {url && !(url.startsWith("https://") || isLocal(url)) && (
              <span className="text-xs text-warning">Use https:// or a local http:// address.</span>
            )}
            {error && <span className="text-xs text-danger">{error}</span>}
          </div>
        </form>
      ) : (
        <div className="px-3 py-2.5">
          <Button size="sm" onClick={() => setAdding(true)}>
            Add Server…
          </Button>
        </div>
      )}
    </SettingsGroup>
  );
}
