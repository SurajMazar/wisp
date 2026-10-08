import type { InvocationBehavior, InvocationSource } from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

/** Assistant surface state (spec §92). More states arrive with voice, screen and AI phases. */
export type AssistantPhase = "idle" | "active";

interface AssistantState {
  phase: AssistantPhase;
  source: InvocationSource | null;
  behavior: InvocationBehavior | null;
  started(source: InvocationSource, behavior: InvocationBehavior): void;
  ended(): void;
  dismiss(): Promise<void>;
}

export const useAssistantStore = create<AssistantState>()((set) => ({
  phase: "idle",
  source: null,
  behavior: null,
  started: (source, behavior) => set({ phase: "active", source, behavior }),
  ended: () => set({ phase: "idle" }),
  async dismiss() {
    await ipc().call("host.assistant.setVisible", { visible: false });
  },
}));
