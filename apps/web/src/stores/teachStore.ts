import { WispError, type Lesson, type LessonAction } from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

interface TeachState {
  lesson: Lesson | null;
  /** Lessons the user closed after they ended. */
  dismissed: string[];
  error: string | null;
  receive(l: Lesson): void;
  load(): Promise<void>;
  control(action: LessonAction): Promise<void>;
  dismiss(): void;
}

export const useTeachStore = create<TeachState>()((set, get) => ({
  lesson: null,
  dismissed: [],
  error: null,
  receive: (lesson) => set({ lesson }),
  async load() {
    const r = await ipc().call("teach.current");
    set({ lesson: r.lesson ?? null });
  },
  async control(action) {
    const l = get().lesson;
    if (!l) return;
    set({ error: null });
    try {
      await ipc().call("teach.control", { lessonId: l.id, action });
    } catch (e) {
      set({ error: e instanceof WispError ? e.message : "That lesson has ended." });
    }
  },
  dismiss: () => set((s) => ({ dismissed: s.lesson ? [...s.dismissed, s.lesson.id] : s.dismissed })),
}));

export const lessonActive = (l: Lesson | null) =>
  !!l && (l.status === "thinking" || l.status === "waitingForUser" || l.status === "stuck");
