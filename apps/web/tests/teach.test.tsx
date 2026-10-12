import { IpcClient, type Lesson } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { Composer } from "../src/components/chat/Composer";
import { LessonCard } from "../src/components/teach/LessonCard";
import { setIpcClient } from "../src/services/ipc";
import { useTeachStore } from "../src/stores/teachStore";

const lesson = (over: Partial<Lesson> = {}): Lesson => ({
  id: "0199c2a8-0000-7000-8000-0000000000f9",
  conversationId: "c1",
  goal: "save this document",
  status: "waitingForUser",
  watching: true,
  startedAtMs: 1,
  steps: [
    {
      instruction: "Open the File menu.",
      status: "done",
      confirmed: "Wisp saw the screen change",
      messageId: "m1",
    },
    {
      instruction: "Click Save [[point: Save]].",
      status: "current",
      messageId: "m2",
      target: {
        label: "Save",
        rect: { x: 10, y: 10, width: 20, height: 10 },
        source: "element",
        mark: "point",
      },
    },
  ],
  ...over,
});

describe("Teacher mode UI", () => {
  let transport: MockTransport;
  beforeEach(() => {
    transport = new MockTransport().handle("teach.control", () => ({}));
    setIpcClient(new IpcClient(transport));
    useTeachStore.setState({ lesson: null, dismissed: [], error: null });
  });

  it("shows the steps as a checklist and controls the lesson", async () => {
    render(<LessonCard />);
    act(() => useTeachStore.getState().receive(lesson()));
    expect(screen.getByRole("region", { name: "Lesson: save this document" })).toBeInTheDocument();
    expect(screen.getByText("Click Save.")).toBeInTheDocument(); // marker hidden
    expect(screen.getByText(/Wisp saw the screen change/)).toBeInTheDocument();
    expect(screen.getByText("Do it when you're ready — Wisp will notice.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "I did it" }));
    await userEvent.click(screen.getByRole("button", { name: "Show me again" }));
    await userEvent.click(screen.getByRole("button", { name: "Stop lesson" }));
    await waitFor(() =>
      expect(
        transport.sent.flatMap((e) =>
          e.kind === "request" && e.method === "teach.control" ? [e.params] : [],
        ),
      ).toEqual([
        { lessonId: lesson().id, action: "next" },
        { lessonId: lesson().id, action: "repeat" },
        { lessonId: lesson().id, action: "stop" },
      ]),
    );
  });

  it("only shows a lesson in its own conversation, and can be closed when it ends", async () => {
    const { rerender } = render(<LessonCard conversationId="other" />);
    act(() => useTeachStore.getState().receive(lesson({ status: "done", note: "Nice — it's saved!" })));
    expect(screen.queryByRole("region")).not.toBeInTheDocument();
    rerender(<LessonCard conversationId="c1" />);
    expect(screen.getByText("Nice — it's saved!")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "I did it" })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Close lesson" }));
    expect(screen.queryByRole("region")).not.toBeInTheDocument();
  });

  it("sends in Teach mode when the toggle is on", async () => {
    const sent: [string, unknown][] = [];
    render(<Composer streaming={false} onSend={(t, o) => sent.push([t, o])} />);
    await userEvent.click(screen.getByRole("button", { name: "Teach me" }));
    expect(screen.getByRole("textbox", { name: "Message" })).toHaveAttribute(
      "placeholder",
      "What do you want to learn? e.g. how to make a pivot table",
    );
    await userEvent.type(screen.getByRole("textbox", { name: "Message" }), "crop a photo{Enter}");
    expect(sent).toEqual([["crop a photo", { teach: true }]]);
    expect(screen.getByRole("button", { name: "Teach me" })).toHaveAttribute("aria-pressed", "false");
  });
});
