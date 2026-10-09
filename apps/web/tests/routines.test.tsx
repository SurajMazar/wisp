import { IpcClient } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";
import { RoutinesPage } from "../src/features/routines/RoutinesPage";
import { setIpcClient } from "../src/services/ipc";
import { useRoutineStore } from "../src/stores/routineStore";

const routine = {
  id: "0199c2a8-0000-7000-8000-0000000000b7",
  name: "Morning summary",
  schedule: "0 9 * * 1-5",
  scheduleText: "Weekdays at 09:00",
  timezone: "Asia/Kathmandu",
  prompt: "Summarize my notes",
  enabled: true,
  retry: { maxAttempts: 2, backoffMinutes: 5 },
  notify: { onSuccess: true, onFailure: true },
  nextRunAtMs: Date.UTC(2026, 9, 12, 3, 15),
  lastStatus: "succeeded" as const,
  createdAtMs: 1,
  updatedAtMs: 1,
};

describe("Routines", () => {
  let transport: MockTransport;
  beforeEach(() => {
    transport = new MockTransport()
      .handle("routine.list", () => ({ routines: [routine] }))
      .handle("agent.list", () => ({ agents: [] }))
      .handle("routine.save", (p) => ({
        ...routine,
        name: p.name,
        prompt: p.prompt,
        schedule: p.schedule,
        id: p.id ?? "new",
      }))
      .handle("routine.runNow", () => ({
        id: "run1",
        routineId: routine.id,
        attempt: 1,
        status: "running",
        scheduledForMs: 1,
        startedAtMs: Date.now(),
      }))
      .handle("routine.runs", () => ({
        runs: [
          {
            id: "run0",
            routineId: routine.id,
            attempt: 2,
            status: "succeeded",
            conversationId: "c1",
            scheduledForMs: 1,
            startedAtMs: 1,
            finishedAtMs: 2,
          },
        ],
      }));
    setIpcClient(new IpcClient(transport));
    useRoutineStore.setState({ routines: [], runs: {}, error: null });
  });

  it("lists routines with their schedule and history", async () => {
    render(
      <MemoryRouter>
        <RoutinesPage />
      </MemoryRouter>,
    );
    expect(await screen.findByText("Morning summary")).toBeInTheDocument();
    expect(
      screen.getByText(/Weekdays at 09:00 · Asia\/Kathmandu · next .* · last worked/),
    ).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "History" }));
    expect(await screen.findByRole("link", { name: "See result" })).toHaveAttribute(
      "href",
      "/conversations/c1",
    );
    expect(screen.getByText(/Worked \(try 2\)/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Run now" }));
    await waitFor(() =>
      expect(transport.sent.some((e) => e.kind === "request" && e.method === "routine.runNow")).toBe(true),
    );
  });

  it("creates a routine from a preset in the user's timezone", async () => {
    render(
      <MemoryRouter>
        <RoutinesPage />
      </MemoryRouter>,
    );
    await userEvent.click(await screen.findByRole("button", { name: "New routine" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Name" }), "Check-in");
    await userEvent.type(screen.getByRole("textbox", { name: "Prompt" }), "Anything new?");
    await userEvent.selectOptions(screen.getByRole("combobox", { name: "When" }), "Every 5 minutes");
    expect(screen.getByRole("textbox", { name: "Cron" })).toHaveValue("*/5 * * * *");
    await userEvent.click(screen.getByRole("button", { name: "Save routine" }));
    await waitFor(() =>
      expect(transport.sent.find((e) => e.kind === "request" && e.method === "routine.save")).toMatchObject({
        params: {
          name: "Check-in",
          prompt: "Anything new?",
          schedule: "*/5 * * * *",
          timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
        },
      }),
    );
  });
});
