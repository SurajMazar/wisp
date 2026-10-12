import { IpcClient, type AppSettings, type ModelsList } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { OnboardingFlow } from "../src/features/onboarding/OnboardingFlow";
import { setIpcClient } from "../src/services/ipc";
import { useModelsStore } from "../src/stores/modelsStore";
import { useSettingsStore } from "../src/stores/settingsStore";
import { defaultSettings } from "./fixtures";

const provider = (kind: "ollama" | "apple", state: string) => ({
  id: kind,
  kind,
  name: kind,
  locality: "local",
  state,
});

describe("First run: Wisp's brain", () => {
  let transport: MockTransport;
  let stored: AppSettings;
  let models: ModelsList;

  const start = async (list: ModelsList) => {
    models = list;
    stored = defaultSettings();
    transport = new MockTransport()
      .handle("settings.get", () => stored)
      .handle("settings.update", ({ update }) => {
        stored = { ...stored, [update.section]: update.value } as AppSettings;
        return stored;
      })
      .handle("models.list", () => models)
      .handle("models.pull", () => ({}))
      .handle("host.mic.permission", () => ({ state: "enabled" }))
      .handle("host.screen.permission", () => ({ screenRecording: "enabled" }))
      .handle("host.ax.status", () => ({ state: "enabled" }));
    setIpcClient(new IpcClient(transport));
    useModelsStore.setState({ list: null, pulls: {} });
    localStorage.setItem("wisp.onboarding.step", "brain");
    await useSettingsStore.getState().load();
    render(<OnboardingFlow />);
  };

  beforeEach(() => localStorage.clear());

  it("offers the recommended model and uses it once downloaded", async () => {
    await start({
      providers: [provider("ollama", "available"), provider("apple", "available")],
      models: [],
    } as never);
    expect(await screen.findByText(/can already answer with Apple's on-device model/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Download qwen3-vl:4b-instruct (3.3 GB)" }));
    expect(transport.sent.some((e) => e.kind === "request" && e.method === "models.pull")).toBe(true);

    act(() =>
      useModelsStore.getState().receivePull({
        name: "qwen3-vl:4b-instruct",
        completed: 1,
        total: 2,
        done: false,
        status: "pulling",
      }),
    );
    expect(screen.getByText(/50% · keeps going in the background/)).toBeInTheDocument();
    models = {
      ...models,
      models: [
        {
          providerId: "ollama",
          id: "qwen3-vl:4b-instruct",
          name: "qwen3-vl:4b-instruct",
          locality: "local",
          capabilities: ["text", "vision"],
        },
      ],
    } as never;
    act(() =>
      useModelsStore
        .getState()
        .receivePull({ name: "qwen3-vl:4b-instruct", completed: 2, total: 2, done: true, status: "success" }),
    );
    await waitFor(() =>
      expect(stored.models.general).toEqual({ providerId: "ollama", model: "qwen3-vl:4b-instruct" }),
    );
    expect(stored.models.vision).toEqual({ providerId: "ollama", model: "qwen3-vl:4b-instruct" });
    expect(await screen.findByText(/Ready — answering with qwen3-vl:4b-instruct/)).toBeInTheDocument();
  });

  it("works with Apple's model when nothing is installed, and says how to do better", async () => {
    await start({
      providers: [provider("ollama", "notInstalled"), provider("apple", "available")],
      models: [],
    } as never);
    expect(await screen.findByText(/answer with Apple's on-device model for now/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continue →" })).toBeInTheDocument();
  });

  it("is ready when a model is already there", async () => {
    await start({
      providers: [provider("ollama", "available")],
      models: [
        {
          providerId: "ollama",
          id: "qwen3-vl:8b-instruct",
          name: "qwen3-vl:8b-instruct",
          locality: "local",
          capabilities: ["text", "vision"],
        },
      ],
    } as never);
    expect(await screen.findByText(/Ready — answering with qwen3-vl:8b-instruct/)).toBeInTheDocument();
  });
});
