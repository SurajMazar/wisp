import type { FocusRegion } from "@wisp/contracts";

export interface QuickAction {
  label: string;
  prompt: string;
}

/** Text the user selected in the app, if Accessibility reported it. */
export function selectedText(r: FocusRegion): string | null {
  const m = /^selected text: "([\s\S]*)"$/m.exec(r.accessibility?.summary ?? "");
  return m?.[1]?.trim() || null;
}

const ERROR =
  /\b(error|exception|failed|failure|fatal|denied|traceback|panicked|not found|cannot|can't|unable to|invalid)\b/i;
const CODE_KEYWORDS =
  /\b(function|const|let|var|def|class|import|return|fn|func|public|private|async|await|struct|impl|#include|SELECT|FROM)\b/;

function looksLikeCode(text: string): boolean {
  const lines = text.split("\n").filter((l) => l.trim());
  if (lines.length === 0) return false;
  const codey = lines.filter(
    (l) => /[{};]\s*$|=>|\(\)|::|^\s{2,}\S|^\s*(\/\/|#)/.test(l) || CODE_KEYWORDS.test(l),
  );
  return codey.length >= Math.max(2, lines.length * 0.3);
}

/** Context-aware quick actions for a selection (spec §94–95). */
export function quickActions(r: FocusRegion): QuickAction[] {
  const selected = selectedText(r);
  const screenText = (r.text?.lines ?? []).map((l) => l.text).join("\n");
  const text = selected ?? screenText;
  if (ERROR.test(text)) {
    return [
      { label: "Explain error", prompt: "Explain this error in plain words." },
      { label: "Fix error", prompt: "How do I fix this error? Give me the steps." },
      { label: "Why did this happen?", prompt: "Why did this happen?" },
    ];
  }
  if (looksLikeCode(text)) {
    return [
      { label: "Explain code", prompt: "Explain what this code does." },
      { label: "Find bugs", prompt: "Are there any bugs in this code?" },
      { label: "Optimize", prompt: "How could this code be simpler or faster?" },
      { label: "Write tests", prompt: "Write tests for this code." },
    ];
  }
  if (selected) {
    return [
      { label: "Explain", prompt: "Explain the selected text." },
      { label: "Summarize", prompt: "Summarize the selected text." },
      { label: "Rewrite", prompt: "Rewrite the selected text to be clearer." },
      {
        label: "Translate",
        prompt: "Translate the selected text into English (or into Spanish if it's already English).",
      },
    ];
  }
  return [
    { label: "What is this?", prompt: "What am I looking at?" },
    { label: "Where should I click?", prompt: "What should I click here, and where is it?" },
    { label: "Summarize", prompt: "Summarize what's shown here." },
    { label: "Teach me", prompt: "Teach me how to use this, one step at a time." },
  ];
}
