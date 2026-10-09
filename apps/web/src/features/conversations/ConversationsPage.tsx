import type { ConversationSummary } from "@wisp/contracts";
import { PlusIcon } from "@wisp/icons";
import { Button, cn } from "@wisp/ui";
import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { ChatThread } from "../../components/chat/ChatThread";
import { Composer, type SendMode } from "../../components/chat/Composer";
import { useComposerFocus } from "../../components/chat/useComposerFocus";
import { RunStack } from "../../components/agents/AgentRunCard";
import { LessonCard } from "../../components/teach/LessonCard";
import { dayGroup } from "../../lib/time";
import { streamingMessage, useConversationStore } from "../../stores/conversationStore";

function Row({ s, active }: { s: ConversationSummary; active: boolean }) {
  return (
    <Link
      to={`/conversations/${s.id}`}
      title={s.title}
      className={cn(
        "flex items-center gap-1.5 rounded-[var(--radius-md)] px-2.5 py-1.5 text-base text-text-muted hover:bg-surface-hover hover:text-text",
        active && "bg-surface-hover text-text",
      )}
    >
      {s.pinned && <span aria-label="Pinned" className="h-1.5 w-1.5 shrink-0 rounded-full bg-accent" />}
      <span className="truncate">{s.title}</span>
    </Link>
  );
}

function History({
  summaries,
  activeId,
  query,
}: {
  summaries: ConversationSummary[];
  activeId?: string;
  query: string;
}) {
  if (summaries.length === 0)
    return (
      <p className="px-2.5 py-4 text-sm text-text-faint">{query ? "No matches." : "No conversations yet."}</p>
    );
  const pinned = summaries.filter((s) => s.pinned);
  const groups = new Map<string, ConversationSummary[]>();
  for (const s of summaries.filter((x) => !x.pinned)) {
    const g = dayGroup(s.updatedAtMs);
    groups.set(g, [...(groups.get(g) ?? []), s]);
  }
  const sections = [...(pinned.length ? [["Pinned", pinned] as const] : []), ...groups.entries()];
  return (
    <>
      {sections.map(([label, items]) => (
        <div key={label} className="mb-3">
          <h3 className="px-2.5 pb-1 text-xs font-medium text-text-faint">{label}</h3>
          {items.map((s) => (
            <Row key={s.id} s={s} active={s.id === activeId} />
          ))}
        </div>
      ))}
    </>
  );
}

export function ConversationsPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const { summaries, conversations, loadList, open, send, cancel, rename, pin, remove, regenerate, error } =
    useConversationStore();
  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const conversation = id ? conversations[id] : undefined;
  const summary = summaries.find((s) => s.id === id);
  const streaming = streamingMessage(conversation);
  const { focus, includeFocus, consumed } = useComposerFocus();

  useEffect(() => {
    const t = setTimeout(() => void loadList(query), 150);
    return () => clearTimeout(t);
  }, [loadList, query]);

  // Reset the delete confirmation when switching conversations (render-time adjustment).
  const [shownId, setShownId] = useState(id);
  if (shownId !== id) {
    setShownId(id);
    setConfirmDelete(false);
  }

  useEffect(() => {
    if (id) void open(id).catch(() => navigate("/conversations", { replace: true }));
  }, [id, open, navigate]);

  const ask = (t: string, opts?: SendMode) =>
    void send(t, { conversationId: id ?? null, includeFocus, ...opts }).then((cid) => {
      if (!cid) return;
      consumed();
      if (cid !== id) navigate(`/conversations/${cid}`);
    });

  const pinned = summary?.pinned ?? conversation?.pinned ?? false;

  return (
    <div className="flex h-full min-h-0">
      <aside className="flex w-60 shrink-0 flex-col border-r border-border pt-10">
        <div className="space-y-2 px-2 pb-3">
          <button
            type="button"
            onClick={() => navigate("/")}
            className="flex w-full items-center gap-2 rounded-[var(--radius-md)] px-2.5 py-1.5 text-base font-medium hover:bg-surface-hover"
          >
            <PlusIcon size={15} /> New chat
          </button>
          <input
            aria-label="Search conversations"
            placeholder="Search chats"
            className="h-7 w-full rounded-[var(--radius-md)] border border-border bg-transparent px-2.5 text-base placeholder:text-text-faint focus:border-border-strong focus:outline-none"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <nav aria-label="Conversations" className="min-h-0 flex-1 overflow-y-auto px-2 pb-3">
          <History summaries={summaries} query={query} {...(id ? { activeId: id } : {})} />
        </nav>
      </aside>

      <section className="flex min-w-0 flex-1 flex-col pt-8">
        {id && conversation ? (
          <>
            <header className="flex h-10 items-center gap-1 px-5">
              {editing !== null ? (
                <form
                  className="flex-1"
                  onSubmit={(e) => {
                    e.preventDefault();
                    if (editing.trim()) void rename(id, editing.trim());
                    setEditing(null);
                  }}
                >
                  <input
                    aria-label="Conversation title"
                    autoFocus
                    className="h-7 w-full max-w-md rounded-[var(--radius-sm)] border border-border-strong bg-surface px-2"
                    value={editing}
                    onChange={(e) => setEditing(e.target.value)}
                    onBlur={() => setEditing(null)}
                  />
                </form>
              ) : (
                <h1 className="min-w-0 flex-1 truncate">
                  <button
                    type="button"
                    title="Rename"
                    onClick={() => setEditing(conversation.title)}
                    className="max-w-full truncate text-left text-base font-medium text-text-muted hover:text-text"
                  >
                    {conversation.title}
                  </button>
                </h1>
              )}
              <Button size="sm" variant="ghost" onClick={() => void pin(id, !pinned)}>
                {pinned ? "Unpin" : "Pin"}
              </Button>
              {confirmDelete ? (
                <>
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() => void remove(id).then(() => navigate("/conversations"))}
                  >
                    Delete
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => setConfirmDelete(false)}>
                    Keep
                  </Button>
                </>
              ) : (
                <Button size="sm" variant="ghost" onClick={() => setConfirmDelete(true)}>
                  Delete…
                </Button>
              )}
            </header>
            <ChatThread
              messages={conversation.messages}
              onRetry={(q) => ask(q)}
              onRegenerate={(mid) => void regenerate(mid)}
            />
            <div className="mx-auto w-full max-w-[720px] px-6 pb-5 pt-1">
              <div className="mb-2 empty:hidden">
                <LessonCard conversationId={id} />
              </div>
              <div className="mb-2 empty:hidden">
                <RunStack conversationId={id} />
              </div>
              {error && (
                <p role="alert" className="mb-1.5 px-2 text-sm text-danger">
                  {error}
                </p>
              )}
              <Composer
                autoFocus
                focus={focus}
                streaming={!!streaming}
                onStop={() => streaming && void cancel(streaming.id)}
                onSend={ask}
                placeholder={includeFocus ? "Ask about what you selected" : "Reply to Wisp"}
              />
              <p className="mt-1.5 text-center text-xs text-text-faint">
                Runs on this Mac by default. Answers can be wrong — check anything important.
              </p>
            </div>
          </>
        ) : (
          <div className="flex flex-1 flex-col items-center justify-center px-8 pb-[12vh]">
            <div className="w-full max-w-[640px]">
              <h2 className="mb-5 text-center text-xl font-medium tracking-tight">Start a new chat</h2>
              <Composer autoFocus focus={focus} streaming={false} onSend={ask} />
            </div>
          </div>
        )}
      </section>
    </div>
  );
}
