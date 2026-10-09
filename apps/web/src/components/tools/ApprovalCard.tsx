import type { ApprovalDecision, ApprovalRequest } from "@wisp/contracts";
import { RiskIcon } from "@wisp/icons";
import { Button, cn } from "@wisp/ui";
import { seconds, useNow } from "../../lib/useNow";
import { useToolStore } from "../../stores/toolStore";
import { RiskBadge } from "./RiskBadge";

const label: Record<ApprovalDecision, string> = {
  allowOnce: "Allow once",
  allowSession: "Allow for this session",
  alwaysAllow: "Always allow",
  deny: "Deny",
};

/** "The assistant wants to: …" (spec §58). Riskier requests offer fewer choices. */
export function ApprovalCard({ request: r }: { request: ApprovalRequest }) {
  const respond = useToolStore((s) => s.respond);
  const now = useNow(true);
  const left = Math.max(0, r.expiresAtMs - now);
  const severe = r.risk === "high" || r.risk === "critical";
  const isText = r.tool === "typeText" || r.tool === "writeClipboard";
  const allow = r.choices.filter((c) => c !== "deny");

  return (
    <div
      role="alertdialog"
      aria-label={`Wisp wants to: ${r.title}`}
      className={cn(
        "rounded-[var(--radius-lg)] border bg-surface p-3 shadow-[var(--shadow-md)]",
        severe ? "border-danger/50" : "border-border-strong",
      )}
    >
      <div className="flex items-center gap-2">
        <RiskIcon size={15} className={severe ? "text-danger" : "text-text-muted"} />
        <span className="text-sm text-text-muted">Wisp wants to</span>
        <span className="ml-auto">
          <RiskBadge risk={r.risk} />
        </span>
      </div>
      <p className="mt-1.5 text-md font-semibold">{r.title}</p>
      {r.detail &&
        (isText ? (
          <pre className="selectable mt-1.5 max-h-28 overflow-auto whitespace-pre-wrap rounded-[var(--radius-sm)] bg-bg-sunken p-2 font-mono text-xs">
            {r.detail}
          </pre>
        ) : (
          <p className="mt-0.5 text-sm text-text-muted">{r.detail}</p>
        ))}
      {r.riskReason && (
        <p className={cn("mt-1.5 text-sm", severe ? "text-danger" : "text-warning")}>
          Careful: {r.riskReason}.
        </p>
      )}
      <div className="mt-3 flex flex-wrap items-center gap-1.5">
        {allow.map((c, i) => (
          <Button
            key={c}
            size="sm"
            variant={i === 0 ? (r.risk === "critical" ? "danger" : "primary") : "secondary"}
            onClick={() => void respond(r.id, c)}
          >
            {label[c]}
          </Button>
        ))}
        <Button size="sm" variant="ghost" onClick={() => void respond(r.id, "deny")}>
          Deny
        </Button>
        <span className="ml-auto text-xs tabular-nums text-text-faint">Denies in {seconds(left)}</span>
      </div>
    </div>
  );
}

/** Every pending request, newest last. */
export function ApprovalStack({ className }: { className?: string }) {
  const pending = useToolStore((s) => s.pending);
  if (pending.length === 0) return null;
  return (
    <div className={cn("space-y-2", className)} aria-live="assertive">
      {pending.map((r) => (
        <ApprovalCard key={r.id} request={r} />
      ))}
    </div>
  );
}
