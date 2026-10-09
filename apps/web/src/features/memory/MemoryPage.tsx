import type { MemoryCategory, MemoryItem, MemoryLevel } from "@wisp/contracts";
import { DeleteIcon, EditIcon, MemoryIcon } from "@wisp/icons";
import { Button, EmptyState, ErrorNotice, SettingsGroup, SettingsRow, Switch } from "@wisp/ui";
import { useEffect, useState } from "react";
import { useMemoryStore } from "../../stores/memoryStore";
import { useSettingsStore } from "../../stores/settingsStore";

const categories: Record<MemoryCategory, string> = {
  userProfile: "About you",
  preference: "Preferences",
  project: "Projects",
  fact: "Facts",
  conversation: "From conversations",
  agent: "Agent notes",
  skill: "Skill notes",
};
const levels: Record<MemoryLevel, string> = {
  persistent: "Kept",
  project: "Project",
  session: "Until restart",
  temporary: "For a day",
};
const sources: Record<MemoryItem["source"], string> = {
  user: "added by you",
  extracted: "you said this",
  agent: "saved by an agent",
};

const field = "rounded-[var(--radius-sm)] border border-border-strong bg-bg px-2 py-1.5 text-base";

function Editor({ item, onDone }: { item?: MemoryItem; onDone: () => void }) {
  const { save, error } = useMemoryStore();
  const [content, setContent] = useState(item?.content ?? "");
  const [category, setCategory] = useState<MemoryCategory>(item?.category ?? "fact");
  const [level, setLevel] = useState<MemoryLevel>(item?.level ?? "persistent");
  return (
    <form
      aria-label={item ? "Edit memory" : "New memory"}
      className="mb-4 space-y-2 rounded-[var(--radius-lg)] border border-border bg-surface p-3"
      onSubmit={(e) => {
        e.preventDefault();
        void save(content, category, level, item?.id).then((ok) => ok && onDone());
      }}
    >
      <textarea
        aria-label="What should Wisp remember?"
        autoFocus
        rows={2}
        maxLength={500}
        className={`${field} w-full`}
        placeholder="e.g. I prefer short answers"
        value={content}
        onChange={(e) => setContent(e.target.value)}
      />
      <div className="flex flex-wrap items-center gap-2">
        <select
          aria-label="Kind"
          className={field}
          value={category}
          onChange={(e) => setCategory(e.target.value as MemoryCategory)}
        >
          {(["userProfile", "preference", "project", "fact"] as const).map((c) => (
            <option key={c} value={c}>
              {categories[c]}
            </option>
          ))}
        </select>
        <select
          aria-label="Keep for"
          className={field}
          value={level}
          onChange={(e) => setLevel(e.target.value as MemoryLevel)}
        >
          {(["persistent", "session", "temporary"] as const).map((l) => (
            <option key={l} value={l}>
              {levels[l]}
            </option>
          ))}
        </select>
        <span className="ml-auto flex gap-2">
          <Button variant="ghost" onClick={onDone}>
            Cancel
          </Button>
          <Button type="submit" variant="primary">
            Save
          </Button>
        </span>
      </div>
      {error && <ErrorNotice title="That memory couldn't be saved." message={error} />}
    </form>
  );
}

/** Memory (spec §67): everything Wisp keeps about you, to inspect, edit and delete. */
export function MemoryPage() {
  const { items, limit, load, remove, clear } = useMemoryStore();
  const { settings, update } = useSettingsStore();
  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<MemoryItem | "new" | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  useEffect(() => {
    const t = setTimeout(() => void load(query.trim() || undefined).catch(() => {}), 150);
    return () => clearTimeout(t);
  }, [load, query]);
  const memory = settings?.memory;
  const grouped = (Object.keys(categories) as MemoryCategory[])
    .map((c) => [c, items.filter((m) => m.category === c)] as const)
    .filter(([, list]) => list.length > 0);

  return (
    <>
      <div className="mb-6 flex items-start gap-3">
        <div className="flex-1">
          <h1 className="text-xl font-semibold tracking-tight">Memory</h1>
          <p className="mt-1 text-md text-text-muted">
            What Wisp remembers about you. It only saves things you clearly tell it — like “remember that…” or
            “I prefer…” — and shows you when it does. Everything stays on this Mac.
          </p>
        </div>
        {!editing && (
          <Button variant="primary" onClick={() => setEditing("new")}>
            Add memory
          </Button>
        )}
      </div>

      {memory && (
        <div className="mb-6">
          <SettingsGroup title="How Wisp uses memory">
            <SettingsRow
              label="Use memories in answers"
              control={
                <Switch
                  label="Use memories in answers"
                  checked={memory.enabled}
                  onCheckedChange={(v) =>
                    void update({ section: "memory", value: { ...memory, enabled: v } })
                  }
                />
              }
            />
            <SettingsRow
              label="Remember what I ask it to"
              hint="“Remember that…”, “My name is…”, “I prefer…”, “From now on…”"
              control={
                <Switch
                  label="Remember what I ask it to"
                  checked={memory.learnFromChats}
                  onCheckedChange={(v) =>
                    void update({ section: "memory", value: { ...memory, learnFromChats: v } })
                  }
                />
              }
            />
          </SettingsGroup>
        </div>
      )}

      {editing && (
        <Editor {...(editing !== "new" ? { item: editing } : {})} onDone={() => setEditing(null)} />
      )}

      <div className="mb-3 flex items-center gap-2">
        <input
          aria-label="Search memories"
          placeholder="Search"
          className={`${field} h-8 flex-1`}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <span className="text-sm text-text-faint">
          {items.length} of {limit}
        </span>
        {items.length > 0 &&
          (confirmClear ? (
            <>
              <Button
                variant="danger"
                size="sm"
                onClick={() => void clear().then(() => setConfirmClear(false))}
              >
                Forget everything
              </Button>
              <Button variant="ghost" size="sm" onClick={() => setConfirmClear(false)}>
                Keep
              </Button>
            </>
          ) : (
            <Button variant="ghost" size="sm" onClick={() => setConfirmClear(true)}>
              Forget everything…
            </Button>
          ))}
      </div>

      {items.length === 0 ? (
        <EmptyState icon={<MemoryIcon size={28} />} title={query ? "No matches" : "Nothing remembered yet"}>
          Tell Wisp “remember that…” in a conversation, or add a memory here.
        </EmptyState>
      ) : (
        grouped.map(([cat, list]) => (
          <section key={cat} className="mb-5">
            <h2 className="mb-1.5 text-sm font-semibold text-text-muted">{categories[cat]}</h2>
            <ul className="divide-y divide-border rounded-[var(--radius-md)] border border-border bg-surface">
              {list.map((m) => (
                <li key={m.id} className="flex items-start gap-2 px-3 py-2.5">
                  <div className="min-w-0 flex-1">
                    <p className="selectable text-base">{m.content}</p>
                    <p className="mt-0.5 text-xs text-text-faint">
                      {levels[m.level]} · {sources[m.source]} · used {m.accessCount}×
                    </p>
                  </div>
                  <Button
                    size="sm"
                    variant="ghost"
                    aria-label={`Edit “${m.content}”`}
                    onClick={() => setEditing(m)}
                  >
                    <EditIcon size={14} />
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    aria-label={`Forget “${m.content}”`}
                    onClick={() => void remove(m.id)}
                  >
                    <DeleteIcon size={14} />
                  </Button>
                </li>
              ))}
            </ul>
          </section>
        ))
      )}
    </>
  );
}
