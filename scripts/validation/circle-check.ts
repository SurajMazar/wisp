/**
 * When you circle something and ask "What is this?", does Wisp name the thing you circled?
 * A document window with labelled buttons, unlabelled icons and plenty of text (so the old
 * text-only shortcut would kick in), a purple pen loop around one target at a time, through a
 * real `wisp-core` and your local model.
 *
 *   pnpm tsx scripts/validation/circle-check.ts [--model qwen3-vl:4b-instruct] [--tries 2]
 *       [--core <path to wisp-core>] [--old]   (--old: omit the ink flag, for older engines)
 */
import { spawn, execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { randomUUID } from "node:crypto";
import { root } from "./harness";

const arg = (k: string) => {
  const i = process.argv.indexOf(k);
  return i > -1 ? process.argv[i + 1] : undefined;
};
type Env = {
  kind: string;
  id?: string;
  event?: string;
  payload?: Record<string, unknown>;
  result?: unknown;
  error?: { message: string };
};

/** Loop bounds (points, top-left origin) and what a right answer mentions. */
const targets = [
  { name: "Export button", loop: [156, 46, 138, 64], expect: /export/i },
  { name: "gear icon", loop: [48, 126, 64, 68], expect: /gear|settings|cog|preferences/i },
  { name: "trash icon", loop: [168, 124, 64, 72], expect: /trash|delete|bin|garbage|remove/i },
  { name: "star icon", loop: [290, 128, 60, 68], expect: /star|favou?rite|bookmark/i },
  { name: "bell icon", loop: [410, 126, 60, 72], expect: /bell|notification|alert/i },
] as const;

const line = (text: string, x: number, y: number, width: number) => ({
  text,
  frame: { x, y, width, height: 20 },
  confidence: 0.95,
});
const screenText = [
  line("Q3 Report — Draft", 540, 10, 140),
  ...["Archive", "Export", "Share", "Delete"].map((l, i) => line(l, 66 + i * 130, 69, 60)),
  ...[
    "Revenue grew 18% quarter over quarter, driven by the enterprise plan and lower churn.",
    "Gross margin held at 71% despite higher hosting costs in the new European region.",
    "Hiring slowed in September; the support team is now fully staffed for the holidays.",
    "Next quarter: ship usage-based billing, renew the data-centre contract, close two deals.",
    "Risks: currency swings, a delayed SOC 2 audit, and one large customer up for renewal.",
    "Appendix A lists every metric with its definition; Appendix B has the raw numbers.",
  ].map((l, i) => line(l, 40, 240 + i * 34, 640)),
];

async function main() {
  const bin = arg("--core") ?? join(root, "target/release/wisp-core");
  if (!arg("--core"))
    execFileSync("cargo", ["build", "-q", "--release", "-p", "wisp-core"], { cwd: root, stdio: "inherit" });
  const data = mkdtempSync(join(tmpdir(), "wisp-circle-"));
  mkdirSync(join(data, "cache/focus"), { recursive: true });
  const core = spawn(bin, ["--data-dir", data], { stdio: ["pipe", "pipe", "ignore"] });
  const write = (e: unknown) => core.stdin!.write(JSON.stringify(e) + "\n");
  const waiting = new Map<string, (e: Env) => void>();
  const listeners: ((e: Env) => void)[] = [];
  const ready = new Promise<void>((resolve) =>
    createInterface({ input: core.stdout! }).on("line", (l) => {
      const e = JSON.parse(l) as Env;
      if (e.kind === "event" && e.event === "core.ready") resolve();
      if (e.kind === "response" && e.id && waiting.has(e.id)) {
        waiting.get(e.id)!(e);
        waiting.delete(e.id);
      }
      if (e.kind === "request" && e.id)
        write({ v: 1, kind: "response", id: e.id, error: { code: "unavailable", message: "no host" } });
      for (const f of listeners) f(e);
    }),
  );
  let n = 0;
  const call = <T>(method: string, params?: unknown) =>
    new Promise<T>((resolve, reject) => {
      const id = `cc:${++n}`;
      waiting.set(id, (e) => (e.error ? reject(new Error(e.error.message)) : resolve(e.result as T)));
      write({ v: 1, kind: "request", id, method, params: params ?? null });
    });
  await ready;
  const models = JSON.parse(
    execFileSync("sqlite3", [
      join(homedir(), "Library/Application Support/Wisp/database/wisp.sqlite3"),
      "select value from settings where key='models'",
    ]).toString(),
  );
  const model = arg("--model") ?? "qwen3-vl:4b-instruct";
  models.general = models.vision = { providerId: "ollama", model };
  models.endpoints = [];
  await call("settings.update", { update: { section: "models", value: models } });

  const tries = Number(arg("--tries") ?? 2);
  let right = 0;
  let total = 0;
  for (const t of targets) {
    for (let k = 0; k < tries; k++) {
      const id = randomUUID();
      const [x, y, w, h] = t.loop;
      execFileSync(
        "swift",
        [
          join(root, "scripts/validation/circle-screen.swift"),
          join(data, "cache/focus", `${id}.model.jpg`),
          ...[x, y, w, h].map(String),
        ],
        { stdio: "ignore" },
      );
      // The drawn loop, as the overlay records it.
      const path = Array.from({ length: 24 }, (_, i) => {
        const a = (i / 24) * 2 * Math.PI;
        return { x: x + w / 2 + (w / 2) * Math.cos(a), y: y + h / 2 + (h / 2) * Math.sin(a) };
      });
      write({
        v: 1,
        kind: "event",
        event: "focus.completed",
        payload: {
          id,
          kind: "lasso",
          ...(process.argv.includes("--old") ? {} : { ink: true }),
          createdAtMs: Date.now(),
          display: { id: 1, name: "Built-in", frame: { x: 0, y: 0, width: 1280, height: 720 }, scale: 2 },
          selection: { x, y, width: w, height: h },
          crop: { x: 0, y: 0, width: 1280, height: 720 },
          path,
          // Where the pen stopped: on the loop's edge.
          cursor: path[0],
          image: { url: `wisp://focus/${id}.png`, widthPx: 2560, heightPx: 1440 },
          application: { name: "Pages", pid: 1 },
          text: { lines: screenText, elapsedMs: 10 },
        },
      });
      await new Promise((r) => setTimeout(r, 1500));
      const done = new Promise<Record<string, unknown>>((resolve) =>
        listeners.push((e) => {
          if (
            e.event === "conversation.messageUpdated" &&
            e.payload?.role === "assistant" &&
            e.payload?.status !== "streaming"
          )
            resolve(e.payload!);
        }),
      );
      await call("conversation.send", {
        text: "What is this?",
        includeFocus: true,
        teach: false,
        act: false,
        allowCloud: false,
      });
      const m = await done;
      listeners.length = 0;
      const ok = t.expect.test(String(m.content));
      total++;
      if (ok) right++;
      const ctx = m.context as { imageBytes?: number };
      console.log(
        `${ok ? "✓" : "✗"} ${t.name.padEnd(14)} · picture ${ctx?.imageBytes ? "sent" : "not sent"} · “${String(m.content).replace(/\s+/g, " ").slice(0, 100)}”`,
      );
    }
  }
  console.log(`\n${right}/${total} named what was circled (${model})`);
  core.stdin!.end();
}
main().catch((e) => {
  console.error(e);
  process.exit(1);
});
