import type { ComputerSettings } from "@wisp/contracts";
import { Button, SettingsGroup, SettingsRow, Switch } from "@wisp/ui";
import { useEffect } from "react";
import { useSettingsStore } from "../../stores/settingsStore";
import { useToolStore } from "../../stores/toolStore";

/** Computer control (spec §55–58): off by default; revocable permissions. */
export function ComputerSettingsGroup({ computer }: { computer: ComputerSettings }) {
  const update = useSettingsStore((s) => s.update);
  const { grants, loadGrants, revoke } = useToolStore();
  useEffect(() => {
    void loadGrants().catch(() => {});
  }, [loadGrants]);
  const save = (patch: Partial<ComputerSettings>) =>
    void update({ section: "computer", value: { ...computer, ...patch } });

  const standing = grants?.grants ?? [];
  const session = grants?.session ?? [];

  return (
    <SettingsGroup
      title="Computer control"
      description="Let Wisp click, type and open things for you. Every action is checked for risk, risky ones always ask first, and everything is listed in Activity."
    >
      <SettingsRow
        label="Allow Wisp to control this Mac"
        hint="Needs Accessibility permission. Reading the screen never needs this."
        control={
          <Switch
            label="Allow Wisp to control this Mac"
            checked={computer.enabled}
            onCheckedChange={(v) => save({ enabled: v })}
          />
        }
      />
      <SettingsRow
        label="Ask before every action"
        hint="Ignore “Always allow” and “Allow for this session”."
        control={
          <Switch
            label="Ask before every action"
            checked={computer.confirmEveryAction}
            onCheckedChange={(v) => save({ confirmEveryAction: v })}
          />
        }
      />
      <SettingsRow
        label="Permissions you've given"
        hint={
          standing.length + session.length === 0
            ? "None. Wisp asks before acting."
            : "High-risk actions always ask, whatever is here."
        }
        control={null}
      />
      {[
        ...standing.map((g) => ({
          id: g.id,
          tool: g.tool,
          kind: g.decision === "deny" ? "Never" : "Always",
        })),
        ...session.map((t) => ({ id: t, tool: t, kind: "This session" })),
      ].map((g) => (
        <div key={g.id} className="flex items-center gap-2 px-4 py-1.5 text-sm">
          <span className="font-medium">{g.tool}</span>
          <span className="text-text-muted">· {g.kind}</span>
          <Button size="sm" variant="ghost" className="ml-auto" onClick={() => void revoke(g.id)}>
            Revoke
          </Button>
        </div>
      ))}
    </SettingsGroup>
  );
}
