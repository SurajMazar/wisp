import { IpcClient, type AgentRun } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { AgentRunCard } from "../src/components/agents/AgentRunCard";
import { Composer } from "../src/components/chat/Composer";
import { AgentsPage } from "../src/features/agents/AgentsPage";
import { setIpcClient } from "../src/services/ipc";
import { useAgentStore } from "../src/stores/agentStore";

const agent = {
  id: "0199c2a8-0000-7000-8000-0000000000c9",
  name: "Desktop Helper",
  avatar: "🧭",
  description: "Opens apps.",
  instructions: "",
  tools: ["click"],
  skills: [],
  limits: { maxSteps: 15, maxMinutes: 10, maxToolCalls: 25, maxTokens: 60000 },
  enabled: true,
  createdAtMs: 1,
  updatedAtMs: 1,
};

const run = (over: Partial<AgentRun> = {}): AgentRun => ({
  id: "r1",
  agentName: "Desktop Helper",
  avatar: "🧭",
  goal: "Save the file",
  status: "running",
  plan: [
    { title: "Open the File menu", status: "done" },
    { title: "Click Save", status: "active" },
    { title: "Check it saved", status: "pending" },
  ],
  steps: [
    { seq: 1, kind: "observe", title: "Looking at the screen", status: "succeeded" },
    { seq: 2, kind: "act", title: "Click “File”", status: "succeeded", detail: "the menu opened" },
    { seq: 3, kind: "act", title: "Click “Save”", status: "running" },
  ],
  stepsUsed: 2,
  toolCallsUsed: 1,
  tokensUsed: 900,
  limits: agent.limits,
  startedAtMs: 1,
  ...over,
});

describe("Agents", () => {
  let transport: MockTransport;
  beforeEach(() => {
    transport = new MockTransport()
      .handle("agent.list", () => ({ agents: [] }))
      .handle("agent.runs", () => ({ runs: [] }))
      .handle("models.list", () => ({ providers: [], models: [] }))
      .handle("tool.list", () => ({ tools: [] }))
      .handle("agent.save", (p) => ({
        ...agent,
        name: p.name,
        avatar: p.avatar || "🤖",
        tools: p.tools ?? [],
      }))
      .handle("agent.run", (p) => run({ goal: p.goal, status: "planning", plan: [], steps: [] }))
      .handle("agent.control", () => ({}));
    setIpcClient(new IpcClient(transport));
    useAgentStore.setState({ agents: [], runs: {}, dismissed: [], error: null });
  });

  it("starts from a template and runs an agent with a goal", async () => {
    render(<AgentsPage />);
    await userEvent.click(await screen.findByRole("button", { name: /Desktop Helper/ }));
    expect(await screen.findByText("Desktop Helper", { selector: "p" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Run" }));
    await userEvent.type(
      screen.getByRole("textbox", { name: "Goal for Desktop Helper" }),
      "Save the file{Enter}",
    );
    await waitFor(() =>
      expect(transport.sent.find((e) => e.kind === "request" && e.method === "agent.run")).toMatchObject({
        params: { goal: "Save the file", agentId: agent.id },
      }),
    );
    expect(await screen.findByRole("region", { name: "Desktop Helper: Save the file" })).toBeInTheDocument();
  });

  it("shows the plan and actions live, with pause and stop", async () => {
    render(<AgentRunCard run={run()} />);
    expect(screen.getByRole("list", { name: "Plan" })).toHaveTextContent("Open the File menu");
    expect(screen.getByText("Click “File”")).toBeInTheDocument();
    expect(screen.getByText(/the menu opened/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Pause" }));
    await userEvent.click(screen.getByRole("button", { name: "Stop" }));
    await waitFor(() =>
      expect(
        transport.sent.flatMap((e) =>
          e.kind === "request" && e.method === "agent.control" ? [e.params] : [],
        ),
      ).toEqual([
        { runId: "r1", action: "pause" },
        { runId: "r1", action: "stop" },
      ]),
    );
  });

  it("asks to continue when stuck and accepts an answer to a question", async () => {
    const { rerender } = render(
      <AgentRunCard
        run={run({
          status: "stuck",
          note: "The assistant may be stuck. It has repeated the same action several times.",
        })}
      />,
    );
    expect(screen.getByText(/may be stuck/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Continue" }));
    rerender(<AgentRunCard run={run({ status: "needsInput", note: "Which folder should I save it in?" })} />);
    await userEvent.type(screen.getByRole("textbox", { name: "Your answer" }), "Documents{Enter}");
    await waitFor(() =>
      expect(
        transport.sent.flatMap((e) =>
          e.kind === "request" && e.method === "agent.control" ? [e.params] : [],
        ),
      ).toEqual([
        { runId: "r1", action: "continue" },
        { runId: "r1", action: "continue", reply: "Documents" },
      ]),
    );
  });

  it("sends in Do it mode", async () => {
    const sent: [string, unknown][] = [];
    render(<Composer streaming={false} onSend={(t, o) => sent.push([t, o])} />);
    await userEvent.click(screen.getByRole("button", { name: "Do it" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Message" }), "make a note{Enter}");
    expect(sent).toEqual([["make a note", { act: true }]]);
    act(() => {});
  });
});
