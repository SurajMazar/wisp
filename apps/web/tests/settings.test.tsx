import { IpcClient, type AppSettings } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { defaultSettings } from "./fixtures";
import { SettingsPage } from "../src/features/settings/SettingsPage";
import { setIpcClient } from "../src/services/ipc";
import { useSettingsStore } from "../src/stores/settingsStore";

const defaults: AppSettings = defaultSettings();

describe("SettingsPage (mock core)", () => {
  let transport: MockTransport;
  let stored: AppSettings;

  beforeEach(() => {
    stored = structuredClone(defaults);
    transport = new MockTransport()
      .handle("settings.get", () => stored)
      .handle("settings.update", ({ update }) => {
        stored = { ...stored, [update.section]: update.value } as AppSettings;
        return stored;
      })
      .handle("host.invocation.status", () => ({
        keyboard: "active",
        mouse: "disabled",
        trackpad: "unsupported",
        menuBar: "active",
        eventTapPermitted: true,
      }));
    setIpcClient(new IpcClient(transport));
    useSettingsStore.setState({ settings: null, status: "idle", error: null, saving: null });
  });

  it("loads settings and saves a theme change through IPC", async () => {
    await useSettingsStore.getState().load();
    render(<SettingsPage />);
    await userEvent.click(screen.getByRole("radio", { name: "Dark" }));
    await waitFor(() => expect(stored.appearance.theme).toBe("dark"));
    expect(screen.getByRole("radio", { name: "Dark" })).toHaveAttribute("aria-checked", "true");
  });

  it("shows a friendly error with technical details when loading fails", async () => {
    transport.handle("settings.get", () => {
      throw new Error("disk I/O error");
    });
    await useSettingsStore.getState().load();
    render(<SettingsPage />);
    expect(screen.getByRole("alert")).toHaveTextContent("Settings couldn't be loaded.");
    await userEvent.click(screen.getByRole("button", { name: "Show Technical Details" }));
    expect(screen.getByText(/disk I\/O error/)).toBeInTheDocument();
  });
});
