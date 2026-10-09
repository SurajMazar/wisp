import type { AccentColor } from "@wisp/contracts";

/** Accent palette shared by app appearance and the assistant cursor (mirrors BuddyTheme.swift). */
export const accents: {
  value: Exclude<AccentColor, "custom">;
  label: string;
  color: string;
  swatch: string;
}[] = [
  { value: "default", label: "Wisp", color: "#7c6cf2", swatch: "linear-gradient(135deg,#8b7cf6,#5ec8f2)" },
  { value: "blue", label: "Blue", color: "#3b82f6", swatch: "#3b82f6" },
  { value: "purple", label: "Purple", color: "#9b5de5", swatch: "#9b5de5" },
  { value: "cyan", label: "Cyan", color: "#0fb5c9", swatch: "#0fb5c9" },
  { value: "green", label: "Green", color: "#2fae66", swatch: "#2fae66" },
  { value: "orange", label: "Orange", color: "#f08a24", swatch: "#f08a24" },
  { value: "red", label: "Red", color: "#e5484d", swatch: "#e5484d" },
  { value: "pink", label: "Pink", color: "#e5559b", swatch: "#e5559b" },
  { value: "monochrome", label: "Monochrome", color: "#8a8a84", swatch: "#8a8a84" },
];

export function accentColor(theme: AccentColor, custom?: string | null): string {
  if (theme === "custom") return custom ?? "#7c6cf2";
  return accents.find((a) => a.value === theme)?.color ?? "#7c6cf2";
}
