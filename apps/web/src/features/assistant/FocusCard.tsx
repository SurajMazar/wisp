import type { FocusKind, FocusRegion } from "@wisp/contracts";
import { Button } from "@wisp/ui";
import { useFocusStore } from "../../stores/focusStore";

const kindLabel: Record<FocusKind, string> = {
  point: "Point",
  rectangle: "Area",
  lasso: "Freeform area",
  circle: "Circled area",
  window: "Window",
  screen: "Screen",
};

export function describeRegion(r: FocusRegion): string {
  const size = `${Math.round(r.selection.width)} × ${Math.round(r.selection.height)} pt`;
  const where = r.window?.title ?? r.application?.name;
  return [size, r.display.name, where].filter(Boolean).join(" · ");
}

/** What the assistant is focused on, with a thumbnail of exactly what was captured. */
export function FocusCard({ region }: { region: FocusRegion }) {
  const clear = useFocusStore((s) => s.clear);
  return (
    <figure className="flex items-center gap-3 rounded-[var(--radius-md)] border border-border bg-surface/70 p-2">
      {region.image ? (
        <img
          src={region.image.url}
          alt={`Captured ${kindLabel[region.kind].toLowerCase()}`}
          className="h-14 w-20 shrink-0 rounded-[var(--radius-sm)] border border-border object-cover"
        />
      ) : (
        <div className="h-14 w-20 shrink-0 rounded-[var(--radius-sm)] bg-bg-sunken" />
      )}
      <figcaption className="min-w-0 flex-1">
        <p className="text-sm font-medium text-text">Focused: {kindLabel[region.kind]}</p>
        <p className="truncate text-xs text-text-muted">{describeRegion(region)}</p>
        {region.image && (
          <p className="text-xs text-text-faint">
            {region.image.widthPx} × {region.image.heightPx} px · stays on this Mac
          </p>
        )}
      </figcaption>
      <Button size="sm" variant="ghost" onClick={() => void clear()}>
        Clear
      </Button>
    </figure>
  );
}
