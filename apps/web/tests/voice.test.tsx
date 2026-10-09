import { IpcClient, type AppSettings, type VoiceStatus } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { MicButton } from "../src/components/chat/MicButton";
import { VoicePage } from "../src/features/voice/VoicePage";
import { setIpcClient } from "../src/services/ipc";
import { useSettingsStore } from "../src/stores/settingsStore";
import { useVoiceStore } from "../src/stores/voiceStore";
import { defaultSettings } from "./fixtures";

const status = (over: Partial<VoiceStatus> = {}): VoiceStatus => ({
  sttReady: true,
  ttsReady: true,
  phonemizer: true,
  models: [
    {
      id: "whisper-base.en",
      name: "Whisper Base (English)",
      kind: "stt",
      preset: "fast",
      sizeBytes: 147964211,
      installed: true,
      languages: ["en"],
      license: "MIT",
    },
    {
      id: "whisper-small.en",
      name: "Whisper Small (English)",
      kind: "stt",
      preset: "balanced",
      sizeBytes: 487614201,
      installed: false,
      languages: ["en"],
      license: "MIT",
    },
    {
      id: "kokoro-82m",
      name: "Kokoro 82M",
      kind: "tts",
      sizeBytes: 95000000,
      installed: true,
      languages: ["en-us"],
      license: "Apache-2.0",
      voices: ["af_heart"],
    },
  ],
  ...over,
});

describe("Voice", () => {
  let transport: MockTransport;
  let stored: AppSettings;
  let current: VoiceStatus;

  beforeEach(async () => {
    stored = defaultSettings();
    current = status();
    transport = new MockTransport()
      .handle("settings.get", () => stored)
      .handle("settings.update", ({ update }) => {
        stored = { ...stored, [update.section]: update.value } as AppSettings;
        return stored;
      })
      .handle("voice.status", () => current)
      .handle("voice.models.download", () => ({}))
      .handle("voice.models.cancel", () => ({}))
      .handle("voice.speak", () => ({}))
      .handle("host.voice.start", () => ({}))
      .handle("host.voice.stop", () => ({ transcript: "testing one two three", audioMs: 4000, sttMs: 180 }))
      .handle("host.voice.stopSpeaking", () => ({}));
    setIpcClient(new IpcClient(transport));
    useVoiceStore.setState({
      status: null,
      level: 0,
      listening: false,
      downloads: {},
      lastResult: null,
      error: null,
    });
    await useSettingsStore.getState().load();
  });

  const sent = (m: string) => transport.sent.filter((e) => e.kind === "request" && e.method === m);

  it("lists speech models with real install state and downloads with progress", async () => {
    render(<VoicePage />);
    expect(await screen.findByText("In use")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Download" }));
    act(() =>
      useVoiceStore
        .getState()
        .receiveProgress({ id: "whisper-small.en", completed: 2.4e8, total: 4.876e8, done: false }),
    );
    expect(screen.getByText(/240 MB \/ 488 MB/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Pause" }));
    expect(sent("voice.models.cancel")).toHaveLength(1);
  });

  it("tests the microphone and shows what was heard", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    render(<VoicePage />);
    await userEvent.click(await screen.findByRole("button", { name: "Test Microphone" }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(4100);
    });
    expect(await screen.findByText(/Heard: “testing one two three”/)).toBeInTheDocument();
    expect(sent("host.voice.start")[0]).toMatchObject({ params: { purpose: "test" } });
    vi.useRealTimers();
  });

  it("tests the Kokoro voice and explains a missing phonemizer", async () => {
    render(<VoicePage />);
    await userEvent.click(await screen.findByRole("button", { name: "Test Voice" }));
    expect(sent("voice.speak")).toHaveLength(1);
    current = status({
      phonemizer: false,
      ttsReady: false,
      detail: "Kokoro needs espeak-ng for pronunciation.",
    });
    await act(() => useVoiceStore.getState().load());
    expect(screen.getByText("Kokoro needs espeak-ng for pronunciation.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Test Voice" })).toBeDisabled();
  });

  it("the panel mic button toggles listening", async () => {
    render(<MicButton />);
    await userEvent.click(screen.getByRole("button", { name: "Talk" }));
    expect(useVoiceStore.getState().listening).toBe(true);
    await userEvent.click(screen.getByRole("button", { name: "Stop listening and ask" }));
    await waitFor(() => expect(sent("host.voice.stop")).toHaveLength(1));
  });
});
