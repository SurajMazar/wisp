/**
 * Measures the spoken-question loop against a real `wisp-core` and your local models:
 * a screen capture arrives when the hold starts, the user talks for a few seconds, then the
 * question is sent. Reports time from release to the answer's first word.
 *
 *   pnpm tsx scripts/validation/voice-latency.ts --region <cached region id> [--hold 3000]
 *
 * Uses a capture from Wisp's cache (copied into a throwaway data folder); each trial gets a
 * fresh copy so the model can't reuse work from the previous one.
 */
import { spawn, execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { randomUUID } from "node:crypto";
import { root, sleep } from "./harness";

const arg = (k: string) => {
  const i = process.argv.indexOf(k);
  return i > -1 ? process.argv[i + 1] : undefined;
};
const regionId = arg("--region");
const hold = Number(arg("--hold") ?? 3000);
/** Speak each question (macOS `say`) and send it as microphone audio: measures the full loop
 * through speech recognition to the first spoken word of the answer. */
const voice = process.argv.includes("--voice");
if (!regionId) throw new Error("--region <id> required");

const cache = join(homedir(), "Library/Application Support/Wisp/cache/focus");
const userSetting = (key: string) => {
  const out = execFileSync("sqlite3", [
    join(homedir(), "Library/Application Support/Wisp/database/wisp.sqlite3"),
    `select value from settings where key='${key}'`,
  ]).toString();
  return out.trim() ? JSON.parse(out) : null;
};
const userSettings = userSetting("models");

/** 16 kHz mono 16-bit PCM of `text` spoken by macOS. */
function speech(text: string, dir: string): Buffer {
  const aiff = join(dir, "q.aiff"),
    wav = join(dir, "q.wav");
  execFileSync("say", ["-o", aiff, text]);
  execFileSync("afconvert", ["-f", "WAVE", "-d", "LEI16@16000", "-c", "1", aiff, wav]);
  const b = readFileSync(wav);
  const at = b.indexOf("data");
  return b.subarray(at + 8, at + 8 + b.readUInt32LE(at + 4));
}

type Env = {
  kind: string;
  id?: string;
  event?: string;
  payload?: Record<string, unknown>;
  result?: unknown;
  error?: { message: string };
};

async function main() {
  // The app ships the optimized build; measure that.
  execFileSync("cargo", ["build", "-q", "--release", "-p", "wisp-core"], { cwd: root, stdio: "inherit" });
  const data = mkdtempSync(join(tmpdir(), "wisp-latency-"));
  mkdirSync(join(data, "cache/focus"), { recursive: true });
  const core = spawn(join(root, "target/release/wisp-core"), ["--data-dir", data], {
    stdio: ["pipe", "pipe", "ignore"],
  });
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
        // No host here: answer host requests with "unavailable" so nothing waits on them.
        core.stdin!.write(
          JSON.stringify({
            v: 1,
            kind: "response",
            id: e.id,
            error: { code: "unavailable", message: "no host" },
          }) + "\n",
        );
      for (const l of listeners) l(e);
    }),
  );
  let n = 0;
  const call = <T>(method: string, params?: unknown) =>
    new Promise<T>((resolve, reject) => {
      const id = `lat:${++n}`;
      waiting.set(id, (e) => (e.error ? reject(new Error(e.error.message)) : resolve(e.result as T)));
      core.stdin!.write(JSON.stringify({ v: 1, kind: "request", id, method, params: params ?? null }) + "\n");
    });
  const emit = (event: string, payload: unknown) =>
    core.stdin!.write(JSON.stringify({ v: 1, kind: "event", event, payload }) + "\n");
  await ready;
  const vision = arg("--vision");
  if (userSettings && vision) userSettings.vision = { providerId: "ollama", model: vision };
  const general = arg("--general");
  if (userSettings && general) userSettings.general = { providerId: "ollama", model: general };
  if (userSettings) await call("settings.update", { update: { section: "models", value: userSettings } });
  if (voice) {
    // Use the speech models already downloaded (linked, not copied).
    const models = join(homedir(), "Library/Application Support/Wisp/models");
    mkdirSync(join(data, "models"), { recursive: true });
    for (const m of readdirSync(models))
      execFileSync("ln", ["-sfn", join(models, m), join(data, "models", m)]);
    const v = userSetting("voice") ?? {};
    const tts = arg("--tts");
    if (tts) v.ttsModel = tts;
    if (Object.keys(v).length) await call("settings.update", { update: { section: "voice", value: v } });
  }
  await call("settings.update", { update: { section: "onboarding", value: { completed: true } } });

  const base = JSON.parse(readFileSync(join(cache, `${regionId}.region.json`), "utf8"));
  const questions = [
    "What is this page about? One sentence.",
    "Where do I click to see more? Answer briefly.",
    "Summarize what I'm looking at in one line.",
    "What colour is the main button?",
  ];
  const results: { q: string; firstMs: number; totalMs: number; image: boolean }[] = [];
  for (const [i, q] of questions.entries()) {
    // A fresh capture for each trial (new id, slightly different text) so nothing is reused.
    const id = randomUUID();
    // A slightly different image each time too (the model caches by content).
    execFileSync(
      "sips",
      [
        "-Z",
        String(1024 - i * 9),
        join(cache, `${regionId}.model.jpg`),
        "--out",
        join(data, "cache/focus", `${id}.model.jpg`),
      ],
      { stdio: "ignore" },
    );
    const region = structuredClone(base);
    region.id = id;
    region.createdAtMs = Date.now();
    if (region.text?.lines?.length)
      region.text.lines[0].text = `[trial ${i} ${id.slice(0, 6)}] ` + region.text.lines[0].text;
    if (region.image) region.image.url = `wisp://focus/${id}.png`;
    // As in the app: the hold starts (speech models warm up), then the capture arrives.
    emit("invocation.started", { atMs: Date.now(), behavior: "hold", source: "keyboard" });
    emit("focus.completed", region);
    const session = randomUUID();
    if (voice) {
      // The user talks while holding: audio streams in during the hold.
      const pcm = speech(q, data);
      for (let off = 0, seq = 0; off < pcm.length; off += 3200, seq++)
        emit("audio.chunk", {
          sessionId: session,
          seq,
          pcm: pcm.subarray(off, off + 3200).toString("base64"),
        });
    }
    await sleep(hold);
    const t0 = Date.now();
    let first = 0;
    let spoken = 0;
    const done = new Promise<{ image: boolean }>((resolve) =>
      listeners.push((e) => {
        if (e.event === "conversation.delta" && !first) first = Date.now() - t0;
        if (e.event === "voice.speechChunk" && !spoken) spoken = Date.now() - t0;
        if (
          e.event === "conversation.messageUpdated" &&
          e.payload?.role === "assistant" &&
          e.payload?.status !== "streaming"
        ) {
          const info = e.payload?.context as { imageBytes?: number } | undefined;
          resolve({ image: !!info?.imageBytes });
        }
      }),
    );
    if (voice) await call("voice.finish", { sessionId: session, includeFocus: true });
    else
      await call("conversation.send", {
        text: q,
        includeFocus: true,
        teach: false,
        act: false,
        allowCloud: false,
      });
    const { image } = await done;
    if (voice) for (let i = 0; i < 100 && !spoken; i++) await sleep(100);
    listeners.length = 0;
    results.push({ q, firstMs: voice ? spoken : first, totalMs: Date.now() - t0, image });
    console.log(
      `${String(first).padStart(6)} ms to first word` +
        (voice ? ` · ${String(spoken).padStart(6)} ms to first spoken word` : "") +
        ` · ${image ? "with image" : "text only "} · ${q}`,
    );
  }
  const firsts = results.map((r) => r.firstMs).sort((a, b) => a - b);
  console.log(
    `median first ${voice ? "spoken " : ""}word: ${firsts[Math.floor(firsts.length / 2)]} ms (hold ${hold} ms)`,
  );
  core.stdin!.end();
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
