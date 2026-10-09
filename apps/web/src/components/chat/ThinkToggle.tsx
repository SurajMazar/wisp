import { ThinkIcon } from "@wisp/icons";
import { cn } from "@wisp/ui";
import { useModelsStore } from "../../stores/modelsStore";
import { useSettingsStore } from "../../stores/settingsStore";

/** Turn the model's "deep thinking" on or off (a global setting, also in Settings → Models). */
export function ThinkToggle({ compact = false }: { compact?: boolean }) {
  const { settings, update } = useSettingsStore();
  const models = settings?.models;
  if (!models) return null;
  const on = models.deepThinking;
  return (
    <button
      type="button"
      aria-pressed={on}
      title={
        on
          ? "Deep thinking is on: slower, reasons before answering. Click to turn off."
          : "Deep thinking is off: faster answers. Click to let the model reason first."
      }
      onClick={() =>
        void update({ section: "models", value: { ...models, deepThinking: !on } }).then(() =>
          useModelsStore.getState().refreshRoutes(),
        )
      }
      className={cn(
        "flex h-7 items-center gap-1 rounded-full px-2 text-sm text-text-muted transition-colors hover:bg-surface-hover hover:text-text",
        on && "bg-accent/15 text-accent hover:bg-accent/20 hover:text-accent",
      )}
    >
      <ThinkIcon size={15} />
      {!compact && <span>Think</span>}
    </button>
  );
}
