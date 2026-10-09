import type { ConversationSummary } from "@wisp/contracts";
import { ConversationsIcon } from "@wisp/icons";
import { Button, EmptyState, cn } from "@wisp/ui";
import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { Composer } from "../../components/chat/Composer";
import { MessageList } from "../../components/chat/MessageList";
import { streamingMessage, useConversationStore } from "../../stores/conversationStore";

function when(ms: number) {
  const d = new Date(ms);
  const today = new Date();
  return d.toDateString() === today.toDateString()
    ? d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })
    : d.toLocaleDateString([], { month: "short", day: "numeric" });
}

function Row({ s, active }: { s: ConversationSummary; active: boolean }) {
  return (
    <Link
      to={`/conversations/${s.id}`}
      className={cn(
        "block rounded-[var(--radius-sm)] px-2 py-1.5 hover:bg-surface-hover",
        active && "bg-surface-hover",
      )}
    >
      <div className="flex items-center gap-1.5">
        {s.pinned && (
          <span aria-label="Pinned" className="text-xs text-accent">
            ●
          </span>
        )}
        <span className="truncate text-base text-text">{s.title}</span>
        <span className="ml-auto shrink-0 text-xs text-text-faint">{when(s.updatedAtMs)}</span>
      </div>
      {s.preview && <p className="truncate text-sm text-text-muted">{s.preview}</p>}
    </Link>
  );
}

export function ConversationsPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const { summaries, conversations, loadList, open, send, cancel, rename, pin, remove } =
    useConversationStore();
  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const conversation = id ? conversations[id] : undefined;
  const summary = summaries.find((s) => s.id === id);
  const streaming = streamingMessage(conversation);

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

  return (
    <div className="-mx-8 -mb-12 -mt-10 flex h-[calc(100vh)] min-h-0">
      <aside className="flex w-64 shrink-0 flex-col border-r border-border pt-10">
        <div className="space-y-2 px-3 pb-2">
          <div className="flex items-center justify-between">
            <h1 className="text-lg font-semibold tracking-tight">Conversations</h1>
            <Button size="sm" onClick={() => navigate("/conversations")}>
              New
            </Button>
          </div>
          <input
            aria-label="Search conversations"
            placeholder="Search"
            className="h-7 w-full rounded-[var(--radius-sm)] border border-border-strong bg-surface px-2 text-base"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <nav aria-label="Conversations" className="min-h-0 flex-1 space-y-px overflow-y-auto px-2 pb-3">
          {summaries.length === 0 ? (
            <p className="px-2 py-4 text-sm text-text-faint">
              {query ? "No matches." : "No conversations yet."}
            </p>
          ) : (
            summaries.map((s) => <Row key={s.id} s={s} active={s.id === id} />)
          )}
        </nav>
      </aside>

      <section className="flex min-w-0 flex-1 flex-col pt-10">
        {id && conversation ? (
          <>
            <header className="flex items-center gap-2 border-b border-border px-6 pb-3">
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
                    className="h-7 w-full rounded-[var(--radius-sm)] border border-border-strong bg-surface px-2"
                    value={editing}
                    onChange={(e) => setEditing(e.target.value)}
                    onBlur={() => setEditing(null)}
                  />
                </form>
              ) : (
                <h2 className="flex-1 truncate text-md font-semibold">{conversation.title}</h2>
              )}
              <Button size="sm" variant="ghost" onClick={() => setEditing(conversation.title)}>
                Rename
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => void pin(id, !(summary?.pinned ?? conversation.pinned))}
              >
                {(summary?.pinned ?? conversation.pinned) ? "Unpin" : "Pin"}
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
            <div className="min-h-0 flex-1 overflow-y-auto px-6 py-4">
              <MessageList
                messages={conversation.messages}
                onRetry={(q) => void send(q, { conversationId: id })}
              />
            </div>
            <div className="px-6 pb-5">
              <Composer
                streaming={!!streaming}
                onStop={() => streaming && void cancel(streaming.id)}
                onSend={(t) => void send(t, { conversationId: id })}
              />
            </div>
          </>
        ) : (
          <div className="flex flex-1 flex-col justify-center px-10">
            <EmptyState icon={<ConversationsIcon size={28} />} title="Start a conversation">
              Ask anything — or open Wisp beside your cursor and point at something.
            </EmptyState>
            <div className="mx-auto w-full max-w-xl">
              <Composer
                autoFocus
                streaming={false}
                onSend={(t) => void send(t, {}).then((cid) => cid && navigate(`/conversations/${cid}`))}
              />
            </div>
          </div>
        )}
      </section>
    </div>
  );
}
