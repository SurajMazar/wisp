import type { FocusKind, FocusRegion } from "@wisp/contracts";
import {
  AreaIcon,
  CloseIcon,
  PlusIcon,
  PointIcon,
  ScreenIcon,
  SendIcon,
  StopIcon,
  WindowIcon,
} from "@wisp/icons";
import { cn } from "@wisp/ui";
import { useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { describeRegion, regionLabel } from "../../lib/focus";
import { useFocusStore } from "../../stores/focusStore";
import { MicButton } from "./MicButton";
import { ModelPicker } from "./ModelPicker";
import { MenuItem, Popover } from "./Popover";

const focusActions: { kind: FocusKind; label: string; hint: string; icon: ReactNode }[] = [
  {
    kind: "rectangle",
    label: "Select an area",
    hint: "Drag around anything on screen",
    icon: <AreaIcon size={15} />,
  },
  { kind: "point", label: "Point at something", hint: "Click a spot", icon: <PointIcon size={15} /> },
  {
    kind: "window",
    label: "Current window",
    hint: "The window under your pointer",
    icon: <WindowIcon size={15} />,
  },
  {
    kind: "screen",
    label: "Whole screen",
    hint: "Everything on this display",
    icon: <ScreenIcon size={15} />,
  },
];

/** The focused screenshot attached to the next question, shown as a removable chip. */
function FocusChip({ region }: { region: FocusRegion }) {
  const clear = useFocusStore((s) => s.clear);
  const details = [
    describeRegion(region),
    region.image && `${region.image.widthPx} × ${region.image.heightPx} px · stays on this Mac`,
  ]
    .filter(Boolean)
    .join("\n");
  return (
    <span
      title={details}
      className="inline-flex max-w-full items-center gap-2 rounded-[var(--radius-md)] border border-border bg-bg-sunken py-1 pl-1 pr-1.5 text-sm"
    >
      {region.image ? (
        <img src={region.image.url} alt="Captured area" className="h-8 w-12 rounded-[5px] object-cover" />
      ) : (
        <AreaIcon size={16} />
      )}
      <span className="min-w-0">
        <span className="block truncate text-text">{regionLabel(region)}</span>
        <span className="block truncate text-xs text-text-faint">{describeRegion(region)}</span>
      </span>
      <button
        type="button"
        aria-label="Remove selection"
        onClick={() => void clear()}
        className="rounded-full p-0.5 text-text-faint hover:bg-surface-hover hover:text-text"
      >
        <CloseIcon size={13} />
      </button>
    </span>
  );
}

export function Composer({
  onSend,
  onStop,
  streaming,
  placeholder = "Ask anything",
  autoFocus,
  focus,
  compact = false,
  text: controlledText,
  onTextChange,
}: {
  onSend: (text: string) => void;
  onStop?: () => void;
  streaming: boolean;
  placeholder?: string;
  autoFocus?: boolean;
  /** Attachable focus region and whether it's attached. */
  focus?: { region: FocusRegion; attached: boolean; setAttached: (v: boolean) => void } | null;
  compact?: boolean;
  /** Optional controlled draft (for suggestion chips). */
  text?: string;
  onTextChange?: (t: string) => void;
}) {
  const [ownText, setOwnText] = useState("");
  const text = controlledText ?? ownText;
  const setText = onTextChange ?? setOwnText;
  const area = useRef<HTMLTextAreaElement>(null);
  const startFocus = useFocusStore((s) => s.start);

  useLayoutEffect(() => {
    const el = area.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, compact ? 140 : 220)}px`;
  }, [text, compact]);

  const submit = () => {
    const t = text.trim();
    if (!t || streaming) return;
    onSend(t);
    setText("");
  };
  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      submit();
    }
  };
  const canSend = text.trim().length > 0;

  return (
    <div
      className={cn(
        "rounded-[22px] border border-border-strong bg-surface shadow-[var(--shadow-md)] transition-colors focus-within:border-text-faint",
        compact ? "px-2.5 pb-2 pt-2" : "px-3.5 pb-2.5 pt-3",
      )}
      onClick={(e) => {
        if (e.target === e.currentTarget) area.current?.focus();
      }}
    >
      {focus?.attached && (
        <div className="mb-2">
          <FocusChip region={focus.region} />
        </div>
      )}
      <textarea
        ref={area}
        aria-label="Message"
        rows={1}
        className={cn(
          "block w-full resize-none bg-transparent px-1 outline-none placeholder:text-text-faint",
          compact ? "text-base" : "text-md",
        )}
        placeholder={placeholder}
        value={text}
        autoFocus={autoFocus}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={onKey}
      />
      <div className="mt-2 flex items-center gap-1">
        <Popover
          label="Add from your screen"
          trigger={(open, toggle) => (
            <button
              type="button"
              aria-label="Add from your screen"
              aria-haspopup="menu"
              aria-expanded={open}
              onClick={toggle}
              className="flex h-7 w-7 items-center justify-center rounded-full border border-border text-text-muted hover:bg-surface-hover hover:text-text"
            >
              <PlusIcon size={15} />
            </button>
          )}
        >
          {(close) => (
            <>
              {focus && !focus.attached && (
                <MenuItem
                  icon={<AreaIcon size={15} />}
                  label="Include last selection"
                  onSelect={() => {
                    focus.setAttached(true);
                    close();
                  }}
                />
              )}
              {focusActions.map((a) => (
                <MenuItem
                  key={a.kind}
                  icon={a.icon}
                  label={a.label}
                  hint={a.hint}
                  onSelect={() => {
                    close();
                    void startFocus(a.kind);
                  }}
                />
              ))}
            </>
          )}
        </Popover>
        <MicButton disabled={streaming} />
        <span className="ml-auto" />
        <ModelPicker />
        {streaming ? (
          <button
            type="button"
            aria-label="Stop"
            onClick={onStop}
            className="flex h-8 w-8 items-center justify-center rounded-full bg-text text-bg hover:opacity-85"
          >
            <StopIcon size={12} fill="currentColor" />
          </button>
        ) : (
          <button
            type="button"
            aria-label="Send"
            disabled={!canSend}
            onClick={submit}
            className="flex h-8 w-8 items-center justify-center rounded-full bg-text text-bg transition-opacity hover:opacity-85 disabled:opacity-25"
          >
            <SendIcon size={16} strokeWidth={2.4} />
          </button>
        )}
      </div>
    </div>
  );
}
