import type { ToolCallRecord } from "@wisp/contracts";
import { ActivityIcon } from "@wisp/icons";
import { EmptyState, cn } from "@wisp/ui";
import { useEffect } from "react";
import { RiskBadge } from "../../components/tools/RiskBadge";
import { useToolStore } from "../../stores/toolStore";

const statusText: Record<ToolCallRecord["status"], string> = {
  waitingForApproval: "Waiting for you",
  running: "Running…",
  succeeded: "Done",
  failed: "Didn't work",
  denied: "Not allowed",
};

const decisionText: Record<string, string> = {
  autoAllow: "Read-only",
  granted: "Always allowed",
  allowSession: "Allowed this session",
  allowOnce: "You allowed it",
  alwaysAllow: "You always allow it",
  deny: "You denied it",
  policyDeny: "Blocked by settings",
};

const time = (ms: number) =>
  new Date(ms).toLocaleString([], { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });

/** Everything Wisp did or tried to do on this Mac (spec §63, §78). Never hidden. */
export function ActivityPage() {
  const { activity, loadActivity } = useToolStore();
  useEffect(() => {
    void loadActivity().catch(() => {});
  }, [loadActivity]);

  return (
    <>
      <h1 className="text-xl font-semibold tracking-tight">Activity</h1>
      <p className="mb-6 mt-1 text-md text-text-muted">
        Every action Wisp took or asked to take, and whether it worked. Typed text isn't stored.
      </p>
      {activity.length === 0 ? (
        <EmptyState icon={<ActivityIcon size={28} />} title="Nothing yet">
          When Wisp clicks, types or opens something for you, it shows up here.
        </EmptyState>
      ) : (
        <ul className="divide-y divide-border rounded-[var(--radius-md)] border border-border bg-surface">
          {activity.map((a) => (
            <li key={a.id} className="px-3 py-2.5">
              <div className="flex items-center gap-2">
                <span className="truncate text-base font-medium">{a.title}</span>
                <RiskBadge risk={a.risk} />
                <span
                  className={cn(
                    "ml-auto shrink-0 text-sm",
                    a.status === "succeeded"
                      ? "text-success"
                      : a.status === "failed" || a.status === "denied"
                        ? "text-danger"
                        : "text-text-muted",
                  )}
                >
                  {statusText[a.status]}
                </span>
              </div>
              <p className="mt-0.5 text-sm text-text-muted">
                {time(a.createdAtMs)} · {decisionText[a.decision] ?? a.decision}
                {a.verification && ` · ${a.verification.detail}`}
                {a.error && ` · ${a.error}`}
              </p>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}
