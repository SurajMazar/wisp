import { IpcClient, type ApprovalRequest, type Message } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";
import { MessageList } from "../src/components/chat/MessageList";
import { ApprovalStack } from "../src/components/tools/ApprovalCard";
import { ActivityPage } from "../src/features/activity/ActivityPage";
import { setIpcClient } from "../src/services/ipc";
import { useToolStore } from "../src/stores/toolStore";

const request = (over: Partial<ApprovalRequest> = {}): ApprovalRequest => ({
  id: "0199c2a8-0000-7000-8000-0000000000d1",
  tool: "click",
  title: "Click “Save”",
  risk: "low",
  choices: ["allowOnce", "allowSession", "alwaysAllow", "deny"],
  expiresAtMs: Date.now() + 120_000,
  ...over,
});

describe("Computer control UI", () => {
  let transport: MockTransport;
  beforeEach(() => {
    transport = new MockTransport()
      .handle("tool.respond", () => ({}))
      .handle("tool.grants", () => ({ grants: [], session: [] }))
      .handle("tool.activity", () => ({
        items: [
          {
            id: "a1",
            tool: "click",
            title: "Click “Delete”",
            risk: "high",
            decision: "deny",
            status: "denied",
            createdAtMs: Date.now(),
          },
          {
            id: "a2",
            tool: "click",
            title: "Click “Save”",
            risk: "low",
            decision: "allowOnce",
            status: "succeeded",
            verification: { status: "verified", detail: "Wisp pressed “Save”; the focus moved" },
            createdAtMs: Date.now(),
          },
        ],
      }))
      .handle("tool.run", () => ({
        record: {
          id: "r1",
          tool: "click",
          title: "Click “Save”",
          risk: "low",
          decision: "allowOnce",
          status: "succeeded",
          verification: { status: "verified", detail: "Wisp pressed “Save”; the window changed" },
          createdAtMs: 1,
        },
      }));
    setIpcClient(new IpcClient(transport));
    useToolStore.setState({ pending: [], activity: [], grants: null, error: null });
  });

  it("asks with every choice for low-risk actions and sends the answer", async () => {
    render(<ApprovalStack />);
    act(() => useToolStore.getState().receiveRequest(request()));
    expect(screen.getByRole("alertdialog", { name: "Wisp wants to: Click “Save”" })).toBeInTheDocument();
    for (const name of ["Allow once", "Allow for this session", "Always allow", "Deny"]) {
      expect(screen.getByRole("button", { name })).toBeInTheDocument();
    }
    await userEvent.click(screen.getByRole("button", { name: "Allow for this session" }));
    await waitFor(() =>
      expect(transport.sent.find((e) => e.kind === "request" && e.method === "tool.respond")).toMatchObject({
        params: { decision: "allowSession" },
      }),
    );
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("shows risky actions with fewer choices and the reason", () => {
    render(<ApprovalStack />);
    act(() =>
      useToolStore.getState().receiveRequest(
        request({
          title: "Click “Empty Trash”",
          risk: "critical",
          riskReason: "“Empty Trash” sounds irreversible",
          choices: ["allowOnce", "deny"],
        }),
      ),
    );
    expect(screen.getByText("Irreversible")).toBeInTheDocument();
    expect(screen.getByText(/Careful: “Empty Trash” sounds irreversible/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Always allow" })).not.toBeInTheDocument();
    // Resolved elsewhere (or expired) → it goes away here too.
    act(() => useToolStore.getState().receiveResolved(request().id));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("clicks what an answer pointed at, through the pipeline", async () => {
    const m: Message = {
      id: "0199c2a8-0000-7000-8000-0000000000b2",
      conversationId: "c",
      role: "assistant",
      content: "Click Save.",
      status: "complete",
      createdAtMs: 1,
      guidance: [{ label: "Save", rect: { x: 100, y: 200, width: 80, height: 20 }, source: "element" }],
    };
    render(
      <MemoryRouter>
        <MessageList messages={[m]} />
      </MemoryRouter>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Click “Save” for me" }));
    await waitFor(() =>
      expect(transport.sent.find((e) => e.kind === "request" && e.method === "tool.run")).toMatchObject({
        params: { call: { tool: "click", args: { x: 140, y: 210, label: "Save" } }, messageId: m.id },
      }),
    );
    expect(await screen.findByText("Done — Wisp pressed “Save”; the window changed")).toBeInTheDocument();
  });

  it("lists activity with risk, outcome and verification", async () => {
    render(<ActivityPage />);
    expect(await screen.findByText("Click “Delete”")).toBeInTheDocument();
    expect(screen.getByText("Not allowed")).toBeInTheDocument();
    expect(screen.getByText(/Wisp pressed “Save”; the focus moved/)).toBeInTheDocument();
  });
});
