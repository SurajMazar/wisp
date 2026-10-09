import { Fragment, type ReactNode } from "react";

/** Minimal, safe Markdown subset for answers: code fences, inline code, bold, lists, paragraphs.
 *  Renders React nodes only — never HTML strings. */
export function RichText({ text }: { text: string }) {
  const blocks: ReactNode[] = [];
  const parts = text.split(/```/);
  parts.forEach((part, i) => {
    if (i % 2 === 1) {
      const body = part.replace(/^[a-zA-Z0-9_+-]*\n/, "");
      blocks.push(
        <pre
          key={i}
          className="selectable my-1.5 overflow-x-auto rounded-[var(--radius-sm)] bg-bg-sunken p-2 font-mono text-xs"
        >
          {body.replace(/\n$/, "")}
        </pre>,
      );
      return;
    }
    part
      .split(/\n{2,}/)
      .filter((p) => p.trim())
      .forEach((para, j) => {
        const lines = para.split("\n");
        const isList = lines.every((l) => /^\s*([-*]|\d+\.)\s+/.test(l) || !l.trim());
        if (isList) {
          const ordered = /^\s*\d+\./.test(lines[0] ?? "");
          const items = lines.filter((l) => l.trim()).map((l) => l.replace(/^\s*([-*]|\d+\.)\s+/, ""));
          const List = ordered ? "ol" : "ul";
          blocks.push(
            <List key={`${i}-${j}`} className={`my-1 pl-5 ${ordered ? "list-decimal" : "list-disc"}`}>
              {items.map((it, k) => (
                <li key={k}>{inline(it)}</li>
              ))}
            </List>,
          );
        } else {
          blocks.push(
            <p key={`${i}-${j}`} className="my-1 whitespace-pre-wrap">
              {inline(para)}
            </p>,
          );
        }
      });
  });
  return <div className="selectable">{blocks}</div>;
}

function inline(s: string): ReactNode {
  return s.split(/(`[^`]+`|\*\*[^*]+\*\*)/).map((t, i) =>
    t.startsWith("`") && t.endsWith("`") && t.length > 1 ? (
      <code key={i} className="rounded-[var(--radius-xs)] bg-bg-sunken px-1 font-mono text-[0.92em]">
        {t.slice(1, -1)}
      </code>
    ) : t.startsWith("**") && t.endsWith("**") && t.length > 4 ? (
      <strong key={i}>{t.slice(2, -2)}</strong>
    ) : (
      <Fragment key={i}>{t}</Fragment>
    ),
  );
}
