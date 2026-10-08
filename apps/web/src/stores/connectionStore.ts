import type { AppInfo } from "@wisp/contracts";
import { create } from "zustand";

type Connection = "connecting" | "connected" | "unavailable";

interface ConnectionState {
  status: Connection;
  info: AppInfo | null;
  setConnected(info: AppInfo): void;
  setUnavailable(): void;
}

/** Whether the UI is talking to a live Wisp core. */
export const useConnectionStore = create<ConnectionState>()((set) => ({
  status: "connecting",
  info: null,
  setConnected: (info) => set({ status: "connected", info }),
  setUnavailable: () => set({ status: "unavailable" }),
}));
