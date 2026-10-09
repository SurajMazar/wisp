import { IpcClient, WispError, type SkillSummary } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { contextLines } from "../src/components/chat/ContextInfo";
import { SkillsPage } from "../src/features/skills/SkillsPage";
import { setIpcClient } from "../src/services/ipc";
import { useSkillStore } from "../src/stores/skillStore";

const sample: SkillSummary = {
  id: "0199c2a8-0000-7000-8000-0000000000e5",
  slug: "mac-keyboard-shortcuts",
  name: "Mac keyboard shortcuts",
  description: "Suggest and teach the standard macOS keyboard shortcuts.",
  source: "builtin",
  enabled: true,
  activation: "auto",
  triggers: ["shortcut", "keyboard"],
  version: 1,
  updatedAtMs: 1,
};

describe("Skills", () => {
  let transport: MockTransport;
  beforeEach(() => {
    transport = new MockTransport()
      .handle("skill.list", () => ({ skills: [sample] }))
      .handle("skill.setEnabled", (p) => ({ ...sample, enabled: p.enabled }))
      .handle("skill.get", () => ({
        summary: sample,
        content: "---\nname: Mac keyboard shortcuts\n---\nBody",
      }))
      .handle("skill.versions", () => ({ versions: [] }))
      .handle("skill.save", () => {
        throw new WispError({ code: "invalidParams", message: "The front matter needs a name." });
      });
    setIpcClient(new IpcClient(transport));
    useSkillStore.setState({ skills: [], error: null });
  });

  it("lists skills with how they're used and toggles them", async () => {
    render(<SkillsPage />);
    expect(await screen.findByText("Mac keyboard shortcuts")).toBeInTheDocument();
    expect(screen.getByText(/Built in · Used when relevant · shortcut, keyboard · v1/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("switch", { name: "Use Mac keyboard shortcuts" }));
    await waitFor(() =>
      expect(
        transport.sent.find((e) => e.kind === "request" && e.method === "skill.setEnabled"),
      ).toMatchObject({
        params: { id: sample.id, enabled: false },
      }),
    );
  });

  it("edits a skill and explains why a save failed", async () => {
    render(<SkillsPage />);
    await userEvent.click(await screen.findByRole("button", { name: "Edit Mac keyboard shortcuts" }));
    const area = await screen.findByRole("textbox", { name: "SKILL.md" });
    expect(area).toHaveValue("---\nname: Mac keyboard shortcuts\n---\nBody");
    await userEvent.click(screen.getByRole("button", { name: "Save skill" }));
    expect(await screen.findByText("The front matter needs a name.")).toBeInTheDocument();
  });

  it("the context disclosure names the skills used", () => {
    expect(
      contextLines({
        screenTextLines: 0,
        uiElements: 0,
        selectedText: false,
        reusedScreen: false,
        historyMessages: 0,
        summarizedMessages: 0,
        estimatedTokens: 10,
        skills: ["Mac keyboard shortcuts"],
      }),
    ).toEqual(["Skill: Mac keyboard shortcuts"]);
  });
});
