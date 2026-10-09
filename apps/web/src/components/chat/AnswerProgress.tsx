import type { Message, MessageStage } from "@wisp/contracts";
import { ChevronRightIcon, StepDoneIcon, ThinkingIcon } from "@wisp/icons";
import { cn } from "@wisp/ui";
import { useLayoutEffect, useRef, useState } from "react";
import { seconds, useNow } from "../../lib/useNow";
import type { StageStep } from "../../stores/conversationStore";

const active: Partial<Record<MessageStage, (m: Message) => string>> = {
  preparing: () => "Getting ready",
  readingScreen: () => "Reading your screen",
  choosingModel: () => "Choosing a model",
  loadingModel: (m) => `Loading ${m.modelName ?? "the model"} into memory`,
  waiting: () => "Thinking",
};

/** Completed steps worth showing, like the step trail in agentic tools. */
function doneSteps(m: Message, steps: StageStep[]): string[] {
  const seen = new Set(steps.map((s) => s.stage));
  const out: string[] = [];
  if (seen.has("readingScreen")) out.push("Read your screen");
  if (m.modelName) out.push(`Using ${m.modelName} · ${m.locality === "cloud" ? "cloud" : "on this Mac"}`);
  if (seen.has("loadingModel") && m.stage !== "loadingModel") out.push("Model loaded");
  return out;
}

/** Live "what's happening" for an answer that hasn't started writing yet. */
export function StepTrail({ m, steps }: { m: Message; steps: StageStep[] }) {
  const streaming = m.status === "streaming";
  const now = useNow(streaming);
  if (!streaming || m.content) return null;
  const stage = m.stage ?? steps.at(-1)?.stage ?? "preparing";
  const label = active[stage]?.(m);
  const since = steps.at(-1)?.at ?? m.createdAtMs;
  const done = doneSteps(m, steps);
  return (
    <div className="space-y-1 text-sm" aria-live="polite">
      {done.map((d) => (
        <p key={d} className="flex items-center gap-1.5 text-text-faint">
          <StepDoneIcon size={13} className="shrink-0" />
          {d}
        </p>
      ))}
      {label && (
        <p className="flex items-center gap-1.5" role="status">
          <span className="relative flex h-[13px] w-[13px] shrink-0 items-center justify-center">
            <span className="absolute h-2 w-2 animate-ping rounded-full bg-accent/60" />
            <span className="h-1.5 w-1.5 rounded-full bg-accent" />
          </span>
          <span className="wisp-shimmer">{label}…</span>
          <span className="text-xs tabular-nums text-text-faint">{seconds(now - since)}</span>
          {stage === "loadingModel" && (
            <span className="text-xs text-text-faint">· only slow the first time</span>
          )}
        </p>
      )}
    </div>
  );
}

/** The model's reasoning: streams open while it thinks, then folds to "Thought for Ns". */
export function ReasoningBlock({ m, steps }: { m: Message; steps: StageStep[] }) {
  const thinking = m.status === "streaming" && !m.content;
  const now = useNow(thinking);
  const [open, setOpen] = useState<boolean | null>(null);
  const body = useRef<HTMLDivElement>(null);
  const expanded = open ?? thinking;

  useLayoutEffect(() => {
    const el = body.current;
    if (el && thinking) el.scrollTop = el.scrollHeight;
  }, [m.reasoning, thinking]);

  if (!m.reasoning) return null;
  const startedAt = steps.find((s) => s.stage === "reasoning")?.at ?? m.createdAtMs;
  const took = m.reasoningMs ?? (thinking ? now - startedAt : undefined);
  return (
    <div className="mb-2">
      <button
        type="button"
        aria-expanded={expanded}
        onClick={() => setOpen(!expanded)}
        className="group/r flex items-center gap-1.5 rounded-[var(--radius-sm)] py-0.5 text-sm text-text-muted hover:text-text"
      >
        <ThinkingIcon size={14} className="shrink-0" />
        {thinking ? (
          <span className="wisp-shimmer">Thinking…</span>
        ) : (
          <span>Thought for {took !== undefined ? seconds(took) : "a moment"}</span>
        )}
        {thinking && took !== undefined && (
          <span className="text-xs tabular-nums text-text-faint">{seconds(took)}</span>
        )}
        <ChevronRightIcon size={13} className={cn("transition-transform", expanded && "rotate-90")} />
      </button>
      {expanded && (
        <div
          ref={body}
          className={cn(
            "selectable mt-1 whitespace-pre-wrap border-l-2 border-border pl-3 text-sm leading-relaxed text-text-faint",
            thinking ? "max-h-40 overflow-y-auto" : "max-h-80 overflow-y-auto",
          )}
        >
          {m.reasoning.trimStart()}
        </div>
      )}
    </div>
  );
}
