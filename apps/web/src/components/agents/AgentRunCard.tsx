import type { AgentRun } from "@wisp/contracts";
import { CloseIcon, PauseIcon, PlayIcon, StepDoneIcon, StepPendingIcon } from "@wisp/icons";
import { Button, cn } from "@wisp/ui";
import { useState } from "react";
import { useNow } from "../../lib/useNow";
import { runActive, useAgentStore } from "../../stores/agentStore";

const statusText: Record<AgentRun["status"], string> = {
  planning: "Planning…",
  running: "Working…",
  paused: "Paused",
  waitingPermission: "Waiting for your OK",
  stuck: "Needs you",
  needsInput: "Has a question",
  completed: "Done",
  failed: "Couldn't finish",
  cancelled: "Stopped",
};

/** Live view of an agent run (spec §63): the plan, every action, and the controls. Never hidden. */
export function AgentRunCard({ run, compact }: { run: AgentRun; compact?: boolean }) {
  const { control, dismiss } = useAgentStore();
  const [reply, setReply] = useState("");
  const active = runActive(run);
  const acts = run.steps.filter((s) => s.kind === "act" || (s.kind === "message" && s.status === "failed"));

  return (
    <section
      aria-label={`${run.agentName}: ${run.goal}`}
      className="rounded-[var(--radius-lg)] border border-border-strong bg-surface p-3"
    >
      <header className="flex items-center gap-2">
        <span aria-hidden className="text-base">
          {run.avatar}
        </span>
        <span className="min-w-0 flex-1 truncate text-sm">
          <span className="font-semibold">{run.agentName}</span>{" "}
          <span className="text-text-muted">· {run.goal}</span>
        </span>
        <span
          role="status"
          className={cn(
            "text-xs",
            active ? "text-accent" : run.status === "completed" ? "text-success" : "text-text-muted",
          )}
        >
          {statusText[run.status]}
        </span>
        {!active && (
          <button
            type="button"
            aria-label="Close"
            onClick={() => dismiss(run.id)}
            className="rounded p-0.5 text-text-faint hover:text-text"
          >
            <CloseIcon size={13} />
          </button>
        )}
      </header>

      {run.plan.length > 0 && (
        <ol className="mt-2 space-y-0.5" aria-label="Plan">
          {run.plan.map((p, i) => (
            <li key={i} className="flex items-center gap-2 text-sm">
              {p.status === "done" ? (
                <StepDoneIcon size={14} className="shrink-0 text-success" />
              ) : p.status === "active" && active ? (
                <span className="flex h-3.5 w-3.5 shrink-0 items-center justify-center">
                  <span className="h-2 w-2 animate-pulse rounded-full bg-accent" />
                </span>
              ) : (
                <StepPendingIcon size={14} className="shrink-0 text-text-faint" />
              )}
              <span className={p.status === "pending" ? "text-text-muted" : ""}>{p.title}</span>
            </li>
          ))}
        </ol>
      )}

      {acts.length > 0 && (
        <details className="mt-2" open={!compact}>
          <summary className="cursor-pointer text-xs text-text-faint">
            {acts.length} action{acts.length === 1 ? "" : "s"} · {run.toolCallsUsed}/{run.limits.maxToolCalls}{" "}
            used
          </summary>
          <ul
            className={cn(
              "mt-1 space-y-0.5 border-l-2 border-border pl-2.5 text-xs",
              compact && "max-h-28 overflow-y-auto",
            )}
          >
            {acts.map((s) => (
              <li key={s.seq} className={cn(s.status === "failed" ? "text-danger" : "text-text-muted")}>
                {s.status === "running"
                  ? "● "
                  : s.status === "succeeded"
                    ? "✓ "
                    : s.status === "failed"
                      ? "✗ "
                      : "· "}
                <span className="text-text">{s.title}</span>
                {s.detail && <span> — {s.detail}</span>}
              </li>
            ))}
          </ul>
        </details>
      )}

      {run.note && (
        <p
          className={cn(
            "mt-2 text-sm",
            run.status === "stuck" || run.status === "failed" ? "text-warning" : "text-text",
          )}
        >
          {run.note}
        </p>
      )}

      {run.status === "needsInput" && (
        <form
          className="mt-2 flex gap-1.5"
          onSubmit={(e) => {
            e.preventDefault();
            if (!reply.trim()) return;
            void control(run.id, "continue", reply.trim());
            setReply("");
          }}
        >
          <input
            aria-label="Your answer"
            value={reply}
            onChange={(e) => setReply(e.target.value)}
            placeholder="Your answer"
            className="h-7 flex-1 rounded-[var(--radius-sm)] border border-border-strong bg-bg px-2 text-sm"
          />
          <Button size="sm" type="submit" variant="primary">
            Reply
          </Button>
        </form>
      )}

      {active && (
        <div className="mt-2.5 flex flex-wrap gap-1.5">
          {run.status === "stuck" && (
            <Button size="sm" variant="primary" onClick={() => void control(run.id, "continue")}>
              Continue
            </Button>
          )}
          {run.status === "paused" ? (
            <Button size="sm" onClick={() => void control(run.id, "resume")}>
              <PlayIcon size={13} /> Resume
            </Button>
          ) : (
            run.status !== "stuck" &&
            run.status !== "needsInput" && (
              <Button size="sm" onClick={() => void control(run.id, "pause")}>
                <PauseIcon size={13} /> Pause
              </Button>
            )
          )}
          <Button size="sm" variant="ghost" className="ml-auto" onClick={() => void control(run.id, "stop")}>
            Stop
          </Button>
        </div>
      )}
    </section>
  );
}

/** Runs to show in a chat surface: active ones, plus finished ones not yet closed. */
export function RunStack({ conversationId, compact }: { conversationId?: string | null; compact?: boolean }) {
  const { runs, dismissed } = useAgentStore();
  const now = useNow(true, 30_000);
  const shown = Object.values(runs)
    .filter((r) => !dismissed.includes(r.id))
    .filter((r) =>
      conversationId
        ? r.conversationId === conversationId
        : runActive(r) || now - r.startedAtMs < 10 * 60_000,
    )
    .sort((a, b) => a.startedAtMs - b.startedAtMs)
    .slice(-2);
  if (shown.length === 0) return null;
  return (
    <div className="space-y-2">
      {shown.map((r) => (
        <AgentRunCard key={r.id} run={r} {...(compact ? { compact } : {})} />
      ))}
    </div>
  );
}
