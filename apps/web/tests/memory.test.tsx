import { IpcClient, type Message } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";
import { MessageList } from "../src/components/chat/MessageList";
import { MemoryPage } from "../src/features/memory/MemoryPage";
import { setIpcClient } from "../src/services/ipc";
import { useMemoryStore } from "../src/stores/memoryStore";

const item = {
  id: "0199c2a8-0000-7000-8000-0000000000aa",
  category: "preference" as const,
  level: "persistent" as const,
  content: "The user prefers dark roast coffee.",
  source: "extracted" as const,
  importance: 0.5,
  accessCount: 2,
  createdAtMs: 1,
  updatedAtMs: 1,
};

describe("Memory", () => {
  let transport: MockTransport;
  beforeEach(() => {
    transport = new MockTransport()
      .handle("memory.list", () => ({ items: [item], limit: 500 }))
      .handle("memory.delete", () => ({}))
      .handle("memory.clear", () => ({ deleted: 1 }))
      .handle("memory.save", (p) => ({ ...item, id: "new", content: p.content, source: "user" }));
    setIpcClient(new IpcClient(transport));
    useMemoryStore.setState({ items: [], error: null });
  });

  it("shows what's remembered, grouped, and lets you forget it", async () => {
    render(<MemoryPage />);
    expect(await screen.findByText("The user prefers dark roast coffee.")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Preferences" })).toBeInTheDocument();
    expect(screen.getByText("Kept · you said this · used 2×")).toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", { name: "Forget “The user prefers dark roast coffee.”" }),
    );
    await waitFor(() =>
      expect(screen.queryByText("The user prefers dark roast coffee.")).not.toBeInTheDocument(),
    );
  });

  it("adds a memory and asks before forgetting everything", async () => {
    render(<MemoryPage />);
    await userEvent.click(await screen.findByRole("button", { name: "Add memory" }));
    await userEvent.type(
      screen.getByRole("textbox", { name: "What should Wisp remember?" }),
      "I like short answers",
    );
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(transport.sent.find((e) => e.kind === "request" && e.method === "memory.save")).toMatchObject({
        params: { content: "I like short answers", category: "fact", level: "persistent" },
      }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Forget everything…" }));
    await userEvent.click(screen.getByRole("button", { name: "Forget everything" }));
    await waitFor(() =>
      expect(transport.sent.some((e) => e.kind === "request" && e.method === "memory.clear")).toBe(true),
    );
  });

  it("answers say what they remembered, with Undo", async () => {
    const m: Message = {
      id: "a1",
      conversationId: "c",
      role: "assistant",
      content: "Noted!",
      status: "complete",
      createdAtMs: 1,
      savedMemories: [{ id: item.id, content: item.content }],
    };
    render(
      <MemoryRouter>
        <MessageList messages={[m]} />
      </MemoryRouter>,
    );
    expect(screen.getByText(item.content)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Undo" }));
    expect(await screen.findByText("Forgotten.")).toBeInTheDocument();
  });
});
