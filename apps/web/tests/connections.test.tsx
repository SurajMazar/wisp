import { IpcClient, type AppSettings, type McpServerStatus } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { ConnectionsPage, splitArgs } from "../src/features/connections/ConnectionsPage";
import { setIpcClient } from "../src/services/ipc";
import { useServiceStore } from "../src/stores/serviceStore";
import { useSettingsStore } from "../src/stores/settingsStore";
import { defaultSettings } from "./fixtures";

describe("Connections", () => {
  let transport: MockTransport;
  let stored: AppSettings;
  let status: McpServerStatus[];

  beforeEach(async () => {
    stored = defaultSettings();
    stored.privacy.mode = "cloudAllowed";
    status = [];
    transport = new MockTransport()
      .handle("settings.get", () => stored)
      .handle("settings.update", ({ update }) => {
        stored = { ...stored, [update.section]: update.value } as AppSettings;
        return stored;
      })
      .handle("mcp.status", () => ({ servers: status }))
      .handle("mcp.reconnect", ({ id }) => status.find((s) => s.id === id)!)
      .handle("host.keychain.set", () => ({}))
      .handle("host.keychain.delete", () => ({}));
    setIpcClient(new IpcClient(transport));
    useServiceStore.setState({ status: {}, error: null });
    await useSettingsStore.getState().load();
  });

  const sent = (m: string) =>
    transport.sent.filter((e) => e.kind === "request" && e.method === m) as unknown as { params: never }[];

  it("splits arguments like a shell", () => {
    expect(splitArgs(`-y @notionhq/notion-mcp-server "~/My Notes" ''`)).toEqual([
      "-y",
      "@notionhq/notion-mcp-server",
      "~/My Notes",
      "",
    ]);
  });

  it("adds a local server, keeping its secret in the Keychain", async () => {
    render(<ConnectionsPage />);
    await userEvent.click(await screen.findByRole("button", { name: "Add Connection…" }));
    await userEvent.click(screen.getByRole("button", { name: "Notion" }));
    expect(screen.getByLabelText("Command")).toHaveValue("npx");
    await userEvent.type(screen.getByLabelText("NOTION_TOKEN value"), "ntn_secret");
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    await waitFor(() => expect(stored.connections.servers).toHaveLength(1));

    const [server] = stored.connections.servers;
    expect(server).toMatchObject({
      id: "notion",
      name: "Notion",
      enabled: true,
      transport: {
        kind: "stdio",
        config: {
          command: "npx",
          args: ["-y", "@notionhq/notion-mcp-server"],
          env: [{ name: "NOTION_TOKEN", keychainAccount: "mcp:notion:NOTION_TOKEN" }],
        },
      },
    });
    expect(JSON.stringify(stored)).not.toContain("ntn_secret");
    expect(sent("host.keychain.set")[0]!.params).toEqual({
      account: "mcp:notion:NOTION_TOKEN",
      secret: "ntn_secret",
    });
  });

  it("shows a connected server's tools and saves a per-tool rule", async () => {
    stored.connections.servers = [
      {
        id: "notes",
        name: "Notes",
        enabled: true,
        localOnly: false,
        tools: [],
        transport: { kind: "stdio", config: { command: "node", args: ["server.mjs"], env: [] } },
      },
    ];
    status = [
      {
        id: "notes",
        state: "ready",
        serverName: "notes-server",
        tools: [
          { name: "search", description: "Find notes", readOnly: true, destructive: false },
          { name: "wipe", description: "Delete everything", readOnly: false, destructive: true },
        ],
      },
    ];
    await useSettingsStore.getState().load();
    render(<ConnectionsPage />);
    expect(await screen.findByText("Connected")).toBeInTheDocument();
    expect(screen.getByText("reads only")).toBeInTheDocument();
    expect(screen.getByText("can delete")).toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("radiogroup", { name: "Rule for wipe" }).querySelector('[value="block"]')!,
    );
    await waitFor(() =>
      expect(stored.connections.servers[0]!.tools).toEqual([{ tool: "wipe", rule: "block" }]),
    );

    // Live status updates arrive as events.
    act(() => useServiceStore.getState().receive({ id: "notes", state: "error", error: "boom", tools: [] }));
    expect(screen.getByText("Couldn't connect")).toBeInTheDocument();
    expect(screen.getByText("boom")).toBeInTheDocument();
  });
});
