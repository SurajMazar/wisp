/**
 * Does a spoken "how do I…" question point at the right control when the control is an
 * unlabelled icon and the app exposes no accessibility labels? Uses a drawn video player whose
 * pause button (❚❚) is at a known spot, through a real `wisp-core` and your local model.
 *
 *   pnpm tsx scripts/validation/point-check.ts --image <player.jpg> [--model qwen3-vl:4b-instruct]
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
const PAUSE = { x: 50, y: 688 }; // centre of the ❚❚ button, in points

async function main() {
  execFileSync("cargo", ["build", "-q", "--release", "-p", "wisp-core"], { cwd: root, stdio: "inherit" });
  const data = mkdtempSync(join(tmpdir(), "wisp-point-"));
  mkdirSync(join(data, "cache/focus"), { recursive: true });
  const core = spawn(join(root, "target/release/wisp-core"), ["--data-dir", data], {
    stdio: ["pipe", "pipe", "ignore"],
  });
  const write = (e: unknown) => core.stdin!.write(JSON.stringify(e) + "\n");
  const waiting = new Map<string, (e: Env) => void>();
  const listeners: ((e: Env) => void)[] = [];
  const ready = new Promise<void>((resolve) =>
    createInterface({ input: core.stdout! }).on("line", (line) => {
      const e = JSON.parse(line) as Env;
      if (e.kind === "event" && e.event === "core.ready") resolve();
      if (e.kind === "response" && e.id && waiting.has(e.id)) {
        waiting.get(e.id)!(e);
        waiting.delete(e.id);
      }
      if (e.kind === "request" && e.id)
        write({ v: 1, kind: "response", id: e.id, error: { code: "unavailable", message: "no host" } });
      for (const l of listeners) l(e);
    }),
  );
  let n = 0;
  const call = <T>(method: string, params?: unknown) =>
    new Promise<T>((resolve, reject) => {
      const id = `pc:${++n}`;
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
  const model = arg("--model");
  if (model) models.general = models.vision = { providerId: "ollama", model };
  await call("settings.update", { update: { section: "models", value: models } });

  let hits = 0;
  const tries = Number(arg("--tries") ?? 3);
  for (let t = 0; t < tries; t++) {
    const id = randomUUID();
    execFileSync(
      "sips",
      ["-Z", "1280", arg("--image")!, "--out", join(data, "cache/focus", `${id}.model.jpg`)],
      { stdio: "ignore" },
    );
    const line = (text: string, y: number) => ({
      text,
      frame: { x: 20, y, width: 600, height: 20 },
      confidence: 0.95,
    });
    write({
      v: 1,
      kind: "event",
      event: "focus.completed",
      payload: {
        id,
        kind: "window",
        createdAtMs: Date.now(),
        display: { id: 1, name: "Built-in", frame: { x: 0, y: 0, width: 1280, height: 720 }, scale: 2 },
        selection: { x: 0, y: 0, width: 1280, height: 720 },
        crop: { x: 0, y: 0, width: 1280, height: 720 },
        cursor: { x: 640, y: 360 },
        image: { url: `wisp://focus/${id}.png`, widthPx: 2560, heightPx: 1440 },
        application: { name: "Brave Browser", pid: 1 },
        text: {
          lines: [
            line(`Nadine - Official Music Video (${t})`, 10),
            line("1.2M views · 2 days ago · Subscribe", 32),
            line("Comments 4,210 · Sort by · Add a comment", 54),
          ],
          elapsedMs: 10,
        },
      },
    });
    await new Promise((r) => setTimeout(r, 2000));
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
      text: "How do I pause this video?",
      includeFocus: true,
      teach: false,
      act: false,
      allowCloud: false,
    });
    const m = await done;
    listeners.length = 0;
    const g =
      (m.guidance as { rect: { x: number; y: number; width: number; height: number }; source: string }[]) ??
      [];
    const hit = g.some(
      (r) =>
        PAUSE.x >= r.rect.x - 20 &&
        PAUSE.x <= r.rect.x + r.rect.width + 20 &&
        PAUSE.y >= r.rect.y - 20 &&
        PAUSE.y <= r.rect.y + r.rect.height + 20,
    );
    if (hit) hits++;
    const ctx = m.context as { imageBytes?: number };
    console.log(
      `${hit ? "✓ points at pause" : "✗ misses pause  "} · picture ${ctx?.imageBytes ? "sent" : "not sent"} · marks ${JSON.stringify(g.map((x) => [Math.round(x.rect.x + x.rect.width / 2), Math.round(x.rect.y + x.rect.height / 2)]))} · “${String(m.content).slice(0, 110)}”`,
    );
  }
  console.log(`${hits}/${tries} pointed at the pause button`);
  core.stdin!.end();
}
main().catch((e) => {
  console.error(e);
  process.exit(1);
});
