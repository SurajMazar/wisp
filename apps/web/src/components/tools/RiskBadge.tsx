import type { RiskLevel } from "@wisp/contracts";
import { cn } from "@wisp/ui";

const styles: Record<RiskLevel, string> = {
  safe: "bg-surface-hover text-text-muted",
  low: "bg-accent/12 text-accent",
  medium: "bg-warning/15 text-warning",
  high: "bg-danger/12 text-danger",
  critical: "bg-danger text-white",
};

const names: Record<RiskLevel, string> = {
  safe: "Read-only",
  low: "Low risk",
  medium: "Medium risk",
  high: "High risk",
  critical: "Irreversible",
};

export function RiskBadge({ risk }: { risk: RiskLevel }) {
  return (
    <span className={cn("rounded-full px-2 py-0.5 text-xs font-medium", styles[risk])}>{names[risk]}</span>
  );
}
