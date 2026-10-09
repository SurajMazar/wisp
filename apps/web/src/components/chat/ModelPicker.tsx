import { CheckIcon, ChevronDownIcon, ChipIcon } from "@wisp/icons";
import { Link } from "react-router";
import { useConversationStore } from "../../stores/conversationStore";
import { useModelsStore } from "../../stores/modelsStore";
import { MenuItem, Popover } from "./Popover";

/** "Auto" or a specific model, like the model switchers in other assistants. */
export function ModelPicker() {
  const { model, setModel } = useConversationStore();
  const { list, load } = useModelsStore();
  const models = list?.models ?? [];
  const current = model
    ? models.find((m) => m.providerId === model.providerId && m.id === model.model)
    : null;
  const label = current ? current.name : "Auto";

  return (
    <Popover
      label="Choose model"
      align="end"
      trigger={(open, toggle) => (
        <button
          type="button"
          aria-haspopup="menu"
          aria-expanded={open}
          aria-label={`Model: ${label}`}
          onClick={() => {
            if (!list) void load();
            toggle();
          }}
          className="flex h-7 max-w-44 items-center gap-1 rounded-full px-2.5 text-sm text-text-muted hover:bg-surface-hover hover:text-text"
        >
          <span className="truncate">{label}</span>
          <ChevronDownIcon size={13} className="shrink-0" />
        </button>
      )}
    >
      {(close) => (
        <>
          <MenuItem
            icon={!model ? <CheckIcon size={14} /> : <span className="inline-block w-3.5" />}
            label="Auto"
            hint="Wisp picks the best local model"
            onSelect={() => {
              setModel(null);
              close();
            }}
          />
          {models.map((m) => {
            const on = model?.providerId === m.providerId && model.model === m.id;
            return (
              <MenuItem
                key={`${m.providerId}/${m.id}`}
                icon={on ? <CheckIcon size={14} /> : <ChipIcon size={14} />}
                label={m.name}
                hint={[
                  m.capabilities.includes("vision") ? "Sees images" : "Text",
                  m.locality === "local" ? "On this Mac" : "Cloud",
                ].join(" · ")}
                onSelect={() => {
                  setModel({ providerId: m.providerId, model: m.id });
                  close();
                }}
              />
            );
          })}
          {!list && <p className="px-2.5 py-1.5 text-sm text-text-faint">Loading…</p>}
          <div className="mt-1 border-t border-border px-2.5 pb-1 pt-1.5">
            <Link to="/models" className="text-xs text-text-muted hover:text-text" onClick={close}>
              Manage models…
            </Link>
          </div>
        </>
      )}
    </Popover>
  );
}
