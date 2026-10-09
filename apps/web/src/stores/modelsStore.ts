import {
  WispError,
  type ModelRef,
  type ModelTestResult,
  type ModelsList,
  type PullProgress,
  type RouteInfo,
} from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

const key = (r: ModelRef) => `${r.providerId}/${r.model}`;

interface ModelsState {
  list: ModelsList | null;
  loading: boolean;
  error: WispError | null;
  pulls: Record<string, PullProgress>;
  tests: Record<string, ModelTestResult | { error: string } | "running">;
  routes: { text: RouteInfo | WispError | null; vision: RouteInfo | WispError | null };
  load(): Promise<void>;
  refreshRoutes(): Promise<void>;
  pull(name: string): Promise<void>;
  cancelPull(name: string): Promise<void>;
  remove(name: string): Promise<void>;
  test(model: ModelRef): Promise<void>;
  receivePull(p: PullProgress): void;
  startOllama(): Promise<void>;
}

const asError = (e: unknown) =>
  e instanceof WispError ? e : new WispError({ code: "internal", message: String(e) });

export const useModelsStore = create<ModelsState>()((set, get) => ({
  list: null,
  loading: false,
  error: null,
  pulls: {},
  tests: {},
  routes: { text: null, vision: null },

  async load() {
    set({ loading: true, error: null });
    try {
      const list = await ipc().call("models.list", {}, { timeoutMs: 20_000 });
      set({ list, loading: false });
      void get().refreshRoutes();
    } catch (e) {
      set({ loading: false, error: asError(e) });
    }
  },

  async refreshRoutes() {
    const route = (needsVision: boolean) =>
      ipc()
        .call("ai.route", { needsVision }, { timeoutMs: 20_000 })
        .catch((e: unknown) => asError(e));
    const [text, vision] = await Promise.all([route(false), route(true)]);
    set({ routes: { text, vision } });
  },

  async pull(name) {
    set((s) => ({ pulls: { ...s.pulls, [name]: { name, status: "starting", done: false } } }));
    try {
      await ipc().call("models.pull", { name });
    } catch (e) {
      set((s) => ({
        pulls: { ...s.pulls, [name]: { name, status: "error", done: true, error: asError(e).message } },
      }));
    }
  },

  async cancelPull(name) {
    await ipc().call("models.cancelPull", { name });
  },

  async remove(name) {
    await ipc().call("models.delete", { name });
    await get().load();
  },

  async test(model) {
    set((s) => ({ tests: { ...s.tests, [key(model)]: "running" } }));
    try {
      const r = await ipc().call("models.test", { model }, { timeoutMs: 180_000 });
      set((s) => ({ tests: { ...s.tests, [key(model)]: r } }));
    } catch (e) {
      set((s) => ({ tests: { ...s.tests, [key(model)]: { error: asError(e).message } } }));
    }
  },

  receivePull(p) {
    set((s) => ({ pulls: { ...s.pulls, [p.name]: p } }));
    if (p.done && p.status === "success") void get().load();
  },

  async startOllama() {
    await ipc().call("host.ollama.start");
    setTimeout(() => void get().load(), 2500);
  },
}));

export { key as modelKey };
