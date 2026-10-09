import type { FileItem } from "@wisp/contracts";
import { DeleteIcon, FilesIcon } from "@wisp/icons";
import { Button, EmptyState, ErrorNotice, Spinner, cn } from "@wisp/ui";
import { useEffect, useState } from "react";
import { useFileStore } from "../../stores/fileStore";

const size = (n: number) =>
  n < 1024 * 1024 ? `${Math.max(1, Math.round(n / 1024))} KB` : `${(n / 1024 / 1024).toFixed(1)} MB`;

function Status({ f }: { f: FileItem }) {
  if (f.status === "indexed") return <span className="text-xs text-text-faint">{f.chunks} passages</span>;
  if (f.status === "failed")
    return <span className="text-xs text-danger">{f.error ?? "Couldn't read it"}</span>;
  return (
    <span className="flex items-center gap-1 text-xs text-text-muted">
      <Spinner /> Reading…
    </span>
  );
}

/** Files (spec §68–69): your documents, read and searched on this Mac; answers cite them. */
export function FilesPage() {
  const { items, hits, error, load, pick, drop, remove, search, open } = useFileStore();
  const [query, setQuery] = useState("");
  const [over, setOver] = useState(false);
  useEffect(() => {
    void load().catch(() => {});
  }, [load]);
  useEffect(() => {
    const t = setTimeout(() => void search(query).catch(() => {}), 200);
    return () => clearTimeout(t);
  }, [search, query]);

  return (
    <div
      onDragOver={(e) => {
        e.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(e) => {
        e.preventDefault();
        setOver(false);
        void drop([...e.dataTransfer.files]);
      }}
      className={cn(
        "min-h-[60vh] rounded-[var(--radius-lg)] transition-colors",
        over && "bg-accent/5 outline-2 outline-dashed outline-accent/50",
      )}
    >
      <div className="mb-6 flex items-start gap-3">
        <div className="flex-1">
          <h1 className="text-xl font-semibold tracking-tight">Files</h1>
          <p className="mt-1 text-md text-text-muted">
            Add documents and Wisp can answer from them, citing where it found things. Files are copied into
            Wisp and read on this Mac — PDFs, Word, text, Markdown, CSV, JSON, code and images.
          </p>
        </div>
        <Button variant="primary" onClick={() => void pick()}>
          Import…
        </Button>
      </div>

      {error && (
        <div className="mb-4">
          <ErrorNotice title="That didn't work." message={error} />
        </div>
      )}

      {items.length > 0 && (
        <input
          aria-label="Search your files"
          placeholder="Search your files"
          className="mb-3 h-8 w-full rounded-[var(--radius-sm)] border border-border-strong bg-bg px-2 text-base"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      )}
      {query && (
        <ul aria-label="Search results" className="mb-6 space-y-2">
          {hits.length === 0 && <li className="text-sm text-text-faint">Nothing found.</li>}
          {hits.map((h, i) => (
            <li key={i} className="rounded-[var(--radius-md)] border border-border bg-surface p-3">
              <button
                type="button"
                onClick={() => void open(h.fileId)}
                className="text-sm font-medium text-accent hover:underline"
              >
                {h.citation}
              </button>
              <p className="selectable mt-1 text-sm text-text-muted">{h.snippet}…</p>
            </li>
          ))}
        </ul>
      )}

      {items.length === 0 ? (
        <EmptyState icon={<FilesIcon size={28} />} title="No files yet">
          Drop files here or choose Import….
        </EmptyState>
      ) : (
        <ul
          aria-label="Files"
          className="divide-y divide-border rounded-[var(--radius-md)] border border-border bg-surface"
        >
          {items.map((f) => (
            <li key={f.id} className="flex items-center gap-3 px-3 py-2.5">
              <div className="min-w-0 flex-1">
                <button
                  type="button"
                  onClick={() => void open(f.id)}
                  className="block max-w-full truncate text-left text-base font-medium hover:underline"
                >
                  {f.name}
                </button>
                <p className="flex items-center gap-2 text-xs text-text-faint">
                  {size(f.sizeBytes)} · <Status f={f} />
                </p>
              </div>
              <Button
                size="sm"
                variant="ghost"
                aria-label={`Remove ${f.name}`}
                onClick={() => void remove(f.id)}
              >
                <DeleteIcon size={14} />
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
