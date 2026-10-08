import type { AppearanceSettings } from "@wisp/contracts";

/** Apply theme + accent to the document. `system` follows prefers-color-scheme live. */
export function applyAppearance(
  appearance: AppearanceSettings,
  root: HTMLElement = document.documentElement,
): () => void {
  root.dataset.accent = appearance.accent === "custom" ? "default" : appearance.accent;
  if (appearance.accent === "custom" && appearance.customColor) {
    root.style.setProperty("--wisp-accent", appearance.customColor);
  } else {
    root.style.removeProperty("--wisp-accent");
  }

  if (appearance.theme !== "system") {
    root.dataset.theme = appearance.theme;
    return () => {};
  }
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  const apply = () => (root.dataset.theme = mq.matches ? "dark" : "light");
  apply();
  mq.addEventListener("change", apply);
  return () => mq.removeEventListener("change", apply);
}
