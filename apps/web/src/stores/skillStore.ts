import { WispError, type Skill, type SkillSummary, type SkillVersion } from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

interface SkillState {
  skills: SkillSummary[];
  error: string | null;
  load(): Promise<void>;
  get(id: string): Promise<Skill>;
  save(content: string, id?: string): Promise<Skill | null>;
  importText(content: string): Promise<Skill | null>;
  setEnabled(id: string, enabled: boolean): Promise<void>;
  remove(id: string): Promise<void>;
  exportText(id: string): Promise<{ filename: string; content: string }>;
  versions(id: string): Promise<SkillVersion[]>;
  restore(id: string, version: number): Promise<Skill | null>;
}

const msg = (e: unknown, fallback: string) =>
  e instanceof WispError
    ? [e.message, e.details].filter((x) => x && x !== "The request was malformed.").join(" ") || e.message
    : fallback;

export const useSkillStore = create<SkillState>()((set, get) => ({
  skills: [],
  error: null,
  async load() {
    set({ skills: (await ipc().call("skill.list")).skills });
  },
  get: (id) => ipc().call("skill.get", { id }),
  async save(content, id) {
    set({ error: null });
    try {
      const s = await ipc().call("skill.save", { content, ...(id ? { id } : {}) });
      await get().load();
      return s;
    } catch (e) {
      set({ error: msg(e, "That skill couldn't be saved.") });
      return null;
    }
  },
  async importText(content) {
    set({ error: null });
    try {
      const s = await ipc().call("skill.import", { content });
      await get().load();
      return s;
    } catch (e) {
      set({ error: msg(e, "That file isn't a valid SKILL.md.") });
      return null;
    }
  },
  async setEnabled(id, enabled) {
    const s = await ipc().call("skill.setEnabled", { id, enabled });
    set((st) => ({ skills: st.skills.map((x) => (x.id === id ? s : x)) }));
  },
  async remove(id) {
    await ipc().call("skill.delete", { id });
    set((st) => ({ skills: st.skills.filter((x) => x.id !== id) }));
  },
  exportText: (id) => ipc().call("skill.export", { id }),
  async versions(id) {
    return (await ipc().call("skill.versions", { id })).versions;
  },
  async restore(id, version) {
    try {
      const s = await ipc().call("skill.restore", { id, version });
      await get().load();
      return s;
    } catch (e) {
      set({ error: msg(e, "That version couldn't be restored.") });
      return null;
    }
  },
}));
