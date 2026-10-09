import { IpcClient } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { PermissionsPage } from "../src/features/permissions/PermissionsPage";
import { setIpcClient } from "../src/services/ipc";
import { usePermissionsStore } from "../src/stores/permissionsStore";

describe("Permissions page", () => {
  let transport: MockTransport;
  let ax: "enabled" | "needsAttention" | "notRequested";

  beforeEach(() => {
    ax = "notRequested";
    transport = new MockTransport()
      .handle("host.mic.permission", () => ({ state: "enabled" }))
      .handle("host.screen.permission", () => ({ screenRecording: "needsAttention" }))
      .handle("host.ax.status", () => ({ state: ax }))
      .handle("host.ax.requestPermission", () => {
        ax = "enabled";
        return { state: ax };
      })
      .handle("host.ax.snapshot", () => ({
        application: { name: "Safari", bundleId: "com.apple.Safari", pid: 9 },
        window: { id: 0, title: "Apple", frame: { x: 0, y: 0, width: 1200, height: 800 } },
        underPointer: {
          role: "Button",
          title: "Submit",
          enabled: true,
          focused: false,
          selected: false,
          depth: 0,
          frame: { x: 820, y: 440, width: 120, height: 32 },
          actions: ["Press"],
        },
        ancestors: [
          { role: "Group", title: "Checkout", enabled: true, focused: false, selected: false, depth: 0 },
        ],
        truncated: false,
        elapsedMs: 12,
      }));
    setIpcClient(new IpcClient(transport));
    usePermissionsStore.setState({ states: {}, checking: false });
  });

  it("shows the real status of each permission and what works", async () => {
    render(<PermissionsPage />);
    await waitFor(() => expect(screen.getByText("Enabled")).toBeInTheDocument());
    expect(screen.getByText("Needs attention")).toBeInTheDocument();
    expect(screen.getByText("Not requested")).toBeInTheDocument();
    expect(screen.getByText("Understand app controls under the pointer").parentElement).toHaveTextContent(
      "needs Accessibility",
    );
  });

  it("requests accessibility and re-checks", async () => {
    render(<PermissionsPage />);
    await waitFor(() => expect(screen.getAllByRole("button", { name: "Allow" }).length).toBeGreaterThan(0));
    const allow = screen.getAllByRole("button", { name: "Allow" }).at(-1)!;
    await userEvent.click(allow);
    await waitFor(() => expect(usePermissionsStore.getState().states.accessibility).toBe("enabled"));
  });

  it("inspects the element under the pointer once allowed", async () => {
    ax = "enabled";
    render(<PermissionsPage />);
    const btn = await screen.findByRole("button", { name: "Inspect Under Pointer" });
    await waitFor(() => expect(btn).toBeEnabled());
    await userEvent.click(btn);
    expect(await screen.findByText("Button “Submit”", {}, { timeout: 5000 })).toBeInTheDocument();
    expect(screen.getByText("Press")).toBeInTheDocument();
    expect(screen.getByText(/Checkout/)).toBeInTheDocument();
  }, 10_000);
});
