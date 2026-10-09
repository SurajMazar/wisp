import type { AgentDefinition, AgentSaveParams, ToolDefinition } from "@wisp/contracts";
import { BotIcon, DeleteIcon, EditIcon, PlayIcon } from "@wisp/icons";
import { Button, EmptyState, ErrorNotice } from "@wisp/ui";
import { useEffect, useState } from "react";
import { AgentRunCard } from "../../components/agents/AgentRunCard";
import { RiskBadge } from "../../components/tools/RiskBadge";
import { ipc } from "../../services/ipc";
import { useAgentStore } from "../../stores/agentStore";
import { useModelsStore } from "../../stores/modelsStore";

const templates: AgentSaveParams[] = [
  {
    name: "Desktop Helper",
    avatar: "🧭",
    description: "Opens apps, clicks through menus and fills in forms for you.",
    instructions: "Work carefully. Prefer menus and buttons you can see over keyboard shortcuts.",
    tools: [
      "readScreen",
      "findElement",
      "click",
      "doubleClick",
      "scroll",
      "typeText",
      "keypress",
      "hotkey",
      "openApplication",
      "switchApplication",
    ],
    skills: [],
    limits: { maxSteps: 15, maxMinutes: 10, maxToolCalls: 25, maxTokens: 60000 },
    enabled: true,
  },
  {
    name: "Note Taker",
    avatar: "📝",
    description: "Writes things down in Notes.",
    instructions: "Use the Notes app. Create a new note for each request unless told otherwise.",
    tools: ["openApplication", "switchApplication", "click", "typeText", "hotkey", "keypress"],
    skills: [],
    limits: { maxSteps: 12, maxMinutes: 5, maxToolCalls: 15, maxTokens: 40000 },
    enabled: true,
  },
  {
    name: "Screen Reader",
    avatar: "🔎",
    description: "Finds and reads things on screen without changing anything.",
    instructions: "Never change anything. Read, find and report.",
    tools: ["readScreen", "findElement", "scroll", "copy"],
    skills: [],
    limits: { maxSteps: 10, maxMinutes: 5, maxToolCalls: 10, maxTokens: 40000 },
    enabled: true,
  },
];

const blank: AgentSaveParams = {
  name: "",
  avatar: "🤖",
  description: "",
  instructions: "",
  tools: ["readScreen", "findElement"],
  skills: [],
  limits: { maxSteps: 15, maxMinutes: 10, maxToolCalls: 25, maxTokens: 60000 },
  enabled: true,
};

function toParams(a: AgentDefinition): AgentSaveParams {
  return {
    id: a.id,
    name: a.name,
    avatar: a.avatar,
    description: a.description,
    instructions: a.instructions,
    tools: a.tools,
    skills: a.skills ?? [],
    limits: a.limits,
    enabled: a.enabled,
    ...(a.model ? { model: a.model } : {}),
  };
}

const field = "w-full rounded-[var(--radius-sm)] border border-border-strong bg-bg px-2 py-1.5 text-base";

function Editor({ initial, onDone }: { initial: AgentSaveParams; onDone: () => void }) {
  const { save, error } = useAgentStore();
  const models = useModelsStore((s) => s.list?.models ?? []);
  const [draft, setDraft] = useState(initial);
  const [tools, setTools] = useState<ToolDefinition[]>([]);
  useEffect(() => {
    void ipc()
      .call("tool.list")
      .then((r) => setTools(r.tools))
      .catch(() => {});
  }, []);
  const set = (patch: Partial<AgentSaveParams>) => setDraft((d) => ({ ...d, ...patch }));
  const modelKey = draft.model ? `${draft.model.providerId}/${draft.model.model}` : "";

  return (
    <form
      aria-label={draft.id ? "Edit agent" : "New agent"}
      className="space-y-3 rounded-[var(--radius-lg)] border border-border bg-surface p-4"
      onSubmit={(e) => {
        e.preventDefault();
        void save(draft).then((a) => a && onDone());
      }}
    >
      <div className="flex gap-2">
        <label className="w-16">
          <span className="text-xs text-text-muted">Avatar</span>
          <input
            aria-label="Avatar"
            className={field}
            value={draft.avatar}
            maxLength={4}
            onChange={(e) => set({ avatar: e.target.value })}
          />
        </label>
        <label className="flex-1">
          <span className="text-xs text-text-muted">Name</span>
          <input
            aria-label="Name"
            required
            className={field}
            value={draft.name}
            onChange={(e) => set({ name: e.target.value })}
          />
        </label>
      </div>
      <label className="block">
        <span className="text-xs text-text-muted">Description</span>
        <input
          aria-label="Description"
          className={field}
          value={draft.description}
          onChange={(e) => set({ description: e.target.value })}
        />
      </label>
      <label className="block">
        <span className="text-xs text-text-muted">Instructions</span>
        <textarea
          aria-label="Instructions"
          rows={3}
          className={field}
          value={draft.instructions}
          onChange={(e) => set({ instructions: e.target.value })}
        />
      </label>
      <label className="block">
        <span className="text-xs text-text-muted">Model</span>
        <select
          aria-label="Model"
          className={field}
          value={modelKey}
          onChange={(e) => {
            const m = models.find((x) => `${x.providerId}/${x.id}` === e.target.value);
            setDraft(({ model: _, ...rest }) =>
              m ? { ...rest, model: { providerId: m.providerId, model: m.id } } : rest,
            );
          }}
        >
          <option value="">Auto (best local model)</option>
          {models.map((m) => (
            <option key={`${m.providerId}/${m.id}`} value={`${m.providerId}/${m.id}`}>
              {m.name}
            </option>
          ))}
        </select>
      </label>
      <fieldset>
        <legend className="text-xs text-text-muted">
          Tools this agent may use (each still follows your Computer control settings)
        </legend>
        <div className="mt-1 grid grid-cols-2 gap-x-3 gap-y-1">
          {tools.map((t) => (
            <label key={t.name} className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={draft.tools.includes(t.name)}
                onChange={(e) =>
                  set({
                    tools: e.target.checked
                      ? [...draft.tools, t.name]
                      : draft.tools.filter((x) => x !== t.name),
                  })
                }
              />
              <span className="flex-1">{t.title}</span>
              <RiskBadge risk={t.risk} />
            </label>
          ))}
        </div>
      </fieldset>
      <fieldset className="grid grid-cols-4 gap-2">
        <legend className="mb-1 text-xs text-text-muted">Limits</legend>
        {(
          [
            ["maxSteps", "Steps"],
            ["maxMinutes", "Minutes"],
            ["maxToolCalls", "Actions"],
            ["maxTokens", "Tokens"],
          ] as const
        ).map(([k, label]) => (
          <label key={k} className="text-xs text-text-muted">
            {label}
            <input
              aria-label={label}
              type="number"
              className={field}
              value={draft.limits[k]}
              onChange={(e) => set({ limits: { ...draft.limits, [k]: Number(e.target.value) } })}
            />
          </label>
        ))}
      </fieldset>
      {error && <ErrorNotice title="That agent couldn't be saved." message={error} />}
      <div className="flex gap-2">
        <Button type="submit" variant="primary">
          Save agent
        </Button>
        <Button variant="ghost" onClick={onDone}>
          Cancel
        </Button>
      </div>
    </form>
  );
}

function AgentRow({ a, onEdit }: { a: AgentDefinition; onEdit: () => void }) {
  const { run, remove } = useAgentStore();
  const [goal, setGoal] = useState("");
  const [asking, setAsking] = useState(false);
  return (
    <li className="px-3 py-3">
      <div className="flex items-center gap-3">
        <span
          aria-hidden
          className="flex h-9 w-9 items-center justify-center rounded-full bg-surface-hover text-lg"
        >
          {a.avatar}
        </span>
        <div className="min-w-0 flex-1">
          <p className="truncate text-base font-medium">{a.name}</p>
          <p className="truncate text-sm text-text-muted">
            {a.description || "No description"} · {a.model?.model ?? "Auto"} · {a.tools.length} tools
            {a.lastRunAtMs ? ` · last run ${new Date(a.lastRunAtMs).toLocaleDateString()}` : ""}
          </p>
        </div>
        <Button size="sm" variant="primary" onClick={() => setAsking((v) => !v)}>
          <PlayIcon size={13} /> Run
        </Button>
        <Button size="sm" variant="ghost" aria-label={`Edit ${a.name}`} onClick={onEdit}>
          <EditIcon size={14} />
        </Button>
        <Button size="sm" variant="ghost" aria-label={`Delete ${a.name}`} onClick={() => void remove(a.id)}>
          <DeleteIcon size={14} />
        </Button>
      </div>
      {asking && (
        <form
          className="mt-2 flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            if (!goal.trim()) return;
            void run(goal.trim(), a.id);
            setGoal("");
            setAsking(false);
          }}
        >
          <input
            aria-label={`Goal for ${a.name}`}
            autoFocus
            className={field}
            placeholder="What should it do?"
            value={goal}
            onChange={(e) => setGoal(e.target.value)}
          />
          <Button type="submit" variant="primary">
            Start
          </Button>
        </form>
      )}
    </li>
  );
}

/** Agents (spec §65, §74): saved assistants that carry out tasks on this Mac. */
export function AgentsPage() {
  const { agents, runs, load, loadRuns, save } = useAgentStore();
  const loadModels = useModelsStore((s) => s.load);
  const [editing, setEditing] = useState<AgentSaveParams | null>(null);
  useEffect(() => {
    void load().catch(() => {});
    void loadRuns().catch(() => {});
    void loadModels().catch(() => {});
  }, [load, loadRuns, loadModels]);
  const recent = Object.values(runs)
    .sort((a, b) => b.startedAtMs - a.startedAtMs)
    .slice(0, 8);

  return (
    <>
      <div className="mb-6 flex items-center gap-3">
        <div className="flex-1">
          <h1 className="text-xl font-semibold tracking-tight">Agents</h1>
          <p className="mt-1 text-md text-text-muted">
            Assistants that carry out tasks on your Mac, step by step — every action is checked, risky ones
            ask first, and you can pause or stop them anytime.
          </p>
        </div>
        {!editing && (
          <Button variant="primary" onClick={() => setEditing(blank)}>
            New agent
          </Button>
        )}
      </div>

      {editing && (
        <div className="mb-6">
          <Editor initial={editing} onDone={() => setEditing(null)} />
        </div>
      )}

      {agents.length === 0 && !editing ? (
        <EmptyState icon={<BotIcon size={28} />} title="No agents yet">
          <span className="block">Start from a template, or create your own.</span>
          <span className="mt-3 flex flex-wrap justify-center gap-2">
            {templates.map((t) => (
              <Button key={t.name} size="sm" onClick={() => void save(t)}>
                {t.avatar} {t.name}
              </Button>
            ))}
          </span>
        </EmptyState>
      ) : (
        agents.length > 0 && (
          <ul
            aria-label="Agents"
            className="divide-y divide-border rounded-[var(--radius-md)] border border-border bg-surface"
          >
            {agents.map((a) => (
              <AgentRow key={a.id} a={a} onEdit={() => setEditing(toParams(a))} />
            ))}
          </ul>
        )
      )}

      {recent.length > 0 && (
        <section className="mt-8">
          <h2 className="mb-2 text-sm font-semibold text-text-muted">Recent runs</h2>
          <div className="space-y-2">
            {recent.map((r) => (
              <AgentRunCard key={r.id} run={r} compact />
            ))}
          </div>
        </section>
      )}
    </>
  );
}
