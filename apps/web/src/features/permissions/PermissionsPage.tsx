import type { AxSnapshot, PermissionState, SettingsPane } from "@wisp/contracts";
import { CheckIcon } from "@wisp/icons";
import { Button, ErrorNotice, SettingsGroup, SettingsRow, cn } from "@wisp/ui";
import { useEffect, useState } from "react";
import { ipc } from "../../services/ipc";
import { usePermissionsStore, type PermissionKind } from "../../stores/permissionsStore";

const rows: { kind: PermissionKind; label: string; why: string; pane: SettingsPane }[] = [
  {
    kind: "microphone",
    label: "Microphone",
    why: "Hear your questions. Only used while you're holding the shortcut to talk.",
    pane: "microphone",
  },
  {
    kind: "screenRecording",
    label: "Screen Recording",
    why: "See the area you point at. Only the area you select, only when you select it.",
    pane: "screenRecording",
  },
  {
    kind: "accessibility",
    label: "Accessibility",
    why: "Understand buttons, menus and fields in other apps, and notice your shortcut anywhere.",
    pane: "accessibility",
  },
];

const badge: Record<PermissionState, { text: string; tone: string }> = {
  enabled: { text: "Enabled", tone: "bg-success" },
  needsAttention: { text: "Needs attention", tone: "bg-warning" },
  notRequested: { text: "Not requested", tone: "bg-text-faint" },
};

function Status({ state }: { state: PermissionState | undefined }) {
  if (!state) return <span className="text-sm text-text-faint">Checking…</span>;
  const b = badge[state];
  return (
    <span className="inline-flex items-center gap-1.5 text-sm text-text-muted">
      <span aria-hidden className={cn("h-1.5 w-1.5 rounded-full", b.tone)} />
      {b.text}
    </span>
  );
}

/** What works with the current permissions (spec §25). Nothing here is aspirational. */
function Capabilities({ states }: { states: Partial<Record<PermissionKind, PermissionState>> }) {
  const on = (k: PermissionKind) => states[k] === "enabled";
  const items: [string, boolean, string?][] = [
    ["Open Wisp from the buddy or menu bar", true],
    ["Point Wisp at an area, window or screen", on("screenRecording"), "needs Screen Recording"],
    ["Understand app controls under the pointer", on("accessibility"), "needs Accessibility"],
    ["Summon with ⌥⌘ in any app", on("accessibility"), "needs Accessibility"],
    ["Talk to Wisp", on("microphone"), "needs Microphone"],
  ];
  return (
    <ul className="space-y-1 px-3 py-2.5">
      {items.map(([text, ok, need]) => (
        <li key={text} className="flex items-center gap-2 text-sm">
          {ok ? (
            <CheckIcon size={14} className="text-success" />
          ) : (
            <span className="inline-block h-3.5 w-3.5 rounded-full border border-border-strong" />
          )}
          <span className={ok ? "text-text" : "text-text-muted"}>{text}</span>
          {!ok && need && <span className="text-xs text-text-faint">— {need}</span>}
        </li>
      ))}
    </ul>
  );
}

function Inspector({ enabled }: { enabled: boolean }) {
  const [countdown, setCountdown] = useState<number | null>(null);
  const [snap, setSnap] = useState<AxSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);

  const run = () => {
    setSnap(null);
    setError(null);
    let n = 3;
    setCountdown(n);
    const t = setInterval(() => {
      n -= 1;
      if (n > 0) return setCountdown(n);
      clearInterval(t);
      setCountdown(null);
      ipc()
        .call("host.ax.snapshot", { maxNodes: 50 })
        .then(setSnap)
        .catch((e: Error) => setError(e.message));
    }, 1000);
  };

  const el = snap?.underPointer;
  return (
    <div className="space-y-2 px-3 py-2.5">
      <div className="flex items-center justify-between gap-4">
        <p className="text-sm text-text-muted">
          Check what Wisp reads: start, then hover any control in another app.
        </p>
        <Button size="sm" disabled={!enabled || countdown !== null} onClick={run}>
          {countdown !== null ? `Reading in ${countdown}…` : "Inspect Under Pointer"}
        </Button>
      </div>
      {error && <p className="text-sm text-danger">{error}</p>}
      {snap && (
        <dl className="selectable grid grid-cols-[110px_1fr] gap-x-3 gap-y-0.5 rounded-[var(--radius-sm)] bg-bg-sunken p-2 text-sm">
          <dt className="text-text-faint">App</dt>
          <dd>{snap.application?.name ?? "—"}</dd>
          <dt className="text-text-faint">Window</dt>
          <dd>{snap.window?.title ?? "—"}</dd>
          <dt className="text-text-faint">Element</dt>
          <dd>
            {el
              ? `${el.role}${el.title ? ` “${el.title}”` : el.description ? ` “${el.description}”` : ""}`
              : "Nothing readable here"}
          </dd>
          {el?.frame && (
            <>
              <dt className="text-text-faint">Bounds</dt>
              <dd className="font-mono text-xs">
                ({Math.round(el.frame.x)}, {Math.round(el.frame.y)}) {Math.round(el.frame.width)}×
                {Math.round(el.frame.height)}
              </dd>
            </>
          )}
          {el?.actions?.length ? (
            <>
              <dt className="text-text-faint">Actions</dt>
              <dd>{el.actions.join(", ")}</dd>
            </>
          ) : null}
          <dt className="text-text-faint">Inside</dt>
          <dd className="truncate">
            {snap.ancestors
              ?.slice(0, 4)
              .map((a) => a.title || a.role)
              .join(" › ") || "—"}
          </dd>
          <dt className="text-text-faint">Read in</dt>
          <dd>{snap.elapsedMs} ms</dd>
        </dl>
      )}
    </div>
  );
}

export function PermissionsPage() {
  const { states, check, request, checking } = usePermissionsStore();
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void check();
    const onFocus = () => void check();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [check]);

  return (
    <>
      <h1 className="mb-6 text-xl font-semibold tracking-tight">Permissions</h1>
      {error && (
        <div className="mb-4">
          <ErrorNotice title="That didn't work." message={error} />
        </div>
      )}
      <SettingsGroup
        title="macOS permissions"
        description="macOS asks once per permission. If you said no, use Open System Settings to turn it on — Wisp notices when you come back."
      >
        {rows.map((r) => (
          <SettingsRow
            key={r.kind}
            label={
              <span className="flex items-center gap-3">
                {r.label} <Status state={states[r.kind]} />
              </span>
            }
            hint={r.why}
            control={
              states[r.kind] === "enabled" ? null : (
                <div className="flex gap-1.5">
                  <Button
                    size="sm"
                    variant="primary"
                    onClick={() => void request(r.kind).catch((e: Error) => setError(e.message))}
                  >
                    {states[r.kind] === "needsAttention" ? "Open System Settings" : "Allow"}
                  </Button>
                  <Button size="sm" disabled={checking} onClick={() => void check()}>
                    Check Again
                  </Button>
                </div>
              )
            }
          />
        ))}
      </SettingsGroup>

      <SettingsGroup title="What Wisp can do right now">
        <Capabilities states={states} />
      </SettingsGroup>

      <SettingsGroup title="Accessibility inspector">
        <Inspector enabled={states.accessibility === "enabled"} />
      </SettingsGroup>
    </>
  );
}
