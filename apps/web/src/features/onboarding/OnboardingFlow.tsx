import type { AccentColor } from "@wisp/contracts";
import { StepDoneIcon, WispMark } from "@wisp/icons";
import { Button, cn } from "@wisp/ui";
import { useEffect, useState, type ReactNode } from "react";
import { ipc } from "../../services/ipc";
import { usePermissionsStore, type PermissionKind } from "../../stores/permissionsStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useVoiceStore } from "../../stores/voiceStore";
import { accents } from "../settings/accents";

type Step =
  "welcome" | "microphone" | "screenRecording" | "accessibility" | "shortcut" | "voice" | "theme" | "ready";
const steps: Step[] = [
  "welcome",
  "microphone",
  "screenRecording",
  "accessibility",
  "shortcut",
  "voice",
  "theme",
  "ready",
];

const permissionCopy: Record<PermissionKind, { title: string; why: string; how: string; button: string }> = {
  microphone: {
    title: "Talk to Wisp",
    why: "Hold your shortcut and ask out loud. Wisp only listens while you hold it, and your voice is turned into text on this Mac.",
    how: "macOS will ask once.",
    button: "Allow Microphone",
  },
  screenRecording: {
    title: "Let Wisp see what you point at",
    why: "When you select part of the screen, Wisp captures just that area so it can explain it. It never records continuously, and captures stay on this Mac.",
    how: "macOS opens System Settings — turn Wisp on, then restart Wisp.",
    button: "Allow Screen Access",
  },
  accessibility: {
    title: "Understand buttons and menus",
    why: "Wisp reads the names of the controls you point at (never passwords), so it can point at the right one — and click it for you only when you approve.",
    how: "macOS opens System Settings — turn Wisp on, then come back.",
    button: "Allow Accessibility",
  },
};

function Frame({ children, footer }: { children: ReactNode; footer: ReactNode }) {
  return (
    <div className="flex h-full flex-col items-center justify-center bg-bg px-8">
      <div className="w-full max-w-[520px]">
        {children}
        <div className="mt-8 flex items-center gap-2">{footer}</div>
      </div>
    </div>
  );
}

/** First launch (spec §19–20): friendly, one permission at a time, every step skippable. */
export function OnboardingFlow() {
  const { settings, update } = useSettingsStore();
  const { states, check, request } = usePermissionsStore();
  const voice = useVoiceStore();
  // Remember the step: granting Screen Recording needs a restart, and setup should resume there.
  const [step, setStepState] = useState<Step>(() => {
    try {
      const saved = localStorage.getItem("wisp.onboarding.step") as Step | null;
      return saved && steps.includes(saved) ? saved : "welcome";
    } catch {
      return "welcome";
    }
  });
  const [asked, setAsked] = useState<Step[]>([]);
  const setStep = (s: Step) => {
    setStepState(s);
    try {
      localStorage.setItem("wisp.onboarding.step", s);
    } catch {
      /* storage unavailable: setup just starts over */
    }
  };
  const index = steps.indexOf(step);
  const next = () => setStep(steps[Math.min(index + 1, steps.length - 1)] ?? "ready");
  const finish = () => {
    try {
      localStorage.removeItem("wisp.onboarding.step");
    } catch {
      /* nothing to clear */
    }
    void update({ section: "onboarding", value: { completed: true, completedAtMs: Date.now() } });
  };

  const loadVoice = voice.load;
  useEffect(() => {
    void check().catch(() => {});
    if (step === "voice") void loadVoice().catch(() => {});
    // Re-check when the user returns from System Settings.
    const onFocus = () => void check().catch(() => {});
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [check, step, loadVoice]);

  const progress = (
    <div className="mb-6 flex gap-1" aria-hidden>
      {steps.slice(1).map((s, i) => (
        <span key={s} className={cn("h-1 flex-1 rounded-full", i < index ? "bg-accent" : "bg-border")} />
      ))}
    </div>
  );

  if (step === "welcome") {
    return (
      <Frame
        footer={
          <>
            <Button variant="primary" onClick={next}>
              Set Up My Assistant →
            </Button>
            <Button variant="ghost" onClick={finish}>
              I'll do this later
            </Button>
          </>
        }
      >
        <WispMark size={44} />
        <h1 className="mt-5 text-2xl font-semibold tracking-tight">Welcome. Meet your AI companion.</h1>
        <p className="mt-3 text-md leading-relaxed text-text-muted">
          Wisp can understand your screen, listen to you, point things out, and help you work on your Mac —
          all on this Mac, privately. Let's set everything up. It takes about a minute.
        </p>
      </Frame>
    );
  }

  if (step === "microphone" || step === "screenRecording" || step === "accessibility") {
    const c = permissionCopy[step];
    const granted = states[step] === "enabled";
    return (
      <Frame
        footer={
          <>
            {granted ? (
              <Button variant="primary" onClick={next}>
                Continue →
              </Button>
            ) : (
              <Button
                variant="primary"
                onClick={() => {
                  setAsked((a) => [...a, step]);
                  void request(step).then(() => check());
                }}
              >
                {c.button}
              </Button>
            )}
            {!granted && step !== "microphone" && asked.includes(step) && (
              <Button
                onClick={() =>
                  void ipc()
                    .call("host.app.relaunch")
                    .catch(() => {})
                }
              >
                Restart Wisp
              </Button>
            )}
            <Button variant="ghost" onClick={next}>
              {granted ? "Back" : "Skip for now"}
            </Button>
          </>
        }
      >
        {progress}
        <h1 className="text-xl font-semibold tracking-tight">{c.title}</h1>
        <p className="mt-3 text-md leading-relaxed text-text-muted">{c.why}</p>
        <p className="mt-2 text-sm text-text-faint">{c.how}</p>
        {!granted && step !== "microphone" && asked.includes(step) && (
          <p className="mt-3 rounded-[var(--radius-md)] border border-border bg-surface p-3 text-sm text-text-muted">
            Already turned Wisp on in System Settings? macOS applies this only after Wisp restarts — choose{" "}
            <b className="text-text">Restart Wisp</b> and setup will continue from here.
          </p>
        )}
        {granted && (
          <p role="status" className="mt-4 flex items-center gap-1.5 text-sm text-success">
            <StepDoneIcon size={15} /> Allowed
          </p>
        )}
      </Frame>
    );
  }

  if (step === "shortcut") {
    return (
      <Frame
        footer={
          <Button variant="primary" onClick={next}>
            Got it →
          </Button>
        }
      >
        {progress}
        <h1 className="text-xl font-semibold tracking-tight">Your shortcut</h1>
        <ul className="mt-4 space-y-3 text-md text-text-muted">
          <li>
            <kbd className="rounded border border-border-strong bg-surface px-1.5 py-0.5 font-mono text-sm text-text">
              ⌥⌘
            </kbd>{" "}
            <b className="text-text">Hold</b> to talk — and drag while holding to point at something.
          </li>
          <li>
            <kbd className="rounded border border-border-strong bg-surface px-1.5 py-0.5 font-mono text-sm text-text">
              ⌥⌘
            </kbd>{" "}
            <b className="text-text">Tap</b> to open Wisp next to your cursor.
          </li>
        </ul>
        <p className="mt-3 text-sm text-text-faint">You can change it anytime in Settings.</p>
      </Frame>
    );
  }

  if (step === "voice") {
    const wanted = (voice.status?.models ?? []).filter(
      (m) => (m.kind === "stt" && m.id === settings?.voice.sttModel) || m.kind === "tts",
    );
    const missing = wanted.filter((m) => !m.installed);
    const mb = Math.round(missing.reduce((n, m) => n + m.sizeBytes, 0) / 1e6);
    return (
      <Frame
        footer={
          <>
            {missing.length > 0 ? (
              <Button variant="primary" onClick={() => missing.forEach((m) => void voice.download(m.id))}>
                Download voice ({mb} MB)
              </Button>
            ) : (
              <Button variant="primary" onClick={next}>
                Continue →
              </Button>
            )}
            <Button variant="ghost" onClick={next}>
              {missing.length > 0 ? "Later" : "Back"}
            </Button>
          </>
        }
      >
        {progress}
        <h1 className="text-xl font-semibold tracking-tight">A voice that runs on your Mac</h1>
        <p className="mt-3 text-md leading-relaxed text-text-muted">
          Speech recognition and Wisp's voice run entirely on this Mac. They need a one-time download.
        </p>
        <ul className="mt-4 space-y-1.5 text-sm">
          {wanted.map((m) => {
            const d = voice.downloads[m.id];
            return (
              <li key={m.id} className="flex items-center gap-2">
                {m.installed ? (
                  <StepDoneIcon size={14} className="text-success" />
                ) : (
                  <span className="h-3.5 w-3.5 rounded-full border border-border-strong" />
                )}
                <span>{m.name}</span>
                <span className="text-text-faint">
                  {m.installed
                    ? "ready"
                    : d && !d.done
                      ? `${Math.round((d.completed / Math.max(1, d.total)) * 100)}%`
                      : `${Math.round(m.sizeBytes / 1e6)} MB`}
                </span>
              </li>
            );
          })}
        </ul>
      </Frame>
    );
  }

  if (step === "theme") {
    const appearance = settings?.appearance;
    const pick = (accent: AccentColor) =>
      appearance && void update({ section: "appearance", value: { ...appearance, accent } });
    return (
      <Frame
        footer={
          <Button variant="primary" onClick={next}>
            Continue →
          </Button>
        }
      >
        {progress}
        <h1 className="text-xl font-semibold tracking-tight">Make it yours</h1>
        <p className="mt-3 text-md text-text-muted">
          Pick a color for Wisp and its cursor. You can fine-tune the cursor's shape and glow in Settings.
        </p>
        <div className="mt-5 flex flex-wrap gap-2" role="radiogroup" aria-label="Accent color">
          {accents.map((a) => (
            <button
              key={a.value}
              type="button"
              role="radio"
              aria-checked={appearance?.accent === a.value}
              aria-label={a.label}
              onClick={() => pick(a.value)}
              className={cn(
                "h-9 w-9 rounded-full border-2 transition-transform hover:scale-105",
                appearance?.accent === a.value ? "border-text" : "border-transparent",
              )}
              style={{ background: a.swatch }}
            />
          ))}
        </div>
      </Frame>
    );
  }

  return (
    <Frame
      footer={
        <Button variant="primary" onClick={finish}>
          Start using Wisp
        </Button>
      }
    >
      {progress}
      <WispMark size={36} />
      <h1 className="mt-4 text-xl font-semibold tracking-tight">You're all set.</h1>
      <p className="mt-3 text-md leading-relaxed text-text-muted">
        Hold <b className="text-text">⌥⌘</b> and ask "What am I looking at?", or say "Teach me how to…" to
        learn something step by step. Everything you allowed can be changed in Permissions.
      </p>
    </Frame>
  );
}
