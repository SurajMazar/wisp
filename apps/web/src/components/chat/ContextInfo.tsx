import type { ContextInfo as Info } from "@wisp/contracts";
import { Popover } from "./Popover";

const kb = (n: number) => (n < 1024 ? `${n} B` : `${Math.round(n / 1024)} KB`);

/** Lines describing exactly what the model was given (privacy transparency). */
export function contextLines(c: Info): string[] {
  const lines: string[] = [];
  if (c.imageBytes) lines.push(`Screenshot of your selection (${kb(c.imageBytes)}, compressed)`);
  if (c.screenTextLines)
    lines.push(`${c.screenTextLines} line${c.screenTextLines === 1 ? "" : "s"} of text read from the screen`);
  if (c.uiElements) lines.push(`${c.uiElements} UI element${c.uiElements === 1 ? "" : "s"} from the app`);
  if (c.selectedText) lines.push("The text you had selected");
  if (c.reusedScreen) lines.push("Your earlier screen selection (reused, not recaptured)");
  if (c.historyMessages)
    lines.push(`${c.historyMessages} earlier message${c.historyMessages === 1 ? "" : "s"}`);
  if (c.summarizedMessages)
    lines.push(`${c.summarizedMessages} older message${c.summarizedMessages === 1 ? "" : "s"}, condensed`);
  if (c.files && c.files.length > 0) lines.push(`From your files: ${c.files.join("; ")}`);
  if (c.memories && c.memories.length > 0)
    lines.push(`${c.memories.length} saved memor${c.memories.length === 1 ? "y" : "ies"} about you`);
  if (c.skills && c.skills.length > 0)
    lines.push(`Skill${c.skills.length === 1 ? "" : "s"}: ${c.skills.join(", ")}`);
  if (lines.length === 0) lines.push("Just your question");
  return lines;
}

export function ContextButton({ context }: { context: Info }) {
  return (
    <Popover
      label="Context used"
      side="top"
      trigger={(open, toggle) => (
        <button
          type="button"
          aria-haspopup="menu"
          aria-expanded={open}
          onClick={toggle}
          className="rounded-[var(--radius-sm)] px-1.5 py-1 text-xs text-text-faint hover:bg-surface-hover hover:text-text"
        >
          Context
        </button>
      )}
    >
      {() => (
        <div className="w-72 p-2 text-sm">
          <p className="mb-1.5 font-medium">What Wisp used to answer</p>
          <ul className="space-y-1 text-text-muted">
            {contextLines(context).map((l) => (
              <li key={l}>· {l}</li>
            ))}
          </ul>
          {context.textOnlyReason && <p className="mt-2 text-xs text-text-faint">{context.textOnlyReason}</p>}
          <p className="mt-2 text-xs text-text-faint">
            About {context.estimatedTokens.toLocaleString()} tokens.
          </p>
        </div>
      )}
    </Popover>
  );
}
