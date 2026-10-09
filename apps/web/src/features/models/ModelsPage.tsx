import type {
  ModelInfo,
  ModelRef,
  ModelSettings,
  ProviderInfo,
  ProviderState,
  RouteInfo,
} from "@wisp/contracts";
import { WispError } from "@wisp/contracts";
import { Button, ErrorNotice, SettingsGroup, SettingsRow, Slider, Spinner, Switch, cn } from "@wisp/ui";
import { useEffect, useState } from "react";
import { modelKey, useModelsStore } from "../../stores/modelsStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { EndpointsGroup } from "./EndpointsGroup";

const stateBadge: Record<ProviderState, { text: string; tone: string }> = {
  available: { text: "Available", tone: "bg-success" },
  notRunning: { text: "Not running", tone: "bg-warning" },
  notInstalled: { text: "Not installed", tone: "bg-text-faint" },
  unavailable: { text: "Unavailable", tone: "bg-warning" },
};

const suggestions = [
  { name: "qwen3-vl:8b-instruct", note: "Sees your screen · 6.1 GB · recommended" },
  { name: "qwen3-vl:4b-instruct", note: "Sees your screen · 3.3 GB · faster" },
  { name: "gemma3:4b", note: "Sees images · 3.3 GB" },
];

const gb = (b?: number | null) => (b ? `${(b / 1e9).toFixed(1)} GB` : "");

function Badge({ state }: { state: ProviderState }) {
  const b = stateBadge[state];
  return (
    <span className="inline-flex items-center gap-1.5 text-sm text-text-muted">
      <span aria-hidden className={cn("h-1.5 w-1.5 rounded-full", b.tone)} />
      {b.text}
    </span>
  );
}

function Chips({ m }: { m: ModelInfo }) {
  const chips = [
    m.capabilities.includes("vision") ? "Sees images" : "Text",
    m.capabilities.includes("toolCalling") ? "Tools" : null,
    m.capabilities.includes("reasoning") ? "Thinks before answering" : null,
    m.contextLength ? `${Math.round(m.contextLength / 1024)}K context` : null,
    m.parameterSize,
    gb(m.sizeBytes),
    m.locality === "local" ? "On this Mac" : "Cloud",
  ].filter(Boolean);
  return (
    <span className="flex flex-wrap gap-1">
      {chips.map((c) => (
        <span
          key={c}
          className="rounded-[var(--radius-xs)] bg-bg-sunken px-1.5 py-px text-xs text-text-muted"
        >
          {c}
        </span>
      ))}
    </span>
  );
}

function RouteLine({ label, route }: { label: string; route: RouteInfo | WispError | null }) {
  return (
    <SettingsRow
      label={label}
      hint={route instanceof WispError ? route.message : route ? route.reason : "Checking…"}
      control={
        route && !(route instanceof WispError) ? (
          <span className="text-sm text-text">
            {route.modelName}{" "}
            <span className="text-text-faint">· {route.locality === "local" ? "on this Mac" : "cloud"}</span>
          </span>
        ) : route ? (
          <span className="text-sm text-warning">None ready</span>
        ) : (
          <Spinner />
        )
      }
    />
  );
}

function ProviderRow({ p, onStartOllama }: { p: ProviderInfo; onStartOllama: () => void }) {
  return (
    <SettingsRow
      label={
        <span className="flex items-center gap-3">
          {p.name} <Badge state={p.state} />
          {p.version && <span className="text-xs text-text-faint">v{p.version}</span>}
        </span>
      }
      hint={p.detail ?? (p.locality === "local" ? "Runs on this Mac" : "Cloud")}
      control={
        p.kind === "ollama" && p.state === "notRunning" ? (
          <Button size="sm" variant="primary" onClick={onStartOllama}>
            Start Ollama
          </Button>
        ) : p.kind === "ollama" && p.state === "notInstalled" ? (
          <Button size="sm" asChild>
            <a href="https://ollama.com/download" target="_blank" rel="noreferrer">
              Get Ollama
            </a>
          </Button>
        ) : null
      }
    />
  );
}

export function ModelsPage() {
  const { list, loading, error, pulls, tests, routes, load, pull, cancelPull, remove, test, startOllama } =
    useModelsStore();
  const settings = useSettingsStore((s) => s.settings?.models);
  const update = useSettingsStore((s) => s.update);
  const [name, setName] = useState("");
  const [draft, setDraft] = useState<ModelSettings | null>(settings ?? null);
  const [synced, setSynced] = useState(settings);
  if (synced !== settings) {
    setSynced(settings);
    setDraft(settings ?? null);
  }

  useEffect(() => {
    void load();
  }, [load]);

  const save = (patch: Partial<ModelSettings>) => {
    if (!draft) return;
    const next = { ...draft, ...patch };
    setDraft(next);
    void update({ section: "models", value: next }).then(() => useModelsStore.getState().refreshRoutes());
  };

  const ollama = list?.providers.find((p) => p.kind === "ollama");
  const models = list?.models ?? [];
  const activePulls = Object.values(pulls).filter((p) => !p.done || p.error || p.status === "cancelled");
  const isPref = (m: ModelInfo, slot: "general" | "vision") =>
    draft?.[slot]?.providerId === m.providerId && draft?.[slot]?.model === m.id;
  const choose = (slot: "general" | "vision", m: ModelInfo | null) =>
    save({ [slot]: m ? ({ providerId: m.providerId, model: m.id } satisfies ModelRef) : null });

  return (
    <>
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-xl font-semibold tracking-tight">AI Models</h1>
        <Button size="sm" onClick={() => void load()} disabled={loading}>
          {loading ? "Checking…" : "Refresh"}
        </Button>
      </div>

      {error && (
        <div className="mb-4">
          <ErrorNotice title="Couldn't check your models." message={error.message} details={error.details} />
        </div>
      )}

      <SettingsGroup
        title="Who answers"
        description="Wisp picks a model for each question. Local Only never uses a cloud model."
      >
        <RouteLine label="Questions" route={routes.text} />
        <RouteLine label="Questions about your screen" route={routes.vision} />
      </SettingsGroup>

      <SettingsGroup title="Providers">
        {list ? (
          list.providers.map((p) => <ProviderRow key={p.id} p={p} onStartOllama={() => void startOllama()} />)
        ) : (
          <SettingsRow label="Checking…" control={<Spinner />} />
        )}
      </SettingsGroup>

      <SettingsGroup
        title="Models on this Mac"
        description={
          models.length === 0 && ollama?.state === "available"
            ? "No models yet — download one below."
            : undefined
        }
      >
        {models.map((m) => {
          const t = tests[modelKey({ providerId: m.providerId, model: m.id })];
          return (
            <div key={`${m.providerId}/${m.id}`} className="space-y-1.5 px-3 py-2.5">
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <p className="truncate text-base text-text">{m.name}</p>
                  <Chips m={m} />
                </div>
                <div className="flex shrink-0 gap-1.5">
                  <Button
                    size="sm"
                    variant={isPref(m, "general") ? "primary" : "secondary"}
                    onClick={() => choose("general", isPref(m, "general") ? null : m)}
                  >
                    {isPref(m, "general") ? "Text ✓" : "Use for text"}
                  </Button>
                  {m.capabilities.includes("vision") && (
                    <Button
                      size="sm"
                      variant={isPref(m, "vision") ? "primary" : "secondary"}
                      onClick={() => choose("vision", isPref(m, "vision") ? null : m)}
                    >
                      {isPref(m, "vision") ? "Screen ✓" : "Use for screen"}
                    </Button>
                  )}
                  <Button
                    size="sm"
                    disabled={t === "running"}
                    onClick={() => void test({ providerId: m.providerId, model: m.id })}
                  >
                    {t === "running" ? "Testing…" : "Test"}
                  </Button>
                  {m.providerId === "ollama" && (
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => void remove(m.id)}
                      aria-label={`Delete ${m.name}`}
                    >
                      Delete
                    </Button>
                  )}
                </div>
              </div>
              {t && t !== "running" && (
                <p className={cn("text-xs", "error" in t ? "text-danger" : "text-text-muted")} role="status">
                  {"error" in t
                    ? t.error
                    : `“${t.text}” · first word in ${t.firstTokenMs} ms · ${(t.totalMs / 1000).toFixed(1)} s total`}
                </p>
              )}
            </div>
          );
        })}
      </SettingsGroup>

      <SettingsGroup
        title="Download a model"
        description="Downloads come from Ollama's library and stay on this Mac. Pausing keeps what's downloaded so far."
      >
        <form
          className="flex gap-2 px-3 py-2.5"
          onSubmit={(e) => {
            e.preventDefault();
            if (name.trim()) void pull(name.trim());
            setName("");
          }}
        >
          <input
            aria-label="Model name"
            placeholder="e.g. qwen3-vl:8b"
            className="h-7 flex-1 rounded-[var(--radius-sm)] border border-border-strong bg-surface px-2 text-base"
            value={name}
            onChange={(e) => setName(e.target.value)}
            disabled={ollama?.state !== "available"}
          />
          <Button type="submit" variant="primary" disabled={!name.trim() || ollama?.state !== "available"}>
            Download
          </Button>
        </form>
        <div className="flex flex-wrap gap-1.5 px-3 pb-2.5">
          {suggestions
            .filter((s) => !models.some((m) => m.id === s.name))
            .map((s) => (
              <button
                key={s.name}
                type="button"
                className="rounded-[var(--radius-sm)] border border-border px-2 py-1 text-left text-sm hover:bg-surface-hover disabled:opacity-45"
                disabled={ollama?.state !== "available" || pulls[s.name]?.done === false}
                onClick={() => void pull(s.name)}
              >
                <span className="font-medium">{s.name}</span>{" "}
                <span className="text-text-faint">{s.note}</span>
              </button>
            ))}
        </div>
        {activePulls.map((p) => {
          const pct = p.total ? Math.min(100, Math.round(((p.completed ?? 0) / p.total) * 100)) : null;
          return (
            <div key={p.name} className="space-y-1 px-3 py-2.5" role="status">
              <div className="flex items-center justify-between text-sm">
                <span>
                  {p.error ? "Couldn't download" : p.status === "cancelled" ? "Paused" : "Downloading"}{" "}
                  <b>{p.name}</b>
                </span>
                {p.error || p.status === "cancelled" ? (
                  <Button size="sm" onClick={() => void pull(p.name)}>
                    {p.error ? "Retry" : "Resume"}
                  </Button>
                ) : (
                  <Button size="sm" variant="ghost" onClick={() => void cancelPull(p.name)}>
                    Pause
                  </Button>
                )}
              </div>
              {p.error ? (
                <p className="text-xs text-danger">{p.error}</p>
              ) : (
                <>
                  <div className="h-1.5 overflow-hidden rounded-full bg-bg-sunken">
                    <div className="h-full bg-accent transition-[width]" style={{ width: `${pct ?? 0}%` }} />
                  </div>
                  <p className="text-xs text-text-faint">
                    {pct !== null ? `${gb(p.completed)} / ${gb(p.total)} · ${pct}%` : p.status}
                  </p>
                </>
              )}
            </div>
          );
        })}
      </SettingsGroup>

      {draft && (
        <SettingsGroup title="Generation">
          <SettingsRow
            label="Deep thinking"
            hint="Let the model reason step by step before answering. Slower, sometimes better for hard questions. When off, a model that always thinks is swapped for its installed “instruct” version."
            control={
              <Switch
                label="Deep thinking"
                checked={draft.deepThinking}
                onCheckedChange={(deepThinking) => save({ deepThinking })}
              />
            }
          />
          <SettingsRow
            label="Creativity"
            hint={`Temperature ${draft.temperature.toFixed(1)} — lower is more precise`}
            control={
              <Slider
                label="Temperature"
                value={draft.temperature}
                min={0}
                max={1.5}
                step={0.1}
                onValueChange={(temperature) => setDraft({ ...draft, temperature })}
                onValueCommit={(temperature) => save({ temperature })}
              />
            }
          />
          <SettingsRow
            label="Longest answer"
            hint={`${draft.maxTokens} tokens`}
            control={
              <Slider
                label="Max tokens"
                value={draft.maxTokens}
                min={128}
                max={4096}
                step={128}
                onValueChange={(maxTokens) => setDraft({ ...draft, maxTokens })}
                onValueCommit={(maxTokens) => save({ maxTokens })}
              />
            }
          />
          <SettingsRow
            label="Context window"
            hint={`${Math.round(draft.contextLength / 1024)}K tokens — larger remembers more but uses more memory`}
            control={
              <Slider
                label="Context length"
                value={draft.contextLength}
                min={2048}
                max={32768}
                step={2048}
                onValueChange={(contextLength) => setDraft({ ...draft, contextLength })}
                onValueCommit={(contextLength) => save({ contextLength })}
              />
            }
          />
        </SettingsGroup>
      )}

      {draft && <EndpointsGroup settings={draft} onSave={save} />}
    </>
  );
}
