import type { SkillSummary, SkillVersion } from "@wisp/contracts";
import { DeleteIcon, EditIcon, SkillsIcon } from "@wisp/icons";
import { Button, EmptyState, ErrorNotice, Switch } from "@wisp/ui";
import { useEffect, useRef, useState } from "react";
import { copyText } from "../../lib/clipboard";
import { useSkillStore } from "../../stores/skillStore";

const TEMPLATE = `---
name: My skill
description: What this skill helps with, in one line.
activation: auto
triggers: [keyword, another keyword]
---

# Instructions
How Wisp should behave when this skill applies.

# Workflow
1. First step
2. Second step

# Reference
Facts, names or settings worth knowing.

# Examples
User: … → Wisp: …
`;

const sourceText: Record<SkillSummary["source"], string> = {
  builtin: "Built in",
  user: "Yours",
  imported: "Imported",
};
const activationText: Record<string, string> = {
  auto: "Used when relevant",
  always: "Always on",
  manual: "Only for agents that attach it",
};

function Editor({ id, initial, onDone }: { id?: string; initial: string; onDone: () => void }) {
  const { save, error, versions, restore } = useSkillStore();
  const [content, setContent] = useState(initial);
  const [history, setHistory] = useState<SkillVersion[]>([]);
  useEffect(() => {
    if (id)
      void versions(id)
        .then(setHistory)
        .catch(() => {});
  }, [id, versions]);
  return (
    <form
      aria-label={id ? "Edit skill" : "New skill"}
      className="space-y-2 rounded-[var(--radius-lg)] border border-border bg-surface p-4"
      onSubmit={(e) => {
        e.preventDefault();
        void save(content, id).then((s) => s && onDone());
      }}
    >
      <label className="block text-xs text-text-muted" htmlFor="skill-md">
        SKILL.md — front matter (name, description, activation, triggers) then the instructions
      </label>
      <textarea
        id="skill-md"
        aria-label="SKILL.md"
        rows={16}
        spellCheck={false}
        className="selectable w-full rounded-[var(--radius-sm)] border border-border-strong bg-bg p-2 font-mono text-[12.5px] leading-relaxed"
        value={content}
        onChange={(e) => setContent(e.target.value)}
      />
      {error && <ErrorNotice title="That skill couldn't be saved." message={error} />}
      <div className="flex flex-wrap items-center gap-2">
        <Button type="submit" variant="primary">
          Save skill
        </Button>
        <Button variant="ghost" onClick={onDone}>
          Cancel
        </Button>
        {history.length > 1 && (
          <span className="ml-auto flex items-center gap-1.5 text-xs text-text-muted">
            Versions:
            {history.map((v, i) => (
              <button
                key={v.version}
                type="button"
                disabled={i === 0}
                title={i === 0 ? "Current" : `Restore version ${v.version}`}
                onClick={() => id && void restore(id, v.version).then((s) => s && setContent(s.content))}
                className="rounded px-1.5 py-0.5 hover:bg-surface-hover disabled:font-semibold disabled:text-text"
              >
                v{v.version}
              </button>
            ))}
          </span>
        )}
      </div>
    </form>
  );
}

/** Skills (spec §64): reusable expertise as SKILL.md. Never executed; enabled skills are offered when relevant. */
export function SkillsPage() {
  const { skills, load, get, setEnabled, remove, exportText, importText, error } = useSkillStore();
  const [editing, setEditing] = useState<{ id?: string; content: string } | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const file = useRef<HTMLInputElement>(null);
  useEffect(() => {
    void load().catch(() => {});
  }, [load]);

  return (
    <>
      <div className="mb-6 flex items-start gap-3">
        <div className="flex-1">
          <h1 className="text-xl font-semibold tracking-tight">Skills</h1>
          <p className="mt-1 text-md text-text-muted">
            Reusable know-how Wisp follows when it's relevant — instructions, workflows, reference and
            examples. Skills are just text: they're never run as code.
          </p>
        </div>
        {!editing && (
          <div className="flex gap-2">
            <Button onClick={() => file.current?.click()}>Import…</Button>
            <Button variant="primary" onClick={() => setEditing({ content: TEMPLATE })}>
              New skill
            </Button>
          </div>
        )}
        <input
          ref={file}
          type="file"
          accept=".md,text/markdown,text/plain"
          aria-label="Import SKILL.md"
          className="hidden"
          onChange={(e) => {
            const f = e.target.files?.[0];
            e.target.value = "";
            if (!f) return;
            if (f.size > 50_000) return setNote("That file is too large for a skill (50 KB max).");
            void f
              .text()
              .then((t) =>
                importText(t).then(
                  (s) => s && setNote(`Imported “${s.summary.name}”. It's off until you turn it on.`),
                ),
              );
          }}
        />
      </div>

      {editing && (
        <div className="mb-6">
          <Editor
            {...(editing.id ? { id: editing.id } : {})}
            initial={editing.content}
            onDone={() => setEditing(null)}
          />
        </div>
      )}
      {!editing && error && (
        <div className="mb-4">
          <ErrorNotice title="That didn't work." message={error} />
        </div>
      )}
      {note && (
        <p role="status" className="mb-4 text-sm text-text-muted">
          {note}
        </p>
      )}

      {skills.length === 0 ? (
        <EmptyState icon={<SkillsIcon size={28} />} title="No skills yet">
          Create one, or import a SKILL.md file.
        </EmptyState>
      ) : (
        <ul
          aria-label="Skills"
          className="divide-y divide-border rounded-[var(--radius-md)] border border-border bg-surface"
        >
          {skills.map((s) => (
            <li key={s.id} className="flex items-center gap-3 px-3 py-3">
              <div className="min-w-0 flex-1">
                <p className="truncate text-base font-medium">{s.name}</p>
                <p className="truncate text-sm text-text-muted">{s.description || "No description"}</p>
                <p className="mt-0.5 text-xs text-text-faint">
                  {sourceText[s.source]} · {activationText[s.activation] ?? s.activation}
                  {s.triggers.length > 0 && ` · ${s.triggers.slice(0, 5).join(", ")}`} · v{s.version}
                </p>
              </div>
              <Switch
                label={`Use ${s.name}`}
                checked={s.enabled}
                onCheckedChange={(v) => void setEnabled(s.id, v)}
              />
              <Button
                size="sm"
                variant="ghost"
                aria-label={`Edit ${s.name}`}
                onClick={() => void get(s.id).then((full) => setEditing({ id: s.id, content: full.content }))}
              >
                <EditIcon size={14} />
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() =>
                  void exportText(s.id).then((x) =>
                    copyText(x.content).then(() => setNote(`Copied ${x.filename} to the clipboard.`)),
                  )
                }
              >
                Export
              </Button>
              <Button
                size="sm"
                variant="ghost"
                aria-label={`Delete ${s.name}`}
                onClick={() => void remove(s.id)}
              >
                <DeleteIcon size={14} />
              </Button>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}
