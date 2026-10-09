import { WispError, type Routine, type RoutineRun, type RoutineSaveParams } from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

interface RoutineState {
  routines: Routine[];
  runs: Record<string, RoutineRun[]>;
  error: string | null;
  load(): Promise<void>;
  save(p: RoutineSaveParams): Promise<boolean>;
  remove(id: string): Promise<void>;
  runNow(id: string): Promise<void>;
  loadRuns(id: string): Promise<void>;
  receiveRun(r: RoutineRun): void;
}

export const useRoutineStore = create<RoutineState>()((set, get) => ({
  routines: [],
  runs: {},
  error: null,
  async load() {
    set({ routines: (await ipc().call("routine.list")).routines });
  },
  async save(p) {
    set({ error: null });
    try {
      await ipc().call("routine.save", p);
      await get().load();
      return true;
    } catch (e) {
      set({ error: e instanceof WispError ? (e.details ?? e.message) : "That routine couldn't be saved." });
      return false;
    }
  },
  async remove(id) {
    await ipc().call("routine.delete", { id });
    set((s) => ({ routines: s.routines.filter((r) => r.id !== id) }));
  },
  async runNow(id) {
    const r = await ipc().call("routine.runNow", { id });
    get().receiveRun(r);
  },
  async loadRuns(id) {
    const r = await ipc().call("routine.runs", { routineId: id, limit: 20 });
    set((s) => ({ runs: { ...s.runs, [id]: r.runs } }));
  },
  receiveRun: (r) =>
    set((s) => {
      const list = s.runs[r.routineId] ?? [];
      return { runs: { ...s.runs, [r.routineId]: [r, ...list.filter((x) => x.id !== r.id)].slice(0, 20) } };
    }),
}));
