import type { FocusRegion } from "@wisp/contracts";

/** "320 × 180 pt · Built-in Retina Display · Docs" */
export function describeRegion(r: FocusRegion): string {
  const size = `${Math.round(r.selection.width)} × ${Math.round(r.selection.height)} pt`;
  const where = r.window?.title ?? r.application?.name;
  return [size, r.display.name, where].filter(Boolean).join(" · ");
}

/** The most specific name for what was selected. */
export function regionLabel(r: FocusRegion): string {
  const el = r.accessibility?.underPointer;
  if (el?.title) return el.title;
  return r.window?.title ?? r.application?.name ?? "Screen area";
}
