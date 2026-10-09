import type { Lesson } from "@wisp/contracts";
import { CloseIcon, StepDoneIcon, StepPendingIcon, TeachIcon } from "@wisp/icons";
import { Button, cn } from "@wisp/ui";
import { useState } from "react";
import { stripMarkers } from "../../lib/guidance";
import { lessonActive, useTeachStore } from "../../stores/teachStore";
import { useToolStore } from "../../stores/toolStore";

const statusText: Record<Lesson["status"], string> = {
  thinking: "Looking at your screen…",
  waitingForUser: "Your turn",
  done: "Done",
  stuck: "Stuck",
  stopped: "Stopped",
  failed: "Couldn't continue",
};

const clean = (s: string) => stripMarkers(s).replace(/^(DONE|STUCK):\s*/, "");

/** A guided lesson (spec §53): each step, what Wisp is doing, and the controls. Never hidden. */
export function LessonCard({
  conversationId,
  compact,
}: {
  conversationId?: string | null;
  compact?: boolean;
}) {
  const { lesson, control, dismissed, dismiss, error } = useTeachStore();
  const run = useToolStore((s) => s.run);
  const [doing, setDoing] = useState<string | null>(null);
  if (!lesson || dismissed.includes(lesson.id)) return null;
  if (conversationId && lesson.conversationId !== conversationId) return null;
  const active = lessonActive(lesson);
  const current = lesson.steps.find((s) => s.status === "current");

  const doIt = async () => {
    const t = current?.target;
    if (!t) return;
    setDoing("Waiting for your OK…");
    const r = await run(
      {
        tool: "click",
        args: {
          x: t.rect.x + t.rect.width / 2,
          y: t.rect.y + t.rect.height / 2,
          ...(t.label ? { label: t.label } : {}),
        },
      },
      current.messageId,
    );
    setDoing(
      r
        ? r.record.status === "succeeded"
          ? null
          : (r.record.error ?? "Not done.")
        : useToolStore.getState().error,
    );
  };

  return (
    <section
      aria-label={`Lesson: ${lesson.goal}`}
      className="rounded-[var(--radius-lg)] border border-accent/35 bg-accent/5 p-3"
    >
      <header className="flex items-center gap-2">
        <TeachIcon size={15} className="shrink-0 text-accent" />
        <span className="min-w-0 flex-1 truncate text-sm font-semibold">Learning: {lesson.goal}</span>
        <span className={cn("text-xs", active ? "text-accent" : "text-text-muted")} role="status">
          {statusText[lesson.status]}
        </span>
        {!active && (
          <button
            type="button"
            aria-label="Close lesson"
            onClick={dismiss}
            className="rounded p-0.5 text-text-faint hover:text-text"
          >
            <CloseIcon size={13} />
          </button>
        )}
      </header>
      {lesson.steps.length > 0 && (
        <ol className={cn("mt-2 space-y-1", compact ? "max-h-32 overflow-y-auto" : "")}>
          {lesson.steps.map((s, i) => (
            <li key={s.messageId} className="flex gap-2 text-sm">
              {s.status === "done" ? (
                <StepDoneIcon size={14} className="mt-0.5 shrink-0 text-success" />
              ) : (
                <StepPendingIcon size={14} className="mt-0.5 shrink-0 animate-pulse text-accent" />
              )}
              <span className={s.status === "done" ? "text-text-muted" : "font-medium"}>
                <span className="sr-only">Step {i + 1}: </span>
                {clean(s.instruction)}
                {s.confirmed && <span className="ml-1 text-xs text-text-faint">· {s.confirmed}</span>}
              </span>
            </li>
          ))}
        </ol>
      )}
      {lesson.status === "thinking" && (
        <p className="wisp-shimmer mt-2 text-sm">Working out the next step…</p>
      )}
      {lesson.note && <p className="mt-2 text-sm text-text-muted">{clean(lesson.note)}</p>}
      {lesson.status === "waitingForUser" && (
        <p className="mt-1.5 text-xs text-text-faint">
          {lesson.watching ? "Do it when you're ready — Wisp will notice." : "Do it, then press “I did it”."}
        </p>
      )}
      {(error || doing) && <p className="mt-1.5 text-xs text-text-muted">{doing ?? error}</p>}
      {active && (
        <div className="mt-2.5 flex flex-wrap gap-1.5">
          <Button
            size="sm"
            variant="primary"
            onClick={() => void control("next")}
            disabled={lesson.status === "thinking"}
          >
            I did it
          </Button>
          <Button
            size="sm"
            onClick={() => void control("repeat")}
            disabled={lesson.status !== "waitingForUser"}
          >
            Show me again
          </Button>
          {current?.target && (
            <Button
              size="sm"
              variant="ghost"
              onClick={() => void doIt()}
              disabled={lesson.status !== "waitingForUser"}
            >
              Do it for me
            </Button>
          )}
          <Button size="sm" variant="ghost" className="ml-auto" onClick={() => void control("stop")}>
            Stop lesson
          </Button>
        </div>
      )}
    </section>
  );
}
