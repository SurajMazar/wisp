import { WispError, type FileHit, type FileItem } from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

interface FileState {
  items: FileItem[];
  hits: FileHit[];
  error: string | null;
  load(): Promise<void>;
  pick(): Promise<void>;
  drop(files: File[]): Promise<void>;
  remove(id: string): Promise<void>;
  search(query: string): Promise<void>;
  open(id: string): Promise<void>;
  receive(f: FileItem): void;
}

const msg = (e: unknown, fallback: string) => (e instanceof WispError ? e.message : fallback);

function toBase64(buf: ArrayBuffer): string {
  const bytes = new Uint8Array(buf);
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(bin);
}

export const useFileStore = create<FileState>()((set, get) => ({
  items: [],
  hits: [],
  error: null,
  async load() {
    set({ items: (await ipc().call("file.list")).items });
  },
  async pick() {
    set({ error: null });
    try {
      await ipc().call("file.import");
      await get().load();
    } catch (e) {
      set({ error: msg(e, "Those files couldn't be added.") });
    }
  },
  async drop(files) {
    set({ error: null });
    for (const f of files) {
      if (f.size > 20 * 1024 * 1024) {
        set({ error: `“${f.name}” is larger than 20 MB — use Import… for big files.` });
        continue;
      }
      try {
        const item = await ipc().call("file.importData", {
          name: f.name,
          dataBase64: toBase64(await f.arrayBuffer()),
        });
        get().receive(item);
      } catch (e) {
        set({ error: msg(e, `“${f.name}” couldn't be added.`) });
      }
    }
  },
  async remove(id) {
    await ipc().call("file.delete", { id });
    set((s) => ({ items: s.items.filter((f) => f.id !== id), hits: s.hits.filter((h) => h.fileId !== id) }));
  },
  async search(query) {
    if (!query.trim()) return set({ hits: [] });
    set({ hits: (await ipc().call("file.search", { query, limit: 8 })).hits });
  },
  async open(id) {
    try {
      await ipc().call("file.open", { id });
    } catch (e) {
      set({ error: msg(e, "That file couldn't be opened.") });
    }
  },
  receive: (f) => set((s) => ({ items: [f, ...s.items.filter((x) => x.id !== f.id)] })),
}));
