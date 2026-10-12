import { WispError, type McpServerStatus } from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

/** Live state of connected services (MCP servers). Configuration lives in settings.connections. */
interface ServiceState {
  status: Record<string, McpServerStatus>;
  error: string | null;
  load(): Promise<void>;
  receive(s: McpServerStatus): void;
  reconnect(id: string): Promise<void>;
}

export const useServiceStore = create<ServiceState>()((set) => ({
  status: {},
  error: null,
  async load() {
    try {
      const r = await ipc().call("mcp.status");
      set({ status: Object.fromEntries(r.servers.map((s) => [s.id, s])) });
    } catch {
      /* engine not ready yet; events will fill it in */
    }
  },
  receive: (s) => set((st) => ({ status: { ...st.status, [s.id]: s } })),
  async reconnect(id) {
    set({ error: null });
    try {
      const s = await ipc().call("mcp.reconnect", { id });
      set((st) => ({ status: { ...st.status, [s.id]: s } }));
    } catch (e) {
      set({ error: e instanceof WispError ? e.message : "Couldn't reconnect." });
    }
  },
}));
