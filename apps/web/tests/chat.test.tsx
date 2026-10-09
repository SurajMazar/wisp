import { IpcClient, type Message } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";
import { AssistantPanel } from "../src/features/assistant/AssistantPanel";
import { ConversationsPage } from "../src/features/conversations/ConversationsPage";
import { setIpcClient } from "../src/services/ipc";
import { useConversationStore } from "../src/stores/conversationStore";
import { useFocusStore } from "../src/stores/focusStore";

const CID = "0199c2a8-0000-7000-8000-0000000000c1";
const msg = (
  id: string,
  role: "user" | "assistant",
  content: string,
  status: Message["status"] = "complete",
): Message => ({
  id,
  conversationId: CID,
  role,
  content,
  status,
  createdAtMs: role === "user" ? 1 : 2,
});

describe("Chat", () => {
  let transport: MockTransport;

  beforeEach(() => {
    transport = new MockTransport()
      .handle("conversation.send", ({ text }) => ({
        conversationId: CID,
        userMessage: msg("u1", "user", text),
        assistantMessage: msg("a1", "assistant", "", "streaming"),
      }))
      .handle("conversation.cancel", () => ({}))
      .handle("conversation.regenerate", ({ messageId }) =>
        msg(`${messageId}-2`, "assistant", "", "streaming"),
      )
      .handle("host.clipboard.writeText", () => ({}))
      .handle("conversation.list", () => ({
        items: [
          {
            id: CID,
            title: "Delete dialog",
            pinned: false,
            messageCount: 2,
            preview: "Click Delete.",
            updatedAtMs: Date.now(),
          },
        ],
      }))
      .handle("conversation.get", () => ({
        id: CID,
        title: "Delete dialog",
        pinned: false,
        createdAtMs: 1,
        updatedAtMs: 2,
        messages: [
          msg("u1", "user", "Which button?"),
          {
            ...msg("a1", "assistant", "Click **Delete**."),
            modelName: "qwen3-vl:8b-instruct",
            locality: "local",
            latencyMs: 2100,
          },
        ],
      }))
      .handle("conversation.pin", () => ({
        id: CID,
        title: "Delete dialog",
        pinned: true,
        messageCount: 2,
        updatedAtMs: 3,
      }));
    setIpcClient(new IpcClient(transport));
    useConversationStore.setState({ summaries: [], conversations: {}, panelId: null, error: null });
    useFocusStore.setState({ region: null, lastEnded: null, permission: "enabled", error: null });
  });

  it("asks from the panel and streams the answer with model and timing", async () => {
    render(<AssistantPanel />);
    await userEvent.type(screen.getByRole("textbox", { name: "Message" }), "What is 2 + 2?{Enter}");
    expect(await screen.findByText("What is 2 + 2?")).toBeInTheDocument();
    expect(screen.getByText("Thinking…")).toBeInTheDocument();
    act(() => {
      const s = useConversationStore.getState();
      s.receiveMessage({
        ...msg("a1", "assistant", "", "streaming"),
        modelName: "qwen3-vl:8b-instruct",
        locality: "local",
      });
      s.receiveDelta({ conversationId: CID, messageId: "a1", delta: "It's " });
      s.receiveDelta({ conversationId: CID, messageId: "a1", delta: "**4**." });
    });
    expect(screen.getByRole("button", { name: "Stop" })).toBeInTheDocument();
    act(() =>
      useConversationStore.getState().receiveMessage({
        ...msg("a1", "assistant", "It's **4**."),
        modelName: "qwen3-vl:8b-instruct",
        locality: "local",
        latencyMs: 1800,
      }),
    );
    expect(screen.getByText("4").tagName).toBe("STRONG");
    expect(screen.getByText("qwen3-vl:8b-instruct · on this Mac · 1.8 s")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Stop" })).not.toBeInTheDocument();
  });

  it("stops a streaming answer", async () => {
    render(<AssistantPanel />);
    await userEvent.type(screen.getByRole("textbox", { name: "Message" }), "Tell me a story{Enter}");
    await userEvent.click(await screen.findByRole("button", { name: "Stop" }));
    await waitFor(() =>
      expect(transport.sent.some((e) => e.kind === "request" && e.method === "conversation.cancel")).toBe(
        true,
      ),
    );
    act(() =>
      useConversationStore.getState().receiveMessage(msg("a1", "assistant", "Once upon", "cancelled")),
    );
    expect(screen.getByText("Stopped")).toBeInTheDocument();
  });

  it("explains errors with retry and technical details", async () => {
    render(<AssistantPanel />);
    await userEvent.type(screen.getByRole("textbox", { name: "Message" }), "hello{Enter}");
    act(() =>
      useConversationStore.getState().receiveMessage({
        ...msg("a1", "assistant", "", "error"),
        error: "I couldn't connect to Ollama. Make sure Ollama is running.",
        errorDetails: "connection refused",
      }),
    );
    expect(screen.getByRole("alert")).toHaveTextContent("Make sure Ollama is running");
    await userEvent.click(screen.getByRole("button", { name: "Show Technical Details" }));
    expect(screen.getByText("connection refused")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Try Again" }));
    await waitFor(() =>
      expect(
        transport.sent.filter((e) => e.kind === "request" && e.method === "conversation.send"),
      ).toHaveLength(2),
    );
  });

  it("attaches the focused area to the next question", async () => {
    useFocusStore.setState({
      region: {
        id: "0199c2a8-0000-7000-8000-00000000f0c5",
        kind: "rectangle",
        display: { id: 1, name: "Built-in", frame: { x: 0, y: 0, width: 1512, height: 982 }, scale: 2 },
        selection: { x: 1, y: 1, width: 300, height: 200 },
        crop: { x: 0, y: 0, width: 320, height: 220 },
        path: [],
        cursor: { x: 5, y: 5 },
        createdAtMs: 1,
      },
    });
    render(<AssistantPanel />);
    expect(screen.getByRole("button", { name: "Remove selection" })).toBeInTheDocument();
    await userEvent.type(screen.getByRole("textbox", { name: "Message" }), "What's this?{Enter}");
    await waitFor(() =>
      expect(
        transport.sent.find((e) => e.kind === "request" && e.method === "conversation.send"),
      ).toMatchObject({ params: { includeFocus: true } }),
    );
  });

  it("lists, opens and pins conversations", async () => {
    render(
      <MemoryRouter initialEntries={[`/conversations/${CID}`]}>
        <Routes>
          <Route path="/conversations/:id" element={<ConversationsPage />} />
          <Route path="/conversations" element={<ConversationsPage />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(await screen.findByRole("heading", { name: "Delete dialog" })).toBeInTheDocument();
    expect(screen.getByText("Delete", { selector: "strong" })).toBeInTheDocument();
    expect(screen.getByText("qwen3-vl:8b-instruct · on this Mac · 2.1 s")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Pin" }));
    await waitFor(() =>
      expect(transport.sent.some((e) => e.kind === "request" && e.method === "conversation.pin")).toBe(true),
    );
  });

  it("copies and regenerates an answer, and sends with the chosen model", async () => {
    useConversationStore.setState({
      conversations: {
        [CID]: {
          id: CID,
          title: "Math",
          pinned: false,
          createdAtMs: 1,
          updatedAtMs: 2,
          messages: [msg("u1", "user", "2+2?"), msg("a1", "assistant", "4")],
        },
      },
      panelId: CID,
      model: { providerId: "ollama", model: "llama3.2:3b" },
    });
    render(<AssistantPanel />);
    await userEvent.click(screen.getByRole("button", { name: "Copy" }));
    await waitFor(() =>
      expect(
        transport.sent.find((e) => e.kind === "request" && e.method === "host.clipboard.writeText"),
      ).toMatchObject({ params: { text: "4" } }),
    );
    expect(await screen.findByRole("button", { name: "Copied" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Regenerate" }));
    await waitFor(() =>
      expect(
        transport.sent.find((e) => e.kind === "request" && e.method === "conversation.regenerate"),
      ).toMatchObject({
        params: { messageId: "a1", model: { providerId: "ollama", model: "llama3.2:3b" } },
      }),
    );
    act(() => useConversationStore.getState().receiveRemoved(msg("a1", "assistant", "4")));
    expect(screen.queryByText("4")).not.toBeInTheDocument();

    await userEvent.type(screen.getByRole("textbox", { name: "Message" }), "and 3+3?{Enter}");
    await waitFor(() =>
      expect(
        transport.sent.find((e) => e.kind === "request" && e.method === "conversation.send"),
      ).toMatchObject({
        params: { model: { providerId: "ollama", model: "llama3.2:3b" } },
      }),
    );
  });
});
