import {
  WispError,
  type Conversation,
  type ConversationSummary,
  type Message,
  type MessageDelta,
  type MessageStage,
  type ModelRef,
} from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

interface ConversationState {
  summaries: ConversationSummary[];
  conversations: Record<string, Conversation>;
  /** Conversation shown in the floating panel. */
  panelId: string | null;
  error: string | null;
  /** Stages each streaming answer went through, with when they started (for the progress UI). */
  progress: Record<string, StageStep[]>;
  receiveReasoning(d: MessageDelta): void;
  /** Model chosen in the composer; null = automatic routing. */
  model: ModelRef | null;
  setModel(m: ModelRef | null): void;
  regenerate(messageId: string): Promise<void>;
  receiveRemoved(m: Message): void;
  loadList(query?: string): Promise<void>;
  open(id: string): Promise<void>;
  send(
    text: string,
    opts: { conversationId?: string | null; includeFocus?: boolean; teach?: boolean; act?: boolean },
  ): Promise<string | null>;
  cancel(messageId: string): Promise<void>;
  rename(id: string, title: string): Promise<void>;
  pin(id: string, pinned: boolean): Promise<void>;
  remove(id: string): Promise<void>;
  setPanel(id: string | null): void;
  receiveDelta(d: MessageDelta): void;
  receiveMessage(m: Message): void;
  receiveSummary(s: ConversationSummary): void;
}

export interface StageStep {
  stage: MessageStage;
  at: number;
}

const upsert = (messages: Message[], m: Message) => {
  const i = messages.findIndex((x) => x.id === m.id);
  const prev = messages[i];
  if (i === -1 || !prev) return [...messages, m].sort((a, b) => a.createdAtMs - b.createdAtMs);
  const next = messages.slice();
  // Keep streamed text if a status-only update arrives before the text was persisted.
  const streaming = m.status === "streaming";
  const longer = (a?: string | null, b?: string | null) => ((a?.length ?? 0) < (b?.length ?? 0) ? b : a);
  next[i] = {
    ...m,
    content: streaming ? (longer(m.content, prev.content) ?? "") : m.content,
    ...(streaming ? { reasoning: longer(m.reasoning, prev.reasoning) ?? null } : {}),
  };
  return next;
};

/** Updates that raced ahead of the `conversation.send` response (the conversation isn't in the
 *  store yet). Applied when the response lands so no progress is lost. */
const early = new Map<string, Message>();

/** Append the message's stage to its history when it changes. */
const track = (progress: Record<string, StageStep[]>, m: Message) => {
  if (!m.stage) return progress;
  const steps = progress[m.id] ?? [];
  if (steps.at(-1)?.stage === m.stage) return progress;
  return { ...progress, [m.id]: [...steps, { stage: m.stage, at: Date.now() }] };
};

export const useConversationStore = create<ConversationState>()((set, get) => ({
  summaries: [],
  conversations: {},
  panelId: null,
  error: null,
  progress: {},
  model: null,
  setModel: (model) => set({ model }),

  async regenerate(messageId) {
    set({ error: null });
    try {
      const model = get().model;
      await ipc().call("conversation.regenerate", { messageId, ...(model ? { model } : {}) });
    } catch (e) {
      set({ error: e instanceof WispError ? e.message : "Couldn't regenerate that." });
    }
  },

  receiveRemoved(m) {
    set((s) => {
      const c = s.conversations[m.conversationId];
      if (!c) return s;
      return {
        conversations: {
          ...s.conversations,
          [c.id]: { ...c, messages: c.messages.filter((x) => x.id !== m.id) },
        },
      };
    });
  },

  async loadList(query) {
    const r = await ipc().call("conversation.list", { limit: 200, ...(query ? { query } : {}) });
    set({ summaries: r.items });
  },

  async open(id) {
    const c = await ipc().call("conversation.get", { id });
    set((s) => {
      // Keep live state (stage, streamed text) of answers still streaming; the stored copy lags.
      const live = s.conversations[id]?.messages ?? [];
      const messages = c.messages.map((m) =>
        m.status === "streaming" ? (live.find((x) => x.id === m.id) ?? m) : m,
      );
      return { conversations: { ...s.conversations, [id]: { ...c, messages } } };
    });
  },

  async send(text, { conversationId, includeFocus, teach, act }) {
    set({ error: null });
    try {
      const model = get().model;
      const r = await ipc().call("conversation.send", {
        text,
        includeFocus: !!includeFocus,
        teach: !!teach,
        act: !!act,
        ...(model ? { model } : {}),
        ...(conversationId ? { conversationId } : {}),
      });
      set((s) => {
        const existing = s.conversations[r.conversationId];
        const now = Date.now();
        const base: Conversation = existing ?? {
          id: r.conversationId,
          title: text.slice(0, 60),
          pinned: false,
          messages: [],
          createdAtMs: now,
          updatedAtMs: now,
        };
        const raced = early.get(r.assistantMessage.id);
        early.delete(r.assistantMessage.id);
        const known = base.messages.find((x) => x.id === r.assistantMessage.id);
        const latest = known ?? raced ?? r.assistantMessage;
        return {
          progress: s.progress[r.assistantMessage.id] ? s.progress : track(s.progress, r.assistantMessage),
          conversations: {
            ...s.conversations,
            [r.conversationId]: {
              ...base,
              messages: upsert(upsert(base.messages, r.userMessage), latest),
            },
          },
        };
      });
      return r.conversationId;
    } catch (e) {
      set({ error: e instanceof WispError ? e.message : "Couldn't send that." });
      return null;
    }
  },

  async cancel(messageId) {
    await ipc().call("conversation.cancel", { id: messageId });
  },

  async rename(id, title) {
    await ipc().call("conversation.rename", { id, title });
  },

  async pin(id, pinned) {
    await ipc().call("conversation.pin", { id, pinned });
  },

  async remove(id) {
    await ipc().call("conversation.delete", { id });
    set((s) => {
      const conversations = Object.fromEntries(Object.entries(s.conversations).filter(([k]) => k !== id));
      return {
        conversations,
        summaries: s.summaries.filter((x) => x.id !== id),
        panelId: s.panelId === id ? null : s.panelId,
      };
    });
  },

  setPanel: (id) => set({ panelId: id }),

  receiveReasoning(d) {
    set((s) => {
      const c = s.conversations[d.conversationId];
      if (!c) return s;
      const messages = c.messages.map((m) =>
        m.id === d.messageId ? { ...m, reasoning: (m.reasoning ?? "") + d.delta } : m,
      );
      return { conversations: { ...s.conversations, [c.id]: { ...c, messages } } };
    });
  },

  receiveDelta(d) {
    set((s) => {
      const c = s.conversations[d.conversationId];
      if (!c) return s;
      const messages = c.messages.map((m) =>
        m.id === d.messageId ? { ...m, content: m.content + d.delta } : m,
      );
      return { conversations: { ...s.conversations, [c.id]: { ...c, messages } } };
    });
  },

  receiveMessage(m) {
    set((s) => {
      const c = s.conversations[m.conversationId];
      if (!c) {
        if (m.status === "streaming") early.set(m.id, m);
        else early.delete(m.id);
        return { progress: track(s.progress, m) };
      }
      return {
        progress: track(s.progress, m),
        conversations: { ...s.conversations, [c.id]: { ...c, messages: upsert(c.messages, m) } },
      };
    });
  },

  receiveSummary(sum) {
    set((s) => {
      const summaries = [sum, ...s.summaries.filter((x) => x.id !== sum.id)].sort(
        (a, b) => Number(b.pinned) - Number(a.pinned) || b.updatedAtMs - a.updatedAtMs,
      );
      const c = s.conversations[sum.id];
      return {
        summaries,
        conversations: c
          ? { ...s.conversations, [sum.id]: { ...c, title: sum.title, pinned: sum.pinned } }
          : s.conversations,
      };
    });
  },
}));

/** True while any message in the conversation is still streaming. */
export const streamingMessage = (c: Conversation | undefined) =>
  c?.messages.find((m) => m.status === "streaming");
