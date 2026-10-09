import { IpcClient } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { OnboardingFlow } from "../src/features/onboarding/OnboardingFlow";
import { setIpcClient } from "../src/services/ipc";
import { useSettingsStore } from "../src/stores/settingsStore";

describe("First run", () => {
  let transport: MockTransport;
  beforeEach(() => {
    transport = new MockTransport()
      .handle("host.mic.permission", () => ({ state: "notRequested" }))
      .handle("host.mic.requestPermission", () => ({ state: "enabled" }))
      .handle("host.screen.permission", () => ({ screenRecording: "notRequested" }))
      .handle("host.ax.status", () => ({ state: "notRequested" }))
      .handle("voice.status", () => ({ models: [], phonemizer: true, sttReady: false, ttsReady: false }))
      .handle("settings.update", () => useSettingsStore.getState().settings as never);
    setIpcClient(new IpcClient(transport));
  });

  it("welcomes, asks for one permission at a time, and can be skipped or finished", async () => {
    render(<OnboardingFlow />);
    expect(screen.getByRole("heading", { name: "Welcome. Meet your AI companion." })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Set Up My Assistant →" }));
    expect(screen.getByRole("heading", { name: "Talk to Wisp" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Allow Microphone" }));
    await waitFor(() =>
      expect(
        transport.sent.some((e) => e.kind === "request" && e.method === "host.mic.requestPermission"),
      ).toBe(true),
    );
    await userEvent.click(screen.getByRole("button", { name: /Not now|Back|Continue/ }));
    expect(screen.getByRole("heading", { name: "Let Wisp see what you point at" })).toBeInTheDocument();
    // Skip the rest.
    for (const name of [/Not now/, /Not now/, /Got it/, /Later|Continue/, /Continue/]) {
      await userEvent.click(screen.getAllByRole("button", { name })[0]!);
    }
    expect(screen.getByRole("heading", { name: "You're all set." })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Start using Wisp" }));
    await waitFor(() =>
      expect(
        transport.sent.find((e) => e.kind === "request" && e.method === "settings.update"),
      ).toMatchObject({
        params: { update: { section: "onboarding", value: { completed: true } } },
      }),
    );
  });
});
