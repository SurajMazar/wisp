import { CheckIcon, CopyIcon } from "@wisp/icons";
import { cn } from "@wisp/ui";
import { Fragment, useState, type ReactNode } from "react";
import { copyText } from "../../lib/clipboard";

function CodeBlock({ lang, code }: { lang: string; code: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="my-2 overflow-hidden rounded-[var(--radius-md)] border border-border bg-bg-sunken">
      <div className="flex items-center justify-between border-b border-border px-3 py-1 text-xs text-text-faint">
        <span>{lang || "code"}</span>
        <button
          type="button"
          aria-label="Copy code"
          className="flex items-center gap-1 rounded px-1 py-0.5 hover:bg-surface-hover hover:text-text"
          onClick={() =>
            void copyText(code).then(() => {
              setCopied(true);
              setTimeout(() => setCopied(false), 1500);
            })
          }
        >
          {copied ? <CheckIcon size={12} /> : <CopyIcon size={12} />}
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      <pre className="selectable overflow-x-auto p-3 font-mono text-[12.5px] leading-relaxed">{code}</pre>
    </div>
  );
}

/** Safe Markdown subset for answers: headings, code fences, inline code, bold, italics, links, lists.
 *  Renders React nodes only — never HTML strings. */
export function RichText({ text, streaming = false }: { text: string; streaming?: boolean }) {
  const blocks: ReactNode[] = [];
  text.split(/```/).forEach((part, i) => {
    if (i % 2 === 1) {
      const nl = part.indexOf("\n");
      const lang = nl > -1 ? part.slice(0, nl).trim() : "";
      const code = (nl > -1 ? part.slice(nl + 1) : part).replace(/\n$/, "");
      blocks.push(<CodeBlock key={i} lang={/^[\w+-]{1,20}$/.test(lang) ? lang : ""} code={code} />);
      return;
    }
    part
      .split(/\n{2,}/)
      .filter((p) => p.trim())
      .forEach((para, j) => {
        const key = `${i}-${j}`;
        const lines = para.split("\n");
        const heading = /^(#{1,4})\s+(.*)$/.exec(para.trim());
        if (heading && lines.length === 1) {
          const level = heading[1]?.length ?? 1;
          blocks.push(
            <p
              key={key}
              className={level <= 2 ? "mb-1 mt-3 text-lg font-semibold" : "mb-1 mt-2.5 font-semibold"}
            >
              {inline(heading[2] ?? "")}
            </p>,
          );
          return;
        }
        const isList = lines.every((l) => /^\s*([-*•]|\d+[.)])\s+/.test(l) || !l.trim());
        if (isList) {
          const ordered = /^\s*\d+[.)]/.test(lines[0] ?? "");
          const items = lines.filter((l) => l.trim()).map((l) => l.replace(/^\s*([-*•]|\d+[.)])\s+/, ""));
          const List = ordered ? "ol" : "ul";
          blocks.push(
            <List
              key={key}
              className={`my-1.5 space-y-1 pl-5 ${ordered ? "list-decimal" : "list-disc"} marker:text-text-faint`}
            >
              {items.map((it, k) => (
                <li key={k}>{inline(it)}</li>
              ))}
            </List>,
          );
        } else {
          blocks.push(
            <p key={key} className="my-2 whitespace-pre-wrap first:mt-0 last:mb-0">
              {inline(para)}
            </p>,
          );
        }
      });
  });
  return <div className={cn("selectable leading-relaxed", streaming && "wisp-streaming")}>{blocks}</div>;
}

function inline(s: string): ReactNode {
  return s
    .split(/(`[^`]+`|\*\*[^*]+\*\*|\[[^\]]+\]\(https?:\/\/[^)\s]+\)|(?<![*\w])\*[^*\s][^*]*\*(?!\*))/)
    .map((t, i) => {
      if (t.startsWith("`") && t.endsWith("`") && t.length > 1)
        return (
          <code
            key={i}
            className="rounded-[var(--radius-xs)] bg-bg-sunken px-1 py-px font-mono text-[0.88em]"
          >
            {t.slice(1, -1)}
          </code>
        );
      if (t.startsWith("**") && t.endsWith("**") && t.length > 4)
        return <strong key={i}>{t.slice(2, -2)}</strong>;
      const link = /^\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)$/.exec(t);
      if (link)
        return (
          <a
            key={i}
            href={link[2]}
            target="_blank"
            rel="noreferrer"
            className="text-accent underline-offset-2 hover:underline"
          >
            {link[1]}
          </a>
        );
      if (t.startsWith("*") && t.endsWith("*") && t.length > 2) return <em key={i}>{t.slice(1, -1)}</em>;
      return <Fragment key={i}>{t}</Fragment>;
    });
}
