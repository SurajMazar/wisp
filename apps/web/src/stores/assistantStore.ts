import type { AssistantState, InvocationBehavior, InvocationSource } from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

/** Mirror of the native assistant state (spec §92), driven by `assistant.stateChanged`. */
interface AssistantStore {
  state: AssistantState;
  expanded: boolean;
  source: InvocationSource | null;
  behavior: InvocationBehavior | null;
  receive(state: AssistantState, expanded: boolean): void;
  invoked(source: InvocationSource, behavior: InvocationBehavior): void;
  load(): Promise<void>;
  dismiss(): Promise<void>;
}

export const useAssistantStore = create<AssistantStore>()((set) => ({
  state: "idle",
  expanded: false,
  source: null,
  behavior: null,
  receive: (state, expanded) => set({ state, expanded }),
  invoked: (source, behavior) => set({ source, behavior }),
  async load() {
    try {
      const s = await ipc().call("host.assistant.state");
      set({ state: s.state, expanded: s.expanded });
    } catch {
      // Not hosted by the app (tests, demo); keep defaults.
    }
  },
  async dismiss() {
    await ipc().call("host.assistant.setVisible", { visible: false });
  },
}));
