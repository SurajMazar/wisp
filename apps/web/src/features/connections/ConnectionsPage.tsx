import type { McpEnvVar, McpRule, McpServerConfig, McpServerStatus, McpToolInfo } from "@wisp/contracts";
import { Button, ErrorNotice, Segmented, SettingsGroup, SettingsRow, Spinner, Switch, cn } from "@wisp/ui";
import { useEffect, useState } from "react";
import { ipc } from "../../services/ipc";
import { useServiceStore } from "../../stores/serviceStore";
import { useSettingsStore } from "../../stores/settingsStore";

type Draft = {
  name: string;
  where: "mac" | "web";
  command: string;
  args: string;
  env: { name: string; value: string; secret: boolean }[];
  localOnly: boolean;
  url: string;
  token: string;
};

const empty: Draft = {
  name: "",
  where: "mac",
  command: "",
  args: "",
  env: [],
  localOnly: false,
  url: "",
  token: "",
};

/** Starting points; the user still supplies their own token. */
const presets: { label: string; draft: Partial<Draft>; note: string }[] = [
  {
    label: "Notion",
    draft: {
      name: "Notion",
      where: "mac",
      command: "npx",
      args: "-y @notionhq/notion-mcp-server",
      env: [{ name: "NOTION_TOKEN", value: "", secret: true }],
    },
    note: "Create an internal integration at notion.so/my-integrations, share your pages with it, and paste its secret.",
  },
  {
    label: "GitHub",
    draft: { name: "GitHub", where: "web", url: "https://api.githubcopilot.com/mcp/" },
    note: "Paste a GitHub personal access token as the token.",
  },
  {
    label: "A folder",
    draft: {
      name: "Folder",
      where: "mac",
      command: "npx",
      args: "-y @modelcontextprotocol/server-filesystem ~/Documents",
      localOnly: true,
    },
    note: "Gives agents read and write access to one folder. Change the path at the end.",
  },
];

/** "a 'b c' d" → ["a", "b c", "d"] */
export function splitArgs(s: string): string[] {
  const out: string[] = [];
  let cur = "";
  let quote: string | null = null;
  let has = false;
  for (const ch of s) {
    if (quote) {
      if (ch === quote) quote = null;
      else cur += ch;
    } else if (ch === '"' || ch === "'") {
      quote = ch;
      has = true;
    } else if (/\s/.test(ch)) {
      if (cur || has) out.push(cur);
      cur = "";
      has = false;
    } else {
      cur += ch;
      has = true;
    }
  }
  if (cur || has) out.push(cur);
  return out;
}

const slug = (name: string, taken: string[]) => {
  const base =
    name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "")
      .slice(0, 24) || "service";
  let id = base;
  for (let i = 2; taken.includes(id); i++) id = `${base}-${i}`;
  return id;
};

const stateLabel: Record<string, { text: string; tone: string }> & { off: { text: string; tone: string } } = {
  ready: { text: "Connected", tone: "text-success" },
  connecting: { text: "Connecting…", tone: "text-text-muted" },
  error: { text: "Couldn't connect", tone: "text-danger" },
  off: { text: "Off", tone: "text-text-faint" },
  blockedByPrivacy: { text: "Off in Local Only", tone: "text-warning" },
};

export function ConnectionsPage() {
  const settings = useSettingsStore((s) => s.settings);
  const update = useSettingsStore((s) => s.update);
  const { status, load, error } = useServiceStore();
  const [adding, setAdding] = useState(false);

  useEffect(() => {
    void load();
  }, [load]);

  if (!settings) {
    return (
      <div className="flex items-center gap-2 pt-16 text-text-muted">
        <Spinner /> Loading…
      </div>
    );
  }
  const servers = settings.connections.servers;
  const save = (next: McpServerConfig[]) => update({ section: "connections", value: { servers: next } });

  return (
    <>
      <h1 className="mb-2 text-xl font-semibold tracking-tight">Connections</h1>
      <p className="mb-6 max-w-2xl text-sm text-text-muted">
        Connect services such as Notion or GitHub through MCP servers, so agents can use them — say “Agent,
        add this to my notes in Notion”. Every action asks first unless you allow that tool. Tokens are kept
        in your Mac's Keychain.
      </p>
      {settings.privacy.mode === "localOnly" && (
        <div className="mb-4">
          <ErrorNotice
            title="Local Only is on"
            message="Only services marked “runs entirely on this Mac” are used. Others stay off until you allow cloud use in Settings → Privacy."
          />
        </div>
      )}
      {error && <p className="mb-3 text-sm text-danger">{error}</p>}

      {servers.map((s) => (
        <ServerCard
          key={s.id}
          server={s}
          status={status[s.id]}
          onChange={(next) => void save(servers.map((x) => (x.id === s.id ? next : x)))}
          onRemove={() => void remove(s, servers, save)}
        />
      ))}

      {adding ? (
        <AddConnection
          taken={servers.map((s) => s.id)}
          onCancel={() => setAdding(false)}
          onAdd={async (s) => {
            await save([...servers, s]);
            setAdding(false);
          }}
        />
      ) : (
        <Button onClick={() => setAdding(true)}>Add Connection…</Button>
      )}
    </>
  );
}

async function remove(
  s: McpServerConfig,
  servers: McpServerConfig[],
  save: (n: McpServerConfig[]) => Promise<void>,
) {
  const accounts = [
    ...(s.transport.kind === "stdio" ? s.transport.config.env.map((e) => e.keychainAccount) : []),
    s.transport.kind === "http" ? s.transport.config.keychainAccount : undefined,
  ].filter((a): a is string => !!a);
  for (const account of accounts)
    await ipc()
      .call("host.keychain.delete", { account })
      .catch(() => {});
  await save(servers.filter((x) => x.id !== s.id));
}

function ServerCard({
  server: s,
  status,
  onChange,
  onRemove,
}: {
  server: McpServerConfig;
  status?: McpServerStatus;
  onChange: (s: McpServerConfig) => void;
  onRemove: () => void;
}) {
  const reconnect = useServiceStore((st) => st.reconnect);
  const state = stateLabel[status?.state ?? (s.enabled ? "connecting" : "off")] ?? stateLabel.off;
  const summary =
    s.transport.kind === "stdio"
      ? [s.transport.config.command, ...s.transport.config.args].join(" ")
      : s.transport.config.url;
  const rule = (tool: string): McpRule => s.tools.find((t) => t.tool === tool)?.rule ?? "ask";
  const setRule = (tool: string, r: McpRule) =>
    onChange({
      ...s,
      tools: [...s.tools.filter((t) => t.tool !== tool), ...(r === "ask" ? [] : [{ tool, rule: r }])],
    });

  return (
    <SettingsGroup title={s.name}>
      <SettingsRow
        label={
          <span className="flex items-center gap-2">
            <span className={cn("text-sm", state.tone)} role="status">
              {state.text}
            </span>
            {status?.serverName && <span className="text-xs text-text-faint">· {status.serverName}</span>}
          </span>
        }
        hint={
          <span className="block max-w-xl truncate font-mono text-xs" title={summary}>
            {summary}
            {s.localOnly ? " · runs on this Mac" : ""}
          </span>
        }
        control={
          <span className="flex items-center gap-1.5">
            <Switch
              label={`${s.name} on`}
              checked={s.enabled}
              onCheckedChange={(enabled) => onChange({ ...s, enabled })}
            />
            <Button size="sm" variant="ghost" disabled={!s.enabled} onClick={() => void reconnect(s.id)}>
              Reconnect
            </Button>
            <Button size="sm" variant="ghost" onClick={onRemove}>
              Remove
            </Button>
          </span>
        }
      />
      {status?.error && <p className="px-3 pb-2 text-xs text-danger">{status.error}</p>}
      {status?.tools.map((t) => (
        <ToolRow key={t.name} tool={t} rule={rule(t.name)} onRule={(r) => setRule(t.name, r)} />
      ))}
    </SettingsGroup>
  );
}

function ToolRow({
  tool: t,
  rule,
  onRule,
}: {
  tool: McpToolInfo;
  rule: McpRule;
  onRule: (r: McpRule) => void;
}) {
  return (
    <SettingsRow
      label={
        <span className="flex items-center gap-2">
          <span>{t.title ?? t.name}</span>
          {t.readOnly && (
            <span className="rounded bg-bg-sunken px-1.5 text-xs text-text-faint">reads only</span>
          )}
          {t.destructive && (
            <span className="rounded bg-danger/10 px-1.5 text-xs text-danger">can delete</span>
          )}
        </span>
      }
      hint={t.description ? <span className="line-clamp-2">{t.description}</span> : undefined}
      control={
        <Segmented<McpRule>
          label={`Rule for ${t.name}`}
          value={rule}
          onValueChange={onRule}
          options={[
            { value: "ask", label: "Ask" },
            { value: "allow", label: "Allow" },
            { value: "block", label: "Block" },
          ]}
        />
      }
    />
  );
}

function AddConnection({
  taken,
  onAdd,
  onCancel,
}: {
  taken: string[];
  onAdd: (s: McpServerConfig) => Promise<void>;
  onCancel: () => void;
}) {
  const [d, setD] = useState<Draft>(empty);
  const [note, setNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const set = (p: Partial<Draft>) => setD((x) => ({ ...x, ...p }));
  const valid =
    d.name.trim() &&
    (d.where === "mac"
      ? d.command.trim()
      : /^https:\/\/|^http:\/\/(127\.0\.0\.1|localhost)/.test(d.url.trim()));
  const input = "h-7 rounded-[var(--radius-sm)] border border-border-strong bg-surface px-2 text-sm";

  const add = async () => {
    setError(null);
    const id = slug(d.name, taken);
    try {
      const env: McpEnvVar[] = [];
      for (const v of d.env.filter((v) => v.name.trim())) {
        if (v.secret && v.value) {
          const account = `mcp:${id}:${v.name.trim()}`;
          await ipc().call("host.keychain.set", { account, secret: v.value });
          env.push({ name: v.name.trim(), keychainAccount: account });
        } else {
          env.push({ name: v.name.trim(), value: v.value });
        }
      }
      let tokenAccount: string | undefined;
      if (d.where === "web" && d.token) {
        tokenAccount = `mcp:${id}:token`;
        await ipc().call("host.keychain.set", { account: tokenAccount, secret: d.token });
      }
      await onAdd({
        id,
        name: d.name.trim(),
        enabled: true,
        localOnly: d.where === "mac" && d.localOnly,
        tools: [],
        transport:
          d.where === "mac"
            ? { kind: "stdio", config: { command: d.command.trim(), args: splitArgs(d.args), env } }
            : {
                kind: "http",
                config: { url: d.url.trim(), ...(tokenAccount && { keychainAccount: tokenAccount }) },
              },
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : "Couldn't add it.");
    }
  };

  return (
    <SettingsGroup title="New connection">
      <div className="space-y-2.5 px-3 py-3">
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-sm text-text-muted">Start from:</span>
          {presets.map((p) => (
            <Button
              key={p.label}
              size="sm"
              variant="ghost"
              onClick={() => {
                setD({ ...empty, ...p.draft });
                setNote(p.note);
              }}
            >
              {p.label}
            </Button>
          ))}
        </div>
        {note && <p className="text-xs text-text-muted">{note}</p>}
        <label className="grid grid-cols-[120px_1fr] items-center gap-2">
          <span className="text-sm text-text-muted">Name</span>
          <input
            className={input}
            value={d.name}
            onChange={(e) => set({ name: e.target.value })}
            aria-label="Name"
          />
        </label>
        <div className="grid grid-cols-[120px_1fr] items-center gap-2">
          <span className="text-sm text-text-muted">Where it runs</span>
          <Segmented<Draft["where"]>
            label="Where the server runs"
            value={d.where}
            onValueChange={(where) => set({ where })}
            options={[
              { value: "mac", label: "A program on this Mac" },
              { value: "web", label: "A server on the web" },
            ]}
          />
        </div>
        {d.where === "mac" ? (
          <>
            <label className="grid grid-cols-[120px_1fr] items-center gap-2">
              <span className="text-sm text-text-muted">Command</span>
              <input
                className={cn(input, "font-mono")}
                value={d.command}
                placeholder="npx"
                aria-label="Command"
                onChange={(e) => set({ command: e.target.value })}
              />
            </label>
            <label className="grid grid-cols-[120px_1fr] items-center gap-2">
              <span className="text-sm text-text-muted">Arguments</span>
              <input
                className={cn(input, "font-mono")}
                value={d.args}
                aria-label="Arguments"
                onChange={(e) => set({ args: e.target.value })}
              />
            </label>
            {d.env.map((v, i) => (
              <div key={i} className="grid grid-cols-[120px_1fr_1fr_auto] items-center gap-2">
                <span className="text-sm text-text-muted">{i === 0 ? "Environment" : ""}</span>
                <input
                  className={cn(input, "font-mono")}
                  value={v.name}
                  placeholder="NAME"
                  aria-label={`Variable ${i + 1} name`}
                  onChange={(e) =>
                    set({ env: d.env.map((x, j) => (j === i ? { ...x, name: e.target.value } : x)) })
                  }
                />
                <input
                  className={input}
                  type={v.secret ? "password" : "text"}
                  autoComplete="off"
                  value={v.value}
                  placeholder={v.secret ? "Secret (kept in Keychain)" : "Value"}
                  aria-label={`${v.name || `Variable ${i + 1}`} value`}
                  onChange={(e) =>
                    set({ env: d.env.map((x, j) => (j === i ? { ...x, value: e.target.value } : x)) })
                  }
                />
                <label className="flex items-center gap-1 text-xs text-text-muted">
                  <input
                    type="checkbox"
                    checked={v.secret}
                    onChange={(e) =>
                      set({ env: d.env.map((x, j) => (j === i ? { ...x, secret: e.target.checked } : x)) })
                    }
                  />
                  Secret
                </label>
              </div>
            ))}
            <div className="grid grid-cols-[120px_1fr] gap-2">
              <span />
              <div className="flex flex-wrap items-center gap-3">
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => set({ env: [...d.env, { name: "", value: "", secret: true }] })}
                >
                  Add Variable
                </Button>
                <label className="flex items-center gap-1.5 text-sm text-text-muted">
                  <input
                    type="checkbox"
                    checked={d.localOnly}
                    onChange={(e) => set({ localOnly: e.target.checked })}
                  />
                  Runs entirely on this Mac (sends nothing elsewhere)
                </label>
              </div>
            </div>
          </>
        ) : (
          <>
            <label className="grid grid-cols-[120px_1fr] items-center gap-2">
              <span className="text-sm text-text-muted">Address</span>
              <input
                className={cn(input, "font-mono")}
                value={d.url}
                placeholder="https://…/mcp"
                aria-label="Address"
                onChange={(e) => set({ url: e.target.value })}
              />
            </label>
            <label className="grid grid-cols-[120px_1fr] items-center gap-2">
              <span className="text-sm text-text-muted">Token</span>
              <input
                className={input}
                type="password"
                autoComplete="off"
                value={d.token}
                placeholder="Optional · kept in Keychain"
                aria-label="Token"
                onChange={(e) => set({ token: e.target.value })}
              />
            </label>
          </>
        )}
        <div className="flex items-center gap-2 pt-1">
          <Button variant="primary" size="sm" disabled={!valid} onClick={() => void add()}>
            Add
          </Button>
          <Button size="sm" variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          {error && <span className="text-xs text-danger">{error}</span>}
        </div>
      </div>
    </SettingsGroup>
  );
}
