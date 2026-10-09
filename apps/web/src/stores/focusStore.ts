import {
  WispError,
  type FocusEnded,
  type FocusKind,
  type FocusRegion,
  type PermissionState,
} from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

interface FocusState {
  region: FocusRegion | null;
  lastEnded: FocusEnded | null;
  permission: PermissionState | null;
  error: string | null;
  receive(region: FocusRegion): void;
  ended(e: FocusEnded): void;
  cleared(): void;
  load(): Promise<void>;
  start(kind: FocusKind): Promise<void>;
  clear(): Promise<void>;
  requestPermission(): Promise<void>;
}

export const useFocusStore = create<FocusState>()((set, get) => ({
  region: null,
  lastEnded: null,
  permission: null,
  error: null,

  receive: (region) => set({ region, lastEnded: null, error: null, permission: "enabled" }),
  ended: (e) =>
    set({ lastEnded: e, permission: e.reason === "permissionDenied" ? "needsAttention" : get().permission }),
  cleared: () => set({ region: null }),

  async load() {
    try {
      const [latest, perm] = await Promise.all([
        ipc().call("focus.latest"),
        ipc().call("host.screen.permission"),
      ]);
      set({ region: latest.region ?? null, permission: perm.screenRecording });
    } catch {
      // Outside the app (tests, demo): nothing to load.
    }
  },

  async start(kind) {
    set({ error: null, lastEnded: null });
    try {
      await ipc().call("host.focus.start", { kind });
    } catch (e) {
      set({ error: e instanceof WispError ? e.message : "Couldn't start focusing." });
    }
  },

  async clear() {
    await ipc().call("host.focus.clear");
  },

  async requestPermission() {
    const r = await ipc().call("host.screen.requestPermission");
    set({ permission: r.screenRecording });
    if (r.screenRecording !== "enabled")
      await ipc().call("host.system.openSettingsPane", { pane: "screenRecording" });
  },
}));
