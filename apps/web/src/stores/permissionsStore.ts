import type { PermissionState } from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

export type PermissionKind = "microphone" | "screenRecording" | "accessibility";

interface PermissionsState {
  states: Partial<Record<PermissionKind, PermissionState>>;
  checking: boolean;
  check(): Promise<void>;
  request(kind: PermissionKind): Promise<void>;
}

/** Live TCC status as macOS reports it — never cached guesses (spec §24). */
export const usePermissionsStore = create<PermissionsState>()((set, get) => ({
  states: {},
  checking: false,

  async check() {
    set({ checking: true });
    const c = ipc();
    const settle = async <T>(p: Promise<T>) => p.then((v) => v).catch(() => null);
    const [mic, screen, ax] = await Promise.all([
      settle(c.call("host.mic.permission")),
      settle(c.call("host.screen.permission")),
      settle(c.call("host.ax.status")),
    ]);
    set({
      checking: false,
      states: {
        ...(mic && { microphone: mic.state }),
        ...(screen && { screenRecording: screen.screenRecording }),
        ...(ax && { accessibility: ax.state }),
      },
    });
  },

  async request(kind) {
    const c = ipc();
    if (kind === "microphone") await c.call("host.mic.requestPermission");
    if (kind === "screenRecording") {
      const r = await c.call("host.screen.requestPermission");
      if (r.screenRecording !== "enabled")
        await c.call("host.system.openSettingsPane", { pane: "screenRecording" });
    }
    if (kind === "accessibility") await c.call("host.ax.requestPermission");
    await get().check();
  },
}));
