import {
  WispError,
  type Conversation,
  type ConversationSummary,
  type Message,
  type MessageDelta,
} from "@wisp/contracts";
import { create } from "zustand";
import { ipc } from "../services/ipc";

interface ConversationState {
  summaries: ConversationSummary[];
  conversations: Record<string, Conversation>;
  /** Conversation shown in the floating panel. */
  panelId: string | null;
  error: string | null;
  loadList(query?: string): Promise<void>;
  open(id: string): Promise<void>;
  send(
    text: string,
    opts: { conversationId?: string | null; includeFocus?: boolean },
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

const upsert = (messages: Message[], m: Message) => {
  const i = messages.findIndex((x) => x.id === m.id);
  const prev = messages[i];
  if (i === -1 || !prev) return [...messages, m].sort((a, b) => a.createdAtMs - b.createdAtMs);
  const next = messages.slice();
  // Keep streamed text if a status-only update arrives before the text was persisted.
  next[i] = {
    ...m,
    content: m.status === "streaming" && m.content.length < prev.content.length ? prev.content : m.content,
  };
  return next;
};

export const useConversationStore = create<ConversationState>()((set) => ({
  summaries: [],
  conversations: {},
  panelId: null,
  error: null,

  async loadList(query) {
    const r = await ipc().call("conversation.list", { limit: 200, ...(query ? { query } : {}) });
    set({ summaries: r.items });
  },

  async open(id) {
    const c = await ipc().call("conversation.get", { id });
    set((s) => ({ conversations: { ...s.conversations, [id]: c } }));
  },

  async send(text, { conversationId, includeFocus }) {
    set({ error: null });
    try {
      const r = await ipc().call("conversation.send", {
        text,
        includeFocus: !!includeFocus,
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
        return {
          conversations: {
            ...s.conversations,
            [r.conversationId]: {
              ...base,
              messages: upsert(upsert(base.messages, r.userMessage), r.assistantMessage),
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
      if (!c) return s;
      return { conversations: { ...s.conversations, [c.id]: { ...c, messages: upsert(c.messages, m) } } };
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
