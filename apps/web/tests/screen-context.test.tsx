import type { FocusRegion } from "@wisp/contracts";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { ContextButton, contextLines } from "../src/components/chat/ContextInfo";
import { quickActions, selectedText } from "../src/lib/quickActions";

const region = (lines: string[], summary = ""): FocusRegion => ({
  id: "0199c2a8-0000-7000-8000-0000000000a1",
  kind: "rectangle",
  display: { id: 1, name: "Built-in", frame: { x: 0, y: 0, width: 1512, height: 982 }, scale: 2 },
  selection: { x: 0, y: 0, width: 300, height: 200 },
  crop: { x: 0, y: 0, width: 300, height: 200 },
  cursor: { x: 1, y: 1 },
  accessibility: { nodes: [], summary },
  text: {
    elapsedMs: 1,
    lines: lines.map((text) => ({ text, frame: { x: 0, y: 0, width: 1, height: 1 }, confidence: 1 })),
  },
  createdAtMs: 1,
});

describe("Quick actions", () => {
  it("offers error actions for errors", () => {
    expect(quickActions(region(["Build failed", "error: cannot find module 'x'"]))[0]?.label).toBe(
      "Explain error",
    );
  });
  it("offers code actions for code", () => {
    const code = ["function add(a, b) {", "  return a + b;", "}"];
    expect(quickActions(region(code)).map((a) => a.label)).toContain("Find bugs");
  });
  it("offers text actions for a selection", () => {
    const r = region(["Lorem ipsum"], 'under pointer: text\nselected text: "The quick brown fox"\n');
    expect(selectedText(r)).toBe("The quick brown fox");
    expect(quickActions(r).map((a) => a.label)).toEqual(["Explain", "Summarize", "Rewrite", "Translate"]);
  });
  it("falls back to general actions", () => {
    expect(quickActions(region(["Settings", "General"]))[1]?.label).toBe("Where should I click?");
  });
});

describe("Context disclosure", () => {
  const info = {
    imageBytes: 90_000,
    screenTextLines: 12,
    uiElements: 1,
    selectedText: false,
    reusedScreen: false,
    historyMessages: 4,
    summarizedMessages: 6,
    estimatedTokens: 1840,
    textOnlyReason: null,
  };
  it("lists exactly what was sent", async () => {
    expect(contextLines(info)).toEqual([
      "Screenshot of your selection (88 KB, compressed)",
      "12 lines of text read from the screen",
      "1 UI element from the app",
      "4 earlier messages",
      "6 older messages, condensed",
    ]);
    render(<ContextButton context={{ ...info, textOnlyReason: "No model that can see images is ready." }} />);
    await userEvent.click(screen.getByRole("button", { name: "Context" }));
    expect(screen.getByText("No model that can see images is ready.")).toBeInTheDocument();
    expect(screen.getByText("About 1,840 tokens.")).toBeInTheDocument();
  });
});
