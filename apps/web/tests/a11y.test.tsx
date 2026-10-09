import { IpcClient } from "@wisp/contracts";
import { MockTransport } from "@wisp/test-utils";
import { render, waitFor } from "@testing-library/react";
import { computeAccessibleName } from "dom-accessibility-api";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import { ActivityPage } from "../src/features/activity/ActivityPage";
import { AgentsPage } from "../src/features/agents/AgentsPage";
import { AssistantPanel } from "../src/features/assistant/AssistantPanel";
import { ConversationsPage } from "../src/features/conversations/ConversationsPage";
import { FilesPage } from "../src/features/files/FilesPage";
import { HomePage } from "../src/features/home/HomePage";
import { MemoryPage } from "../src/features/memory/MemoryPage";
import { OnboardingFlow } from "../src/features/onboarding/OnboardingFlow";
import { RoutinesPage } from "../src/features/routines/RoutinesPage";
import { SkillsPage } from "../src/features/skills/SkillsPage";
import { setIpcClient } from "../src/services/ipc";

const pages = {
  Home: <HomePage />,
  Conversations: <ConversationsPage />,
  Panel: <AssistantPanel />,
  Agents: <AgentsPage />,
  Skills: <SkillsPage />,
  Memory: <MemoryPage />,
  Files: <FilesPage />,
  Routines: <RoutinesPage />,
  Activity: <ActivityPage />,
  Onboarding: <OnboardingFlow />,
};

describe("Every screen", () => {
  for (const [name, page] of Object.entries(pages)) {
    it(`${name}: renders even when the engine fails, and every control has a name`, async () => {
      // No handlers: every call fails, exercising error and empty states.
      setIpcClient(new IpcClient(new MockTransport()));
      const { container } = render(<MemoryRouter>{page}</MemoryRouter>);
      await waitFor(() => expect(container.childElementCount).toBeGreaterThan(0));
      await new Promise((r) => setTimeout(r, 50));
      const unnamed = [
        ...container.querySelectorAll("button, input, textarea, select, [role=switch], [role=radio]"),
      ]
        .filter(
          (el) =>
            !(el as HTMLElement).hidden &&
            (el as HTMLInputElement).type !== "hidden" &&
            !el.closest(".hidden"),
        )
        .filter((el) => computeAccessibleName(el).trim() === "")
        .map((el) => el.outerHTML.slice(0, 120));
      expect(unnamed).toEqual([]);
      const images = [...container.querySelectorAll("img")].filter((i) => !i.hasAttribute("alt"));
      expect(images).toEqual([]);
    });
  }
});
