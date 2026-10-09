import { WispError, type MemoryCategory, type MemoryItem, type MemoryLevel } from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

interface MemoryState {
  items: MemoryItem[];
  limit: number;
  error: string | null;
  load(query?: string): Promise<void>;
  save(content: string, category: MemoryCategory, level: MemoryLevel, id?: string): Promise<boolean>;
  remove(id: string): Promise<void>;
  clear(): Promise<number>;
}

export const useMemoryStore = create<MemoryState>()((set, get) => ({
  items: [],
  limit: 500,
  error: null,
  async load(query) {
    const r = await ipc().call("memory.list", query ? { query } : {});
    set({ items: r.items, limit: r.limit });
  },
  async save(content, category, level, id) {
    set({ error: null });
    try {
      await ipc().call("memory.save", { content, category, level, ...(id ? { id } : {}) });
      await get().load();
      return true;
    } catch (e) {
      set({ error: e instanceof WispError ? (e.details ?? e.message) : "That memory couldn't be saved." });
      return false;
    }
  },
  async remove(id) {
    await ipc().call("memory.delete", { id });
    set((s) => ({ items: s.items.filter((m) => m.id !== id) }));
  },
  async clear() {
    const r = await ipc().call("memory.clear");
    set({ items: [] });
    return r.deleted;
  },
}));
