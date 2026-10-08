import { WispError, type AppSettings, type SettingsSection } from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

type Status = "idle" | "loading" | "ready" | "error";

interface SettingsState {
  settings: AppSettings | null;
  status: Status;
  error: WispError | null;
  /** Section currently being saved, for per-control pending state. */
  saving: SettingsSection["section"] | null;
  load(): Promise<void>;
  update(update: SettingsSection): Promise<void>;
  /** Apply settings pushed by the core (`settings.changed`). */
  receive(settings: AppSettings): void;
}

const toError = (e: unknown) =>
  e instanceof WispError
    ? e
    : new WispError({ code: "internal", message: "Something went wrong.", details: String(e) });

export const useSettingsStore = create<SettingsState>()((set) => ({
  settings: null,
  status: "idle",
  error: null,
  saving: null,

  async load() {
    set({ status: "loading", error: null });
    try {
      const settings = await ipc().call("settings.get");
      set({ settings, status: "ready" });
    } catch (e) {
      set({ status: "error", error: toError(e) });
    }
  },

  async update(update) {
    set({ saving: update.section, error: null });
    try {
      const settings = await ipc().call("settings.update", { update });
      set({ settings, saving: null });
    } catch (e) {
      set({ saving: null, error: toError(e) });
    }
  },

  receive(settings) {
    set({ settings, status: "ready" });
  },
}));
