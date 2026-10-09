import {
  WispError,
  type AgentDefinition,
  type AgentRun,
  type AgentSaveParams,
  type RunAction,
} from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

interface AgentState {
  agents: AgentDefinition[];
  /** Runs by id, live and recent. */
  runs: Record<string, AgentRun>;
  dismissed: string[];
  error: string | null;
  load(): Promise<void>;
  loadRuns(agentId?: string): Promise<void>;
  save(p: AgentSaveParams): Promise<AgentDefinition | null>;
  remove(id: string): Promise<void>;
  run(goal: string, agentId?: string): Promise<AgentRun | null>;
  control(runId: string, action: RunAction, reply?: string): Promise<void>;
  receiveRun(r: AgentRun): void;
  dismiss(runId: string): void;
}

const msg = (e: unknown, fallback: string) => (e instanceof WispError ? e.message : fallback);

export const useAgentStore = create<AgentState>()((set) => ({
  agents: [],
  runs: {},
  dismissed: [],
  error: null,
  async load() {
    const r = await ipc().call("agent.list");
    set({ agents: r.agents });
  },
  async loadRuns(agentId) {
    const r = await ipc().call("agent.runs", { limit: 50, ...(agentId ? { agentId } : {}) });
    set((s) => ({ runs: { ...Object.fromEntries(r.runs.map((x) => [x.id, x])), ...s.runs } }));
  },
  async save(p) {
    set({ error: null });
    try {
      const a = await ipc().call("agent.save", p);
      set((s) => ({
        agents: [...s.agents.filter((x) => x.id !== a.id), a].sort((x, y) => x.name.localeCompare(y.name)),
      }));
      return a;
    } catch (e) {
      set({ error: msg(e, "That agent couldn't be saved.") });
      return null;
    }
  },
  async remove(id) {
    await ipc().call("agent.delete", { id });
    set((s) => ({ agents: s.agents.filter((a) => a.id !== id) }));
  },
  async run(goal, agentId) {
    set({ error: null });
    try {
      const r = await ipc().call("agent.run", { goal, ...(agentId ? { agentId } : {}) });
      set((s) => ({ runs: { ...s.runs, [r.id]: r } }));
      return r;
    } catch (e) {
      set({ error: msg(e, "That couldn't start.") });
      return null;
    }
  },
  async control(runId, action, reply) {
    try {
      await ipc().call("agent.control", { runId, action, ...(reply ? { reply } : {}) });
    } catch (e) {
      set({ error: msg(e, "That run has ended.") });
    }
  },
  receiveRun: (r) => set((s) => ({ runs: { ...s.runs, [r.id]: r } })),
  dismiss: (id) => set((s) => ({ dismissed: [...s.dismissed, id] })),
}));

export const runActive = (r: AgentRun) =>
  ["planning", "running", "paused", "waitingPermission", "stuck", "needsInput"].includes(r.status);
