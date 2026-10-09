import { useState } from "react";
import { useFocusStore } from "../../stores/focusStore";

/** The latest focus selection, attachable once: a new selection re-attaches automatically. */
export function useComposerFocus() {
  const region = useFocusStore((s) => s.region);
  const [attached, setAttached] = useState(true);
  const [seen, setSeen] = useState(region?.id);
  if (region?.id !== seen) {
    setSeen(region?.id);
    setAttached(true);
  }
  return {
    focus: region ? { region, attached, setAttached } : null,
    includeFocus: !!region && attached,
    consumed: () => setAttached(false),
  };
}
