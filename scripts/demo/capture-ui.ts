/**
 * Screenshots of Wisp's real UI for the product demo, running against a real `wisp-core` and
 * your local model (nothing in the UI is mocked; native-only `host.*` calls are unavailable).
 *
 *   pnpm tsx scripts/demo/capture-ui.ts <out dir> [--general qwen3-vl:4b-instruct]
 */
import { spawn, execFileSync, type ChildProcessByStdio } from "node:child_process";
import type { Readable, Writable } from "node:stream";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync, symlinkSync, unlinkSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { chromium, type Page } from "playwright";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const out = process.argv[2];
if (!out) throw new Error("usage: capture-ui.ts <out dir> [--general <model>]");
const arg = (k: string) => {
  const i = process.argv.indexOf(k);
  return i > -1 ? process.argv[i + 1] : undefined;
};
const general = arg("--general") ?? "qwen3-vl:4b-instruct";
const PORT = 4181;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const dataDir = mkdtempSync(join(tmpdir(), "wisp-demo-"));
// Your downloaded voice models, read in place (the link is removed before the temp folder is).
const userModels = join(homedir(), "Library/Application Support/Wisp/models");
if (existsSync(userModels)) symlinkSync(userModels, join(dataDir, "models"));
let core: ChildProcessByStdio<Writable, Readable, null>;
let deliver: (env: unknown) => void = () => {};
const waiting = new Map<string, (e: { result?: unknown; error?: { message: string } }) => void>();
let n = 0;

function call<T>(method: string, params?: unknown): Promise<T> {
  return new Promise((resolve, reject) => {
    const id = `demo:${++n}`;
    waiting.set(id, (e) => (e.error ? reject(new Error(e.error.message)) : resolve(e.result as T)));
    core.stdin.write(JSON.stringify({ v: 1, kind: "request", id, method, params: params ?? null }) + "\n");
  });
}

/** Emulates the WKWebView message handler, as in record-demo.ts. */
const bridgeScript = `(() => {
  const fid = Math.random().toString(36).slice(2, 8);
  window.webkit = { messageHandlers: { wisp: { postMessage(m) {
    if (m && m.kind === "request") m = { ...m, id: fid + ":" + m.id };
    window.__wispToCore(JSON.stringify(m));
  } } } };
  window.__wispDeliver = (env) => {
    if (env.kind === "response") {
      if (!env.id.startsWith(fid + ":")) return;
      env = { ...env, id: env.id.slice(fid.length + 1) };
    }
    window.__wispReceive && window.__wispReceive(env);
  };
})();`;

async function shot(page: Page, name: string) {
  await sleep(500);
  await page.screenshot({ path: join(out, `${name}.png`) });
  console.log(`  ${name}.png`);
}

async function section(page: Page, title: string) {
  await page.getByText(title, { exact: true }).first().scrollIntoViewIfNeeded();
  await page.evaluate((t) => {
    const el = [...document.querySelectorAll("h2,h3")].find((x) => x.textContent?.trim() === t);
    el?.scrollIntoView({ block: "start" });
    let p = el?.parentElement;
    while (p && p.scrollHeight <= p.clientHeight) p = p.parentElement;
    if (p) p.scrollTop -= 28;
  }, title);
  await sleep(400);
}

async function main() {
  mkdirSync(out, { recursive: true });
  execFileSync("cargo", ["build", "-q", "--release", "-p", "wisp-core"], { cwd: root, stdio: "inherit" });
  execFileSync("pnpm", ["--filter", "@wisp/web", "build"], { cwd: root, stdio: "ignore" });
  const preview = spawn(
    "pnpm",
    [
      "--filter",
      "@wisp/web",
      "exec",
      "vite",
      "preview",
      "--port",
      String(PORT),
      "--strictPort",
      "--host",
      "127.0.0.1",
    ],
    { cwd: root, stdio: "ignore" },
  );
  for (let i = 0; i < 100; i++) {
    try {
      await fetch(`http://127.0.0.1:${PORT}/`);
      break;
    } catch {
      await sleep(100);
    }
  }

  core = spawn(join(root, "target/release/wisp-core"), ["--data-dir", dataDir], {
    stdio: ["pipe", "pipe", "ignore"],
  });
  await new Promise<void>((resolve) =>
    createInterface({ input: core.stdout }).on("line", (line) => {
      const env = JSON.parse(line) as { kind: string; id?: string; event?: string };
      if (env.kind === "event" && env.event === "core.ready") resolve();
      if (env.kind === "response" && env.id && waiting.has(env.id)) {
        waiting.get(env.id)!(env as never);
        waiting.delete(env.id);
        return;
      }
      deliver(env);
    }),
  );

  // A finished first run, dark theme, your local model, and the test MCP server connected.
  const models = JSON.parse(
    execFileSync("sqlite3", [
      join(homedir(), "Library/Application Support/Wisp/database/wisp.sqlite3"),
      "select value from settings where key='models'",
    ]).toString(),
  );
  models.general = models.vision = { providerId: "ollama", model: general };
  const update = (section: string, value: unknown) => call("settings.update", { update: { section, value } });
  await update("models", models);
  await update("onboarding", { completed: true, completedAtMs: Date.now() });
  await update("appearance", { theme: "dark", accent: "default" });
  const server = join(dataDir, "notes-server.mjs");
  copyFileSync(join(root, "scripts/demo/notes-server.mjs"), server);
  await update("connections", {
    servers: [
      {
        id: "notes",
        name: "Team notes",
        enabled: true,
        localOnly: true,
        tools: [],
        transport: {
          kind: "stdio",
          config: {
            command: "node",
            // A copy outside the checkout, so the command line on screen has no home folder.
            args: [server],
            env: [],
          },
        },
      },
    ],
  });
  for (let i = 0; i < 50; i++) {
    const s = await call<{ servers: { state: string }[] }>("mcp.status");
    if (s.servers[0]?.state === "ready") break;
    await sleep(100);
  }

  const browser = await chromium.launch();
  const context = await browser.newContext({
    viewport: { width: 1280, height: 760 },
    deviceScaleFactor: 2,
    colorScheme: "dark",
  });
  await context.exposeBinding("__wispToCore", (_src, line: string) => core.stdin.write(line + "\n"));
  await context.addInitScript(bridgeScript);
  const page = await context.newPage();
  deliver = (env) => {
    for (const f of page.frames())
      void f
        .evaluate(
          (e) => (window as unknown as { __wispDeliver?: (x: unknown) => void }).__wispDeliver?.(e),
          env,
        )
        .catch(() => {});
  };

  await page.goto(`http://127.0.0.1:${PORT}/#/home`);
  await page
    .getByText("How can I help?")
    .first()
    .waitFor({ timeout: 15000 })
    .catch(() => {});
  await shot(page, "home");

  // A typed question, answered by the local model while we watch it stream.
  const box = page.locator("textarea").first();
  await box.click();
  await box.pressSequentially("What makes a good commit message? Keep it short.", { delay: 25 });
  await shot(page, "chat-typed");
  await box.press("Enter");
  await page.waitForURL(/conversations\//, { timeout: 15000 });
  await shot(page, "chat-thinking");
  const answer = page.locator("main");
  const before = (await answer.innerText()).length;
  const start = Date.now();
  let midShot = false;
  while (Date.now() - start < 90_000) {
    const text = (await answer.innerText()).length - before;
    if (!midShot && text > 110) {
      await page.screenshot({ path: join(out, "chat-streaming.png") });
      console.log("  chat-streaming.png");
      midShot = true;
    }
    if (midShot && (await page.getByRole("button", { name: /stop/i }).count()) === 0) break;
    await sleep(150);
  }
  await shot(page, "chat-done");

  await page.goto(`http://127.0.0.1:${PORT}/#/connections`);
  await page
    .getByText("Team notes")
    .first()
    .waitFor({ timeout: 10000 })
    .catch(() => {});
  await shot(page, "connections");

  await page.goto(`http://127.0.0.1:${PORT}/#/settings`);
  await section(page, "Dictation");
  await shot(page, "dictation");

  await page.goto(`http://127.0.0.1:${PORT}/#/models`);
  await page
    .getByText("Checking…")
    .first()
    .waitFor({ state: "detached", timeout: 30000 })
    .catch(() => {});
  await shot(page, "models");
  await section(page, "Cloud API keys");
  await shot(page, "models-cloud");

  await page.goto(`http://127.0.0.1:${PORT}/#/voice`);
  await sleep(1000);
  await section(page, "Voice engine");
  await shot(page, "voice");

  await page.goto(`http://127.0.0.1:${PORT}/#/settings`);
  await section(page, "Privacy");
  await shot(page, "privacy");

  await browser.close();
  core.stdin.end();
  preview.kill();
  if (existsSync(userModels)) unlinkSync(join(dataDir, "models"));
  rmSync(dataDir, { recursive: true, force: true });
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
