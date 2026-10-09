import { IpcClient, type Message } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";
import { MessageList } from "../src/components/chat/MessageList";
import { FilesPage } from "../src/features/files/FilesPage";
import { setIpcClient } from "../src/services/ipc";
import { useFileStore } from "../src/stores/fileStore";

const file = {
  id: "0199c2a8-0000-7000-8000-0000000000f6",
  name: "Acme handbook.md",
  mime: "text/markdown",
  sizeBytes: 2048,
  status: "indexed" as const,
  chunks: 4,
  createdAtMs: 1,
};

describe("Files", () => {
  let transport: MockTransport;
  beforeEach(() => {
    transport = new MockTransport()
      .handle("file.list", () => ({ items: [file] }))
      .handle("file.import", () => ({ items: [] }))
      .handle("file.open", () => ({}))
      .handle("file.importData", (p) => ({ ...file, id: "f2", name: p.name, status: "indexing", chunks: 0 }))
      .handle("file.search", () => ({
        hits: [
          {
            fileId: file.id,
            name: file.name,
            citation: "Acme handbook.md, lines 7–9",
            snippet: "Employees get 25 vacation days per year.",
            score: 0.8,
          },
        ],
      }));
    setIpcClient(new IpcClient(transport));
    useFileStore.setState({ items: [], hits: [], error: null });
  });

  it("lists files, searches them and opens the source", async () => {
    render(<FilesPage />);
    expect(await screen.findByText("Acme handbook.md")).toBeInTheDocument();
    expect(screen.getByText("4 passages")).toBeInTheDocument();
    await userEvent.type(screen.getByRole("textbox", { name: "Search your files" }), "vacation days");
    expect(await screen.findByText(/Employees get 25 vacation days/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Acme handbook.md, lines 7–9" }));
    await waitFor(() =>
      expect(transport.sent.some((e) => e.kind === "request" && e.method === "file.open")).toBe(true),
    );
  });

  it("imports with the picker and by dropping files", async () => {
    render(<FilesPage />);
    await userEvent.click(await screen.findByRole("button", { name: "Import…" }));
    await waitFor(() =>
      expect(transport.sent.some((e) => e.kind === "request" && e.method === "file.import")).toBe(true),
    );
    const dropped = new File(["# Notes\nShip on Friday."], "notes.md", { type: "text/markdown" });
    fireEvent.drop(screen.getByRole("heading", { name: "Files" }), { dataTransfer: { files: [dropped] } });
    await waitFor(() =>
      expect(
        transport.sent.find((e) => e.kind === "request" && e.method === "file.importData"),
      ).toMatchObject({
        params: { name: "notes.md", dataBase64: btoa("# Notes\nShip on Friday.") },
      }),
    );
    expect(await screen.findByText("notes.md")).toBeInTheDocument();
  });

  it("answers show their sources", async () => {
    const m: Message = {
      id: "a1",
      conversationId: "c",
      role: "assistant",
      content: "You get 25 vacation days [1].",
      status: "complete",
      createdAtMs: 1,
      citations: [{ n: 1, fileId: file.id, label: "Acme handbook.md, lines 7–9" }],
    };
    render(
      <MemoryRouter>
        <MessageList messages={[m]} />
      </MemoryRouter>,
    );
    await userEvent.click(screen.getByRole("button", { name: "[1] Acme handbook.md, lines 7–9" }));
    await waitFor(() =>
      expect(transport.sent.find((e) => e.kind === "request" && e.method === "file.open")).toMatchObject({
        params: { id: file.id },
      }),
    );
  });
});
