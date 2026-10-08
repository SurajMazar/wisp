import { WispError, type InvocationStatus, type ShortcutConflict } from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";
import { useSettingsStore } from "./settingsStore";

interface InvocationState {
  status: InvocationStatus | null;
  recording: boolean;
  /** Conflicts for the shortcut most recently recorded. */
  conflicts: ShortcutConflict[];
  message: string | null;
  loadStatus(): Promise<void>;
  receiveStatus(s: InvocationStatus): void;
  record(): Promise<void>;
  openAccessibility(): Promise<void>;
}

export const useInvocationStore = create<InvocationState>()((set, get) => ({
  status: null,
  recording: false,
  conflicts: [],
  message: null,

  async loadStatus() {
    try {
      set({ status: await ipc().call("host.invocation.status") });
    } catch {
      set({ status: null });
    }
  },

  receiveStatus(status) {
    set({ status });
  },

  async record() {
    if (get().recording) return;
    set({ recording: true, message: null, conflicts: [] });
    try {
      const { shortcut, conflicts } = await ipc().call(
        "host.shortcut.record",
        { timeoutMs: 10_000 },
        { timeoutMs: 15_000 },
      );
      set({ conflicts });
      if (conflicts.some((c) => c.severity === "blocking")) {
        set({ message: "That shortcut is already used by macOS, so it wasn't saved." });
        return;
      }
      const current = useSettingsStore.getState().settings?.invocation;
      if (current)
        await useSettingsStore.getState().update({ section: "invocation", value: { ...current, shortcut } });
    } catch (e) {
      if (e instanceof WispError && (e.code === "cancelled" || e.code === "timeout")) {
        set({ message: e.code === "timeout" ? "No shortcut was pressed." : null });
      } else {
        set({ message: e instanceof WispError ? e.message : "Recording failed." });
      }
    } finally {
      set({ recording: false });
      void get().loadStatus();
    }
  },

  async openAccessibility() {
    await ipc().call("host.system.openSettingsPane", { pane: "accessibility" });
  },
}));
