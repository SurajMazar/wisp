import type { CursorShape } from "@wisp/contracts";

/** SVG outlines mirroring BuddyShapes.swift (20×20 grid, y down, tip toward the top-left). */
type Pt = [number, number, number]; // x, y, corner radius

const wisp: Pt[] = [
  [1.0, 1.0, 1.4],
  [19.4, 7.4, 2.0],
  [9.2, 9.2, 1.2],
  [7.4, 19.4, 2.0],
];
const pointer: Pt[] = [
  [2, 1, 0.8],
  [2, 17.5, 0.8],
  [6.2, 13.8, 0.6],
  [9.1, 20.2, 0.8],
  [11.6, 19.1, 0.8],
  [8.8, 12.9, 0.6],
  [14.4, 12.9, 0.8],
];

type XY = [number, number];

function rounded(pts: Pt[]): string {
  const n = pts.length;
  const at = (i: number): Pt => pts[((i % n) + n) % n] ?? [0, 0, 0];
  const toward = (from: Pt, to: Pt): XY => {
    const dx = to[0] - from[0];
    const dy = to[1] - from[1];
    const len = Math.hypot(dx, dy) || 1;
    const k = Math.min(from[2], len) / len;
    return [from[0] + dx * k, from[1] + dy * k];
  };
  const f = (v: number) => v.toFixed(2);
  const first = at(0);
  const last = at(n - 1);
  let d = `M${f((first[0] + last[0]) / 2)} ${f((first[1] + last[1]) / 2)}`;
  for (let i = 0; i < n; i++) {
    const p = at(i);
    const [ax, ay] = toward(p, at(i - 1));
    const [bx, by] = toward(p, at(i + 1));
    d += ` L${f(ax)} ${f(ay)} Q${p[0]} ${p[1]} ${f(bx)} ${f(by)}`;
  }
  return `${d} Z`;
}

export const shapePaths: Record<CursorShape, string> = {
  wisp: rounded(wisp),
  pointer: rounded(pointer),
  dot: "M1.6 7.8 a6.2 6.2 0 1 0 12.4 0 a6.2 6.2 0 1 0 -12.4 0 Z",
};

export const glowPresets: { label: string; from: string; to: string }[] = [
  { label: "Aurora", from: "#8B7CF6", to: "#5EC8F2" },
  { label: "Sunset", from: "#F97316", to: "#EC4899" },
  { label: "Ocean", from: "#2563EB", to: "#14B8A6" },
  { label: "Mint", from: "#22C55E", to: "#06B6D4" },
  { label: "Ember", from: "#EF4444", to: "#F59E0B" },
];
