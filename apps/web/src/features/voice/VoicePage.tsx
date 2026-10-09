import type { SpeakAnswers, SpeechModelInfo, VoiceSettings } from "@wisp/contracts";
import {
  Button,
  ErrorNotice,
  Segmented,
  SettingsGroup,
  SettingsRow,
  Slider,
  Spinner,
  Switch,
} from "@wisp/ui";
import { useEffect, useState } from "react";
import { useSettingsStore } from "../../stores/settingsStore";
import { useVoiceStore } from "../../stores/voiceStore";

const voices: { id: string; label: string }[] = [
  { id: "af_heart", label: "Heart (US, warm)" },
  { id: "af_bella", label: "Bella (US)" },
  { id: "af_sky", label: "Sky (US)" },
  { id: "am_michael", label: "Michael (US)" },
  { id: "am_adam", label: "Adam (US)" },
  { id: "bf_emma", label: "Emma (UK)" },
];

const mb = (b: number) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : `${Math.round(b / 1e6)} MB`);
const presetLabel: Record<string, string> = {
  fast: "Fast",
  balanced: "Balanced",
  highQuality: "High quality",
};

function ModelRow({
  m,
  selected,
  onSelect,
}: {
  m: SpeechModelInfo;
  selected: boolean;
  onSelect?: () => void;
}) {
  const { downloads, download, cancelDownload, remove } = useVoiceStore();
  const d = downloads[m.id];
  const active = d && !d.done;
  const pct = d && d.total ? Math.round((d.completed / d.total) * 100) : 0;
  return (
    <div className="space-y-1 px-3 py-2.5">
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <p className="text-base">
            {m.name}{" "}
            {m.preset && (
              <span className="text-sm text-text-faint">· {presetLabel[m.preset] ?? m.preset}</span>
            )}
          </p>
          <p className="text-xs text-text-muted">
            {mb(m.sizeBytes)} · {m.languages.join(", ")} · {m.license} ·{" "}
            {m.installed ? "On this Mac" : "Not downloaded"}
          </p>
        </div>
        <div className="flex shrink-0 gap-1.5">
          {m.installed && onSelect && (
            <Button
              size="sm"
              variant={selected ? "primary" : "secondary"}
              onClick={onSelect}
              disabled={selected}
            >
              {selected ? "In use" : "Use"}
            </Button>
          )}
          {!m.installed && !active && (
            <Button size="sm" onClick={() => void download(m.id)}>
              {d?.error === "Paused" ? "Resume" : "Download"}
            </Button>
          )}
          {active && (
            <Button size="sm" variant="ghost" onClick={() => void cancelDownload(m.id)}>
              Pause
            </Button>
          )}
          {m.installed && !selected && (
            <Button size="sm" variant="ghost" onClick={() => void remove(m.id)}>
              Remove
            </Button>
          )}
        </div>
      </div>
      {active && (
        <div role="status">
          <div className="h-1.5 overflow-hidden rounded-full bg-bg-sunken">
            <div className="h-full bg-accent" style={{ width: `${pct}%` }} />
          </div>
          <p className="mt-0.5 text-xs text-text-faint">
            {mb(d.completed)} / {mb(d.total)} · checked against its published checksum when done
          </p>
        </div>
      )}
      {d?.done && d.error && d.error !== "Paused" && <p className="text-xs text-danger">{d.error}</p>}
    </div>
  );
}

function MicTest() {
  const { listening, level, start, stop, lastResult, error } = useVoiceStore();
  const [testing, setTesting] = useState(false);
  const run = async () => {
    setTesting(true);
    await start("test");
    setTimeout(() => void stop().finally(() => setTesting(false)), 4000);
  };
  return (
    <SettingsRow
      label="Test microphone"
      hint={
        listening ? (
          <span className="flex items-center gap-2">
            Say something…
            <span className="inline-block h-1.5 w-24 overflow-hidden rounded-full bg-bg-sunken">
              <span
                className="block h-full bg-success transition-[width] duration-75"
                style={{ width: `${Math.round(level * 100)}%` }}
              />
            </span>
          </span>
        ) : error ? (
          <span className="text-danger">{error}</span>
        ) : lastResult ? (
          lastResult.transcript ? (
            `✓ Heard: “${lastResult.transcript}” (${lastResult.sttMs} ms)`
          ) : (
            "Didn't hear any speech. Try speaking a little louder."
          )
        ) : (
          "Records 4 seconds and shows what Wisp heard."
        )
      }
      control={
        <Button size="sm" disabled={testing} onClick={() => void run()}>
          {testing ? "Listening…" : "Test Microphone"}
        </Button>
      }
    />
  );
}

export function VoicePage() {
  const { status, load, speak, stopSpeaking, error } = useVoiceStore();
  const settings = useSettingsStore((s) => s.settings?.voice);
  const update = useSettingsStore((s) => s.update);
  const [draft, setDraft] = useState<VoiceSettings | null>(settings ?? null);
  const [synced, setSynced] = useState(settings);
  if (synced !== settings) {
    setSynced(settings);
    setDraft(settings ?? null);
  }

  useEffect(() => {
    void load();
  }, [load]);

  if (!draft || !status) {
    return (
      <div className="flex items-center gap-2 pt-16 text-text-muted">
        <Spinner /> Loading voice…
      </div>
    );
  }

  const save = (patch: Partial<VoiceSettings>) => {
    const next = { ...draft, ...patch };
    setDraft(next);
    void update({ section: "voice", value: next }).then(() => load());
  };
  const stt = status.models.filter((m) => m.kind === "stt");
  const tts = status.models.filter((m) => m.kind === "tts");

  return (
    <>
      <h1 className="mb-6 text-xl font-semibold tracking-tight">Voice</h1>

      {status.detail && (
        <div className="mb-4">
          <ErrorNotice title="Voice needs one more thing" message={status.detail} />
        </div>
      )}

      <SettingsGroup
        title="Talking to Wisp"
        description="Hold your shortcut and speak; let go to ask. Everything is transcribed on this Mac — audio never leaves it."
      >
        <SettingsRow
          label="Hold to talk"
          control={
            <Switch
              label="Hold to talk"
              checked={draft.enabled}
              onCheckedChange={(enabled) => save({ enabled })}
            />
          }
        />
        <SettingsRow
          label="Sensitivity"
          hint="Higher hears quieter speech; lower ignores more background noise."
          control={
            <Slider
              label="Voice sensitivity"
              value={draft.vadSensitivity}
              min={0}
              max={1}
              step={0.05}
              onValueChange={(vadSensitivity) => setDraft({ ...draft, vadSensitivity })}
              onValueCommit={(vadSensitivity) => save({ vadSensitivity })}
            />
          }
        />
        <MicTest />
      </SettingsGroup>

      <SettingsGroup
        title="Speech recognition"
        description="Whisper, running on this Mac. Bigger models are more accurate and slower."
      >
        {stt.map((m) => (
          <ModelRow
            key={m.id}
            m={m}
            selected={draft.sttModel === m.id}
            onSelect={() => save({ sttModel: m.id })}
          />
        ))}
      </SettingsGroup>

      <SettingsGroup title="Spoken answers" description="Kokoro, running on this Mac.">
        <SettingsRow
          label="Speak answers"
          control={
            <Segmented<SpeakAnswers>
              label="When to speak answers"
              value={draft.speakAnswers}
              onValueChange={(speakAnswers) => save({ speakAnswers })}
              options={[
                { value: "voiceQuestions", label: "To spoken questions" },
                { value: "always", label: "Always" },
                { value: "never", label: "Never" },
              ]}
            />
          }
        />
        <SettingsRow
          label="Voice"
          control={
            <select
              aria-label="Voice"
              className="h-6 rounded-[var(--radius-sm)] border border-border-strong bg-surface px-1.5 text-sm"
              value={draft.voice}
              onChange={(e) => save({ voice: e.target.value })}
            >
              {voices.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.label}
                </option>
              ))}
            </select>
          }
        />
        <SettingsRow
          label="Speed"
          hint={`${draft.speed.toFixed(2)}×`}
          control={
            <Slider
              label="Speech speed"
              value={draft.speed}
              min={0.5}
              max={2}
              step={0.05}
              onValueChange={(speed) => setDraft({ ...draft, speed })}
              onValueCommit={(speed) => save({ speed })}
            />
          }
        />
        <SettingsRow
          label="Volume"
          control={
            <Slider
              label="Speech volume"
              value={draft.volume}
              min={0}
              max={1}
              step={0.05}
              onValueChange={(volume) => setDraft({ ...draft, volume })}
              onValueCommit={(volume) => save({ volume })}
            />
          }
        />
        <SettingsRow
          label="Test voice"
          hint={error ?? (status.ttsReady ? undefined : "Download Kokoro below first.")}
          control={
            <div className="flex gap-1.5">
              <Button
                size="sm"
                disabled={!status.ttsReady}
                onClick={() => void speak("Hi, I'm Wisp. Point at anything and ask me about it.")}
              >
                Test Voice
              </Button>
              <Button size="sm" variant="ghost" onClick={() => void stopSpeaking()}>
                Stop
              </Button>
            </div>
          }
        />
        {tts.map((m) => (
          <ModelRow key={m.id} m={m} selected={true} />
        ))}
      </SettingsGroup>
    </>
  );
}
