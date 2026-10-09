import {
  WispError,
  type SpeechModelProgress,
  type VoiceFinishResult,
  type VoiceStatus,
} from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

interface VoiceState {
  status: VoiceStatus | null;
  level: number;
  listening: boolean;
  downloads: Record<string, SpeechModelProgress>;
  lastResult: VoiceFinishResult | null;
  error: string | null;
  load(): Promise<void>;
  setLevel(l: number): void;
  receiveProgress(p: SpeechModelProgress): void;
  download(id: string): Promise<void>;
  cancelDownload(id: string): Promise<void>;
  remove(id: string): Promise<void>;
  start(purpose: "ask" | "test"): Promise<void>;
  stop(): Promise<VoiceFinishResult | null>;
  speak(text: string): Promise<void>;
  stopSpeaking(): Promise<void>;
}

const message = (e: unknown) => (e instanceof WispError ? e.message : String(e));

export const useVoiceStore = create<VoiceState>()((set, get) => ({
  status: null,
  level: 0,
  listening: false,
  downloads: {},
  lastResult: null,
  error: null,

  async load() {
    try {
      set({ status: await ipc().call("voice.status") });
    } catch (e) {
      set({ error: message(e) });
    }
  },

  setLevel: (level) => set({ level }),

  receiveProgress(p) {
    set((s) => ({ downloads: { ...s.downloads, [p.id]: p } }));
    if (p.done && !p.error) void get().load();
  },

  async download(id) {
    set((s) => ({ downloads: { ...s.downloads, [id]: { id, completed: 0, total: 1, done: false } } }));
    await ipc().call("voice.models.download", { id });
  },

  async cancelDownload(id) {
    await ipc().call("voice.models.cancel", { id });
  },

  async remove(id) {
    await ipc().call("voice.models.remove", { id });
    await get().load();
  },

  async start(purpose) {
    set({ error: null, lastResult: null });
    try {
      await ipc().call("host.voice.start", { purpose });
      set({ listening: true });
    } catch (e) {
      set({ error: message(e), listening: false });
    }
  },

  async stop() {
    set({ listening: false });
    try {
      const r = await ipc().call("host.voice.stop", {}, { timeoutMs: 130_000 });
      set({ lastResult: r });
      return r;
    } catch (e) {
      set({ error: message(e) });
      return null;
    }
  },

  async speak(text) {
    set({ error: null });
    try {
      await ipc().call("voice.speak", { text });
    } catch (e) {
      set({ error: message(e) });
    }
  },

  async stopSpeaking() {
    await ipc().call("host.voice.stopSpeaking");
  },
}));
