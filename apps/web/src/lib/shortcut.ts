import type { Modifier, Shortcut } from "@wisp/contracts";

const order: Modifier[] = ["control", "option", "shift", "command", "function"];
const symbols: Record<Modifier, string> = {
  control: "⌃",
  option: "⌥",
  shift: "⇧",
  command: "⌘",
  function: "fn",
};
const names: Record<Modifier, string> = {
  control: "Control",
  option: "Option",
  shift: "Shift",
  command: "Command",
  function: "Fn",
};

/** Compact macOS notation, e.g. "⌥⌘" or "⌃⌥Space". Mirrors KeyNames.display in Swift. */
export function formatShortcut(s: Shortcut): string {
  const mods = order
    .filter((m) => s.modifiers.includes(m))
    .map((m) => symbols[m])
    .join("");
  return mods + (s.keyLabel ?? (s.keyCode != null ? `Key ${s.keyCode}` : ""));
}

/** Spoken form for VoiceOver, e.g. "Option Command". */
export function speakShortcut(s: Shortcut): string {
  const mods = order.filter((m) => s.modifiers.includes(m)).map((m) => names[m]);
  return [...mods, ...(s.keyLabel ? [s.keyLabel] : [])].join(" ");
}
