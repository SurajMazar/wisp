import { IpcClient, type Message } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import { MessageList } from "../src/components/chat/MessageList";
import { stripMarkers } from "../src/lib/guidance";
import { setIpcClient } from "../src/services/ipc";

describe("Guidance", () => {
  it("hides markers, including one still streaming in", () => {
    expect(stripMarkers("Click Save [[point: Save]] to keep it.")).toBe("Click Save to keep it.");
    expect(stripMarkers("Click Save [[point: Sa")).toBe("Click Save");
    expect(stripMarkers("Click Save [")).toBe("Click Save");
  });

  it("shows what an answer pointed at and replays it on click", async () => {
    const transport = new MockTransport().handle("conversation.showGuidance", () => ({}));
    setIpcClient(new IpcClient(transport));
    const m: Message = {
      id: "0199c2a8-0000-7000-8000-0000000000b1",
      conversationId: "c",
      role: "assistant",
      content: "Click Save to keep your changes.",
      status: "complete",
      createdAtMs: 1,
      guidance: [{ label: "Save", rect: { x: 1, y: 1, width: 10, height: 10 }, source: "element" }],
    };
    render(
      <MemoryRouter>
        <MessageList messages={[m]} />
      </MemoryRouter>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(
        transport.sent.find((e) => e.kind === "request" && e.method === "conversation.showGuidance"),
      ).toMatchObject({
        params: { id: m.id },
      }),
    );
  });
});
