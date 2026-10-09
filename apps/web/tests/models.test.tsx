import { IpcClient, type AppSettings, type ModelsList } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { ModelsPage } from "../src/features/models/ModelsPage";
import { setIpcClient } from "../src/services/ipc";
import { useModelsStore } from "../src/stores/modelsStore";
import { useSettingsStore } from "../src/stores/settingsStore";
import { defaultSettings } from "./fixtures";

const list: ModelsList = {
  providers: [
    { id: "apple", kind: "apple", name: "Apple Foundation Model", locality: "local", state: "available" },
    {
      id: "ollama",
      kind: "ollama",
      name: "Ollama",
      locality: "local",
      state: "available",
      version: "0.40.1",
    },
  ],
  models: [
    {
      providerId: "apple",
      id: "system",
      name: "Apple on-device model",
      locality: "local",
      capabilities: ["text", "streaming"],
      contextLength: 4096,
    },
    {
      providerId: "ollama",
      id: "qwen3-vl:8b",
      name: "qwen3-vl:8b",
      locality: "local",
      capabilities: ["text", "vision", "streaming"],
      contextLength: 262144,
      sizeBytes: 6.1e9,
      parameterSize: "8.8B",
    },
  ],
};

describe("Models page", () => {
  let transport: MockTransport;
  let stored: AppSettings;

  beforeEach(async () => {
    stored = defaultSettings();
    transport = new MockTransport()
      .handle("settings.get", () => stored)
      .handle("settings.update", ({ update }) => {
        stored = { ...stored, [update.section]: update.value } as AppSettings;
        return stored;
      })
      .handle("models.list", () => list)
      .handle("ai.route", ({ needsVision }) => ({
        model: needsVision
          ? { providerId: "ollama", model: "qwen3-vl:8b" }
          : { providerId: "apple", model: "system" },
        modelName: needsVision ? "qwen3-vl:8b" : "Apple on-device model",
        providerName: needsVision ? "Ollama" : "Apple Foundation Model",
        locality: "local",
        reason: needsVision ? "Best local model that can see images" : "Runs on this Mac",
      }))
      .handle("models.test", ({ model }) => ({
        model: model!,
        locality: "local",
        text: "4",
        firstTokenMs: 420,
        totalMs: 900,
      }))
      .handle("models.pull", () => ({}))
      .handle("models.cancelPull", () => ({}));
    setIpcClient(new IpcClient(transport));
    useModelsStore.setState({
      list: null,
      pulls: {},
      tests: {},
      routes: { text: null, vision: null },
      error: null,
    });
    await useSettingsStore.getState().load();
  });

  it("shows live providers, capabilities and who answers", async () => {
    render(<ModelsPage />);
    expect(await screen.findByText("Best local model that can see images")).toBeInTheDocument();
    expect(screen.getAllByText("Available")).toHaveLength(2);
    expect(screen.getByText("Sees images")).toBeInTheDocument();
    expect(screen.getByText("256K context")).toBeInTheDocument();
    expect(screen.getByText("v0.40.1")).toBeInTheDocument();
  });

  it("tests a model and shows real timing", async () => {
    render(<ModelsPage />);
    const buttons = await screen.findAllByRole("button", { name: "Test" });
    await userEvent.click(buttons[0]!);
    expect(await screen.findByText(/“4” · first word in 420 ms/)).toBeInTheDocument();
  });

  it("sets a preferred screen model", async () => {
    render(<ModelsPage />);
    await userEvent.click(await screen.findByRole("button", { name: "Use for screen" }));
    await waitFor(() => expect(stored.models.vision).toEqual({ providerId: "ollama", model: "qwen3-vl:8b" }));
  });

  it("downloads with progress and can pause and resume", async () => {
    render(<ModelsPage />);
    await userEvent.click(await screen.findByRole("button", { name: /qwen3-vl:4b-instruct/ }));
    act(() =>
      useModelsStore.getState().receivePull({
        name: "qwen3-vl:4b-instruct",
        status: "pulling",
        completed: 1.1e9,
        total: 3.3e9,
        done: false,
      }),
    );
    expect(screen.getByText("1.1 GB / 3.3 GB · 33%")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Pause" }));
    act(() =>
      useModelsStore
        .getState()
        .receivePull({ name: "qwen3-vl:4b-instruct", status: "cancelled", done: true }),
    );
    expect(await screen.findByRole("button", { name: "Resume" })).toBeInTheDocument();
  });
});
