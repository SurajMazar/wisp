import type { Routine, RoutineSaveParams } from "@wisp/contracts";
import { DeleteIcon, EditIcon, PlayIcon, RoutinesIcon } from "@wisp/icons";
import { Button, EmptyState, ErrorNotice, Switch } from "@wisp/ui";
import { useEffect, useState } from "react";
import { Link } from "react-router";
import { useAgentStore } from "../../stores/agentStore";
import { useRoutineStore } from "../../stores/routineStore";

const presets: { label: string; cron: string }[] = [
  { label: "Every 5 minutes", cron: "*/5 * * * *" },
  { label: "Every hour", cron: "0 * * * *" },
  { label: "Every morning at 9", cron: "0 9 * * *" },
  { label: "Weekdays at 9", cron: "0 9 * * 1-5" },
  { label: "Every evening at 6", cron: "0 18 * * *" },
  { label: "Fridays at 5 pm", cron: "0 17 * * 5" },
];

const localZone = () => Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
const when = (ms?: number | null) =>
  ms
    ? new Date(ms).toLocaleString([], {
        weekday: "short",
        month: "short",
        day: "numeric",
        hour: "numeric",
        minute: "2-digit",
      })
    : "—";
const field = "w-full rounded-[var(--radius-sm)] border border-border-strong bg-bg px-2 py-1.5 text-base";

function Editor({ initial, onDone }: { initial: RoutineSaveParams; onDone: () => void }) {
  const { save, error } = useRoutineStore();
  const agents = useAgentStore((s) => s.agents);
  const loadAgents = useAgentStore((s) => s.load);
  const [d, setD] = useState(initial);
  useEffect(() => {
    void loadAgents().catch(() => {});
  }, [loadAgents]);
  const set = (p: Partial<RoutineSaveParams>) => setD((x) => ({ ...x, ...p }));
  return (
    <form
      aria-label={d.id ? "Edit routine" : "New routine"}
      className="mb-6 space-y-3 rounded-[var(--radius-lg)] border border-border bg-surface p-4"
      onSubmit={(e) => {
        e.preventDefault();
        void save(d).then((ok) => ok && onDone());
      }}
    >
      <label className="block">
        <span className="text-xs text-text-muted">Name</span>
        <input
          aria-label="Name"
          required
          className={field}
          value={d.name}
          onChange={(e) => set({ name: e.target.value })}
        />
      </label>
      <label className="block">
        <span className="text-xs text-text-muted">What should Wisp do?</span>
        <textarea
          aria-label="Prompt"
          rows={3}
          required
          className={field}
          placeholder="e.g. Summarize my notes from today"
          value={d.prompt}
          onChange={(e) => set({ prompt: e.target.value })}
        />
      </label>
      <div className="grid grid-cols-2 gap-3">
        <label className="block">
          <span className="text-xs text-text-muted">When</span>
          <select
            aria-label="When"
            className={field}
            value={presets.some((p) => p.cron === d.schedule) ? d.schedule : "custom"}
            onChange={(e) => e.target.value !== "custom" && set({ schedule: e.target.value })}
          >
            {presets.map((p) => (
              <option key={p.cron} value={p.cron}>
                {p.label}
              </option>
            ))}
            <option value="custom">Custom (cron)…</option>
          </select>
        </label>
        <label className="block">
          <span className="text-xs text-text-muted">Cron (minute hour day month weekday)</span>
          <input
            aria-label="Cron"
            className={`${field} font-mono`}
            value={d.schedule}
            onChange={(e) => set({ schedule: e.target.value })}
          />
        </label>
        <label className="block">
          <span className="text-xs text-text-muted">Timezone</span>
          <input
            aria-label="Timezone"
            className={field}
            value={d.timezone}
            onChange={(e) => set({ timezone: e.target.value })}
          />
        </label>
        <label className="block">
          <span className="text-xs text-text-muted">Run as</span>
          <select
            aria-label="Run as"
            className={field}
            value={d.agentId ?? ""}
            onChange={(e) =>
              setD(({ agentId: _, ...rest }) =>
                e.target.value ? { ...rest, agentId: e.target.value } : rest,
              )
            }
          >
            <option value="">A question to Wisp</option>
            {agents.map((a) => (
              <option key={a.id} value={a.id}>
                {a.avatar} {a.name} (agent)
              </option>
            ))}
          </select>
        </label>
      </div>
      <div className="flex flex-wrap items-center gap-4 text-sm">
        <label className="flex items-center gap-2">
          Tries
          <input
            aria-label="Tries"
            type="number"
            min={1}
            max={5}
            className="w-14 rounded-[var(--radius-sm)] border border-border-strong bg-bg px-1.5 py-1"
            value={d.retry?.maxAttempts ?? 2}
            onChange={(e) =>
              set({
                retry: { maxAttempts: Number(e.target.value), backoffMinutes: d.retry?.backoffMinutes ?? 5 },
              })
            }
          />
        </label>
        <label className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={d.notify?.onSuccess ?? true}
            onChange={(e) =>
              set({ notify: { onSuccess: e.target.checked, onFailure: d.notify?.onFailure ?? true } })
            }
          />
          Notify when done
        </label>
        <label className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={d.notify?.onFailure ?? true}
            onChange={(e) =>
              set({ notify: { onSuccess: d.notify?.onSuccess ?? true, onFailure: e.target.checked } })
            }
          />
          Notify if it fails
        </label>
      </div>
      {error && <ErrorNotice title="That routine couldn't be saved." message={error} />}
      <div className="flex gap-2">
        <Button type="submit" variant="primary">
          Save routine
        </Button>
        <Button variant="ghost" onClick={onDone}>
          Cancel
        </Button>
      </div>
    </form>
  );
}

function Row({ r, onEdit }: { r: Routine; onEdit: () => void }) {
  const { save, remove, runNow, runs, loadRuns } = useRoutineStore();
  const [open, setOpen] = useState(false);
  const history = runs[r.id] ?? [];
  return (
    <li className="px-3 py-3">
      <div className="flex items-center gap-3">
        <div className="min-w-0 flex-1">
          <p className="truncate text-base font-medium">{r.name}</p>
          <p className="truncate text-sm text-text-muted">
            {r.scheduleText} · {r.timezone}
            {r.enabled ? ` · next ${when(r.nextRunAtMs)}` : " · off"}
            {r.lastStatus &&
              ` · last ${r.lastStatus === "succeeded" ? "worked" : r.lastStatus === "failed" ? "failed" : "running"}`}
          </p>
        </div>
        <Switch
          label={`Run ${r.name} on schedule`}
          checked={r.enabled}
          onCheckedChange={(v) =>
            void save({
              id: r.id,
              name: r.name,
              schedule: r.schedule,
              timezone: r.timezone,
              prompt: r.prompt,
              enabled: v,
              retry: r.retry,
              notify: r.notify,
              ...(r.agentId ? { agentId: r.agentId } : {}),
            })
          }
        />
        <Button size="sm" onClick={() => void runNow(r.id).then(() => setOpen(true))}>
          <PlayIcon size={13} /> Run now
        </Button>
        <Button size="sm" variant="ghost" aria-label={`Edit ${r.name}`} onClick={onEdit}>
          <EditIcon size={14} />
        </Button>
        <Button size="sm" variant="ghost" aria-label={`Delete ${r.name}`} onClick={() => void remove(r.id)}>
          <DeleteIcon size={14} />
        </Button>
      </div>
      <button
        type="button"
        className="mt-1 text-xs text-text-faint hover:text-text"
        onClick={() => {
          setOpen((o) => !o);
          void loadRuns(r.id).catch(() => {});
        }}
      >
        {open ? "Hide history" : "History"}
      </button>
      {open && (
        <ul
          aria-label={`${r.name} history`}
          className="mt-1 space-y-0.5 border-l-2 border-border pl-2.5 text-xs"
        >
          {history.length === 0 && <li className="text-text-faint">No runs yet.</li>}
          {history.map((x) => (
            <li key={x.id} className={x.status === "failed" ? "text-danger" : "text-text-muted"}>
              {when(x.startedAtMs)} ·{" "}
              {x.status === "running"
                ? "Running…"
                : x.status === "succeeded"
                  ? "Worked"
                  : `Failed: ${x.error ?? ""}`}
              {x.attempt > 1 && ` (try ${x.attempt})`}
              {x.conversationId && (
                <>
                  {" · "}
                  <Link to={`/conversations/${x.conversationId}`} className="text-accent hover:underline">
                    See result
                  </Link>
                </>
              )}
            </li>
          ))}
        </ul>
      )}
    </li>
  );
}

/** Routines (spec §66): prompts or agents that run on a schedule. */
export function RoutinesPage() {
  const { routines, load } = useRoutineStore();
  const [editing, setEditing] = useState<RoutineSaveParams | null>(null);
  useEffect(() => {
    void load().catch(() => {});
  }, [load]);
  const blank: RoutineSaveParams = {
    name: "",
    prompt: "",
    schedule: "0 9 * * *",
    timezone: localZone(),
    enabled: true,
    retry: { maxAttempts: 2, backoffMinutes: 5 },
    notify: { onSuccess: true, onFailure: true },
  };
  return (
    <>
      <div className="mb-6 flex items-start gap-3">
        <div className="flex-1">
          <h1 className="text-xl font-semibold tracking-tight">Routines</h1>
          <p className="mt-1 text-md text-text-muted">
            Things Wisp does on a schedule — like a morning summary of your notes. They run while Wisp is
            open; a run missed while it was closed happens once when it opens.
          </p>
        </div>
        {!editing && (
          <Button variant="primary" onClick={() => setEditing(blank)}>
            New routine
          </Button>
        )}
      </div>
      {editing && <Editor initial={editing} onDone={() => setEditing(null)} />}
      {routines.length === 0 ? (
        !editing && (
          <EmptyState icon={<RoutinesIcon size={28} />} title="No routines yet">
            Create one to have Wisp do something every morning, every Friday, or every few minutes.
          </EmptyState>
        )
      ) : (
        <ul
          aria-label="Routines"
          className="divide-y divide-border rounded-[var(--radius-md)] border border-border bg-surface"
        >
          {routines.map((r) => (
            <Row
              key={r.id}
              r={r}
              onEdit={() =>
                setEditing({
                  id: r.id,
                  name: r.name,
                  prompt: r.prompt,
                  schedule: r.schedule,
                  timezone: r.timezone,
                  enabled: r.enabled,
                  retry: r.retry,
                  notify: r.notify,
                  ...(r.agentId ? { agentId: r.agentId } : {}),
                })
              }
            />
          ))}
        </ul>
      )}
    </>
  );
}
