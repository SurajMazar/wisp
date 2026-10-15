import { WispError, type AppSettings, type SettingsSection } from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

type Status = "idle" | "loading" | "ready" | "error";
type Section = SettingsSection["section"];
type SectionValue<K extends Section> = Extract<SettingsSection, { section: K }>["value"];
/** The fields to change, or a function that works them out from the latest saved values. */
export type SettingsPatch<K extends Section> =
  Partial<SectionValue<K>> | ((current: SectionValue<K>) => Partial<SectionValue<K>>);

interface SettingsState {
  settings: AppSettings | null;
  status: Status;
  error: WispError | null;
  /** Section currently being saved, for per-control pending state. */
  saving: SettingsSection["section"] | null;
  load(): Promise<void>;
  update(update: SettingsSection): Promise<void>;
  /**
   * Change some fields of a section on top of the latest saved settings. Prefer this to
   * `update`: a section saved from an older copy (another window, a draft) would undo changes
   * made since, such as a cloud key added on the Models page.
   */
  patch<K extends Section>(section: K, change: SettingsPatch<K>): Promise<void>;
  /** Apply settings pushed by the core (`settings.changed`). */
  receive(settings: AppSettings): void;
}

const toError = (e: unknown) =>
  e instanceof WispError
    ? e
    : new WispError({ code: "internal", message: "Something went wrong.", details: String(e) });

export const useSettingsStore = create<SettingsState>()((set, get) => ({
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

  async patch(section, change) {
    let latest: AppSettings;
    try {
      latest = await ipc().call("settings.get");
    } catch (e) {
      set({ error: toError(e) });
      return;
    }
    // The signature ties `change` to `section`; inside, the values are plain objects.
    const current = (latest as Record<Section, object>)[section];
    const delta = typeof change === "function" ? (change as (c: object) => object)(current) : change;
    await get().update({ section, value: { ...current, ...delta } } as SettingsSection);
  },

  receive(settings) {
    set({ settings, status: "ready" });
  },
}));
