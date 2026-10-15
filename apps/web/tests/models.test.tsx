import { IpcClient, type AppSettings, type ModelsList } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { act, render, screen, waitFor, within } from "@testing-library/react";
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

  it("keeps a cloud key added elsewhere when it saves a choice", async () => {
    render(<ModelsPage />);
    const use = await screen.findByRole("button", { name: "Use for screen" });
    // Another window adds an OpenAI key after this page loaded its copy of the settings.
    const openai = {
      id: "openai",
      name: "OpenAI",
      baseUrl: "https://api.openai.com/v1",
      keychainAccount: "endpoint:openai",
      preset: "openai" as const,
    };
    stored = { ...stored, models: { ...stored.models, endpoints: [openai] } };
    await userEvent.click(use);
    await waitFor(() => expect(stored.models.vision).toEqual({ providerId: "ollama", model: "qwen3-vl:8b" }));
    expect(stored.models.endpoints).toEqual([openai]);
  });

  it("lists cloud models apart from local ones, grouped and searchable", async () => {
    const cloud = (id: string) => ({
      providerId: "endpoint:openai",
      id,
      name: id,
      locality: "cloud" as const,
      capabilities: ["text" as const, "streaming" as const],
    });
    transport.handle("models.list", () => ({
      providers: [
        ...list.providers,
        {
          id: "endpoint:openai",
          kind: "openaiCompatible",
          name: "OpenAI",
          locality: "cloud",
          state: "available",
        },
      ],
      models: [...list.models, cloud("gpt-5"), cloud("gpt-5-mini"), cloud("o4-mini")],
    }));
    render(<ModelsPage />);
    const cloudGroup = (await screen.findByRole("region", { name: "OpenAI" })).closest("section")!;
    expect(within(cloudGroup).getByText("3 models")).toBeInTheDocument();
    expect(within(cloudGroup).getByText("gpt-5")).toBeInTheDocument();
    // Local models aren't mixed in with the cloud ones.
    expect(within(cloudGroup).queryByText("qwen3-vl:8b")).not.toBeInTheDocument();

    await userEvent.type(screen.getByRole("searchbox", { name: "Search cloud models" }), "mini");
    expect(screen.getByText("2 of 3 models")).toBeInTheDocument();
    expect(screen.queryByText("gpt-5")).not.toBeInTheDocument();
    expect(screen.getByText("gpt-5-mini")).toBeInTheDocument();
    expect(screen.getByText("o4-mini")).toBeInTheDocument();
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
