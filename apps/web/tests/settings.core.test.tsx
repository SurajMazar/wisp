/**
 * Phase 1 required test, minus the Swift hop:
 *   React → IPC → Rust (real wisp-core) → SQLite, restart the core, verify persistence.
 * The Swift hop is covered by apps/macos tests and the manual launch checklist.
 */
import { IpcClient } from "@wisp/contracts";
import { startCore, type CoreProcess } from "@wisp/test-utils/node";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, describe, expect, it } from "vitest";
import { SettingsPage } from "../src/features/settings/SettingsPage";
import { setIpcClient } from "../src/services/ipc";
import { useSettingsStore } from "../src/stores/settingsStore";

const dataDir = mkdtempSync(join(tmpdir(), "wisp-web-it-"));
let core: CoreProcess | null = null;

afterAll(async () => {
  await core?.stop();
  rmSync(dataDir, { recursive: true, force: true });
});

async function boot() {
  core = await startCore(dataDir);
  setIpcClient(new IpcClient(core.transport));
  useSettingsStore.setState({ settings: null, status: "idle", error: null, saving: null });
  await useSettingsStore.getState().load();
}

describe("settings persistence through the real core", () => {
  it("survives a core restart", async () => {
    await boot();
    const { unmount } = render(<SettingsPage />);
    await userEvent.click(screen.getByRole("radio", { name: "Dark" }));
    await userEvent.click(screen.getByRole("radio", { name: "Local Preferred" }));
    await waitFor(() => expect(useSettingsStore.getState().settings?.privacy.mode).toBe("localPreferred"));
    unmount();
    expect(await core!.stop()).toBe(0);

    await boot();
    render(<SettingsPage />);
    expect(screen.getByRole("radio", { name: "Dark" })).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("radio", { name: "Local Preferred" })).toHaveAttribute("aria-checked", "true");
  });
});
