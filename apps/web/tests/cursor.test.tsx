import { IpcClient, type AppSettings } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { CursorSettingsGroup } from "../src/features/settings/CursorSettingsGroup";
import { setIpcClient } from "../src/services/ipc";
import { useSettingsStore } from "../src/stores/settingsStore";
import { defaultSettings } from "./fixtures";

describe("Assistant cursor settings", () => {
  let transport: MockTransport;
  let stored: AppSettings;

  beforeEach(async () => {
    stored = defaultSettings();
    transport = new MockTransport()
      .handle("settings.get", () => stored)
      .handle("settings.update", ({ update }) => {
        stored = { ...stored, [update.section]: update.value } as AppSettings;
        return stored;
      })
      .handle("host.cursor.preview", () => ({}));
    setIpcClient(new IpcClient(transport));
    await useSettingsStore.getState().load();
  });

  const renderGroup = () =>
    render(<CursorSettingsGroup cursor={useSettingsStore.getState().settings!.cursor} />);

  it("defaults to an always-visible buddy", () => {
    expect(stored.cursor.enabled).toBe(true);
    expect(stored.cursor.visibility).toBe("always");
    renderGroup();
    expect(screen.getByRole("switch", { name: "Show assistant cursor" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  it("saves color and visibility changes", async () => {
    renderGroup();
    await userEvent.click(screen.getByRole("radio", { name: "Cursor Green" }));
    await waitFor(() => expect(stored.cursor.theme).toBe("green"));
    await userEvent.click(screen.getByRole("radio", { name: "When summoned" }));
    await waitFor(() => expect(stored.cursor.visibility).toBe("whenSummoned"));
  });

  it("previews states locally and on screen", async () => {
    renderGroup();
    await userEvent.click(screen.getByRole("button", { name: "Thinking" }));
    expect(screen.getByText("Thinking…")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Preview on screen" }));
    await waitFor(() =>
      expect(transport.sent.some((e) => e.kind === "request" && e.method === "host.cursor.preview")).toBe(
        true,
      ),
    );
  });

  it("keeps the label within 24 characters", async () => {
    renderGroup();
    await userEvent.click(screen.getByRole("switch", { name: "Show cursor label" }));
    await waitFor(() => expect(stored.cursor.showLabel).toBe(true));
    const input = screen.getByRole("textbox", { name: "Cursor label" });
    await userEvent.clear(input);
    await userEvent.type(input, "My very own helpful little buddy{Enter}");
    await waitFor(() => expect(stored.cursor.label.length).toBeLessThanOrEqual(24));
    expect(stored.cursor.label.startsWith("My very own")).toBe(true);
  });
});

describe("Cursor shape and glow", () => {
  let transport: MockTransport;
  let stored: AppSettings;

  beforeEach(async () => {
    stored = defaultSettings();
    transport = new MockTransport()
      .handle("settings.get", () => stored)
      .handle("settings.update", ({ update }) => {
        stored = { ...stored, [update.section]: update.value } as AppSettings;
        return stored;
      });
    setIpcClient(new IpcClient(transport));
    await useSettingsStore.getState().load();
  });

  it("defaults to the Wisp shape with an Aurora gradient glow", () => {
    expect(stored.cursor.shape).toBe("wisp");
    expect(stored.cursor.glowMode).toBe("gradient");
    render(<CursorSettingsGroup cursor={stored.cursor} />);
    expect(screen.getByRole("radio", { name: "Wisp shape" })).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("radio", { name: "Aurora glow" })).toHaveAttribute("aria-checked", "true");
  });

  it("changes shape and glow gradient", async () => {
    render(<CursorSettingsGroup cursor={stored.cursor} />);
    await userEvent.click(screen.getByRole("radio", { name: "Pointer shape" }));
    await waitFor(() => expect(stored.cursor.shape).toBe("pointer"));
    await userEvent.click(screen.getByRole("radio", { name: "Sunset glow" }));
    await waitFor(() =>
      expect([stored.cursor.glowFrom, stored.cursor.glowTo]).toEqual(["#F97316", "#EC4899"]),
    );
    await userEvent.click(screen.getByRole("radio", { name: "Cursor color" }));
    await waitFor(() => expect(stored.cursor.glowMode).toBe("accent"));
  });
});
