import { IpcClient, WispError, type AppSettings, type InvocationStatus } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { InvocationSettingsGroup } from "../src/features/settings/InvocationSettingsGroup";
import { formatShortcut } from "../src/lib/shortcut";
import { setIpcClient } from "../src/services/ipc";
import { useInvocationStore } from "../src/stores/invocationStore";
import { useSettingsStore } from "../src/stores/settingsStore";

const base: AppSettings = {
  general: { launchAtLogin: false, showMenuBarIcon: true },
  appearance: { theme: "system", accent: "default" },
  privacy: { mode: "localOnly" },
  onboarding: { completed: false },
  invocation: {
    shortcut: { modifiers: ["option", "command"] },
    behavior: "hold",
    mouseButton: "none",
    mouseAction: "focusCursor",
    holdDelayMs: 150,
  },
};

const status = (keyboard: InvocationStatus["keyboard"]): InvocationStatus => ({
  keyboard,
  mouse: "disabled",
  trackpad: "unsupported",
  menuBar: "active",
  eventTapPermitted: keyboard !== "needsPermission",
});

describe("Invocation settings", () => {
  let transport: MockTransport;
  let stored: AppSettings;

  beforeEach(async () => {
    stored = structuredClone(base);
    transport = new MockTransport()
      .handle("settings.get", () => stored)
      .handle("settings.update", ({ update }) => {
        stored = { ...stored, [update.section]: update.value } as AppSettings;
        return stored;
      })
      .handle("host.invocation.status", () => status("active"));
    setIpcClient(new IpcClient(transport));
    useInvocationStore.setState({ status: null, recording: false, conflicts: [], message: null });
    await useSettingsStore.getState().load();
    await useInvocationStore.getState().loadStatus();
  });

  const renderGroup = () =>
    render(<InvocationSettingsGroup invocation={useSettingsStore.getState().settings!.invocation} />);

  it("formats shortcuts in macOS order", () => {
    expect(formatShortcut({ modifiers: ["command", "option"] })).toBe("⌥⌘");
    expect(formatShortcut({ modifiers: ["control", "option"], keyCode: 49, keyLabel: "Space" })).toBe(
      "⌃⌥Space",
    );
  });

  it("shows the default ⌥⌘ shortcut as active", () => {
    renderGroup();
    expect(screen.getByLabelText("Option Command")).toHaveTextContent("⌥⌘");
    expect(screen.getAllByText("Active").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("Not supported by macOS")).toBeInTheDocument();
  });

  it("guides the user to grant permission when the event tap is unavailable", async () => {
    transport.handle("host.invocation.status", () => ({
      ...status("needsPermission"),
      detail: "Allow Accessibility.",
    }));
    transport.handle("host.system.openSettingsPane", () => ({}));
    await useInvocationStore.getState().loadStatus();
    renderGroup();
    await userEvent.click(screen.getByRole("button", { name: "Open System Settings" }));
    expect(
      transport.sent.some((e) => e.kind === "request" && e.method === "host.system.openSettingsPane"),
    ).toBe(true);
  });

  it("saves a recorded shortcut without conflicts", async () => {
    transport.handle("host.shortcut.record", () => ({
      shortcut: { modifiers: ["option"], keyCode: 49, keyLabel: "Space" },
      conflicts: [],
    }));
    renderGroup();
    await userEvent.click(screen.getByRole("button", { name: "Record New Shortcut" }));
    await waitFor(() => expect(stored.invocation.shortcut.keyLabel).toBe("Space"));
  });

  it("refuses to save a shortcut macOS already uses", async () => {
    transport.handle("host.shortcut.record", () => ({
      shortcut: { modifiers: ["command"], keyCode: 49, keyLabel: "Space" },
      conflicts: [{ severity: "blocking", with: "macOS: Spotlight" }],
    }));
    renderGroup();
    await userEvent.click(screen.getByRole("button", { name: "Record New Shortcut" }));
    expect(await screen.findByText(/wasn't saved/)).toBeInTheDocument();
    expect(screen.getByText(/Spotlight/)).toBeInTheDocument();
    expect(stored.invocation.shortcut.keyCode).toBeUndefined();
  });

  it("treats a cancelled recording quietly", async () => {
    transport.handle("host.shortcut.record", () => {
      throw new WispError({ code: "cancelled", message: "Recording cancelled." });
    });
    renderGroup();
    await userEvent.click(screen.getByRole("button", { name: "Record New Shortcut" }));
    await waitFor(() => expect(useInvocationStore.getState().recording).toBe(false));
    expect(stored.invocation.shortcut).toEqual(base.invocation.shortcut);
  });
});
