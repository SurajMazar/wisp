import {
  WispError,
  type ApprovalDecision,
  type ApprovalRequest,
  type ToolCall,
  type ToolCallRecord,
  type ToolGrants,
  type ToolRunResult,
} from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

interface ToolState {
  /** Approval requests waiting for an answer, oldest first. */
  pending: ApprovalRequest[];
  activity: ToolCallRecord[];
  grants: ToolGrants | null;
  error: string | null;
  receiveRequest(r: ApprovalRequest): void;
  receiveResolved(id: string): void;
  receiveCall(r: ToolCallRecord): void;
  respond(id: string, decision: ApprovalDecision): Promise<void>;
  run(call: ToolCall, messageId?: string): Promise<ToolRunResult | null>;
  loadActivity(): Promise<void>;
  loadGrants(): Promise<void>;
  revoke(id: string): Promise<void>;
}

const msg = (e: unknown, fallback: string) => (e instanceof WispError ? e.message : fallback);

export const useToolStore = create<ToolState>()((set, get) => ({
  pending: [],
  activity: [],
  grants: null,
  error: null,

  receiveRequest: (r) => set((s) => ({ pending: [...s.pending.filter((p) => p.id !== r.id), r] })),
  receiveResolved: (id) => set((s) => ({ pending: s.pending.filter((p) => p.id !== id) })),
  receiveCall: (r) =>
    set((s) => ({ activity: [r, ...s.activity.filter((a) => a.id !== r.id)].slice(0, 200) })),

  async respond(id, decision) {
    get().receiveResolved(id);
    try {
      await ipc().call("tool.respond", { id, decision });
      if (decision === "alwaysAllow" || decision === "allowSession") void get().loadGrants();
    } catch (e) {
      set({ error: msg(e, "That request already ended.") });
    }
  },

  async run(call, messageId) {
    set({ error: null });
    try {
      return await ipc().call("tool.run", { call, ...(messageId ? { messageId } : {}) });
    } catch (e) {
      set({ error: msg(e, "Wisp couldn't do that.") });
      return null;
    }
  },

  async loadActivity() {
    const r = await ipc().call("tool.activity", { limit: 200 });
    set({ activity: r.items });
  },

  async loadGrants() {
    set({ grants: await ipc().call("tool.grants") });
  },

  async revoke(id) {
    set({ grants: await ipc().call("tool.revoke", { id }) });
  },
}));
