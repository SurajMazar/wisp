import { IpcClient, type FocusRegion } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { AssistantPanel } from "../src/features/assistant/AssistantPanel";
import { setIpcClient } from "../src/services/ipc";
import { useFocusStore } from "../src/stores/focusStore";

const region: FocusRegion = {
  id: "0199c2a8-0000-7000-8000-000000000001",
  kind: "rectangle",
  display: {
    id: 1,
    name: "Built-in Retina Display",
    frame: { x: 0, y: 0, width: 1512, height: 982 },
    scale: 2,
  },
  selection: { x: 100, y: 100, width: 320, height: 180 },
  crop: { x: 76, y: 76, width: 368, height: 228 },
  path: [],
  cursor: { x: 420, y: 280 },
  image: { url: "wisp://focus/0199c2a8-0000-7000-8000-000000000001.png", widthPx: 736, heightPx: 456 },
  application: { name: "Safari", bundleId: "com.apple.Safari", pid: 42 },
  window: { id: 7, title: "Docs", frame: { x: 0, y: 25, width: 1512, height: 900 } },
  createdAtMs: 1,
};

describe("Focus in the assistant panel", () => {
  let transport: MockTransport;

  beforeEach(() => {
    transport = new MockTransport()
      .handle("host.focus.start", () => ({}))
      .handle("host.focus.clear", () => ({}))
      .handle("host.screen.requestPermission", () => ({ screenRecording: "needsAttention" }))
      .handle("host.system.openSettingsPane", () => ({}));
    setIpcClient(new IpcClient(transport));
    useFocusStore.setState({ region: null, lastEnded: null, permission: "enabled", error: null });
  });

  const requests = (m: string) => transport.sent.filter((e) => e.kind === "request" && e.method === m);

  it("starts each kind of focus from the panel", async () => {
    render(<AssistantPanel />);
    for (const [label, kind] of [
      ["Area", "rectangle"],
      ["Point", "point"],
      ["Window", "window"],
      ["Screen", "screen"],
    ] as const) {
      await userEvent.click(screen.getByRole("button", { name: label }));
      await waitFor(() => expect(requests("host.focus.start").at(-1)).toMatchObject({ params: { kind } }));
    }
  });

  it("shows the captured region with exact pixel size and where it came from", () => {
    act(() => useFocusStore.getState().receive(region));
    render(<AssistantPanel />);
    expect(screen.getByRole("img", { name: "Captured area" })).toHaveAttribute("src", region.image!.url);
    expect(screen.getByText("320 × 180 pt · Built-in Retina Display · Docs")).toBeInTheDocument();
    expect(
      screen
        .getByText("320 × 180 pt · Built-in Retina Display · Docs")
        .closest("[title]")
        ?.getAttribute("title"),
    ).toMatch(/736 × 456 px/);
  });

  it("clears the focus", async () => {
    act(() => useFocusStore.getState().receive(region));
    render(<AssistantPanel />);
    await userEvent.click(screen.getByRole("button", { name: "Remove selection" }));
    expect(requests("host.focus.clear")).toHaveLength(1);
    act(() => useFocusStore.getState().cleared());
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("guides the user when screen access is off", async () => {
    act(() =>
      useFocusStore.getState().ended({ reason: "permissionDenied", detail: "Screen access is disabled." }),
    );
    render(<AssistantPanel />);
    await userEvent.click(screen.getByRole("button", { name: "Allow Screen Access" }));
    await waitFor(() => expect(requests("host.system.openSettingsPane")).toHaveLength(1));
    expect(requests("host.system.openSettingsPane")[0]).toMatchObject({
      params: { pane: "screenRecording" },
    });
  });
});
