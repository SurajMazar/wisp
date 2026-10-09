import type { FocusKind } from "@wisp/contracts";
import { Button } from "@wisp/ui";
import { useFocusStore } from "../../stores/focusStore";

const actions: { kind: FocusKind; label: string; hint: string }[] = [
  { kind: "rectangle", label: "Area", hint: "Drag around something" },
  { kind: "point", label: "Point", hint: "Click a spot" },
  { kind: "window", label: "Window", hint: "The window under your pointer" },
  { kind: "screen", label: "Screen", hint: "The whole display" },
];

/** Start a focus without the shortcut (works without Accessibility permission). */
export function FocusActions() {
  const { start, permission, lastEnded, error, requestPermission } = useFocusStore();
  const needsPermission = permission === "needsAttention" || lastEnded?.reason === "permissionDenied";

  if (needsPermission) {
    return (
      <div
        role="alert"
        className="rounded-[var(--radius-md)] border border-warning/30 bg-warning/10 p-2.5 text-sm"
      >
        <p className="font-medium text-text">Let Wisp see what you point at</p>
        <p className="mt-0.5 text-text-muted">
          Screen access is off. Wisp only captures the area you select, only when you select it.
        </p>
        <Button size="sm" variant="primary" className="mt-2" onClick={() => void requestPermission()}>
          Allow Screen Access
        </Button>
      </div>
    );
  }

  return (
    <div>
      <div className="flex gap-1.5" role="group" aria-label="Focus on">
        {actions.map((a) => (
          <Button key={a.kind} size="sm" title={a.hint} onClick={() => void start(a.kind)}>
            {a.label}
          </Button>
        ))}
      </div>
      {(error || (lastEnded && lastEnded.reason !== "cancelled")) && (
        <p className="mt-1.5 text-xs text-text-muted" role="status">
          {error ??
            lastEnded?.detail ??
            (lastEnded?.reason === "nothingSelected" ? "Nothing was selected." : "")}
        </p>
      )}
    </div>
  );
}
