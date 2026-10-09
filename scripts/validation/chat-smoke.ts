/**
 * End-to-end chat smoke test: the built React app in Chromium, wired to a real `wisp-core`
 * (same stdio protocol as the macOS host) answering with your local models (Ollama).
 *
 *   pnpm tsx scripts/validation/chat-smoke.ts [--shots <dir>]
 *
 * Sends from Home, expects to land in the conversation with the question and a streamed answer
 * visible on screen (not just in the DOM).
 */
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const PORT = 4181;
const shotsArg = process.argv.indexOf("--shots");
const shots = shotsArg > -1 ? process.argv[shotsArg + 1] : undefined;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const bridge = `(() => {
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

async function main() {
  execFileSync("cargo", ["build", "-q", "-p", "wisp-core"], { cwd: root, stdio: "inherit" });
  execFileSync("pnpm", ["--filter", "@wisp/web", "build"], { cwd: root, stdio: "ignore" });
  const server = spawn(
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
  const core = spawn(
    join(root, "target/debug/wisp-core"),
    ["--data-dir", mkdtempSync(join(tmpdir(), "wisp-smoke-"))],
    {
      stdio: ["pipe", "pipe", "ignore"],
    },
  );
  let deliver: (e: unknown) => void = () => {};
  const ready = new Promise<void>((resolve) =>
    createInterface({ input: core.stdout }).on("line", (line) => {
      const env = JSON.parse(line) as { kind: string; event?: string };
      if (env.kind === "event" && env.event === "core.ready") resolve();
      deliver(env);
    }),
  );
  for (let i = 0; i < 100; i++) {
    try {
      await fetch(`http://127.0.0.1:${PORT}/`);
      break;
    } catch {
      await sleep(100);
    }
  }
  await ready;
  const browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport: { width: 1200, height: 800 }, colorScheme: "dark" });
  await ctx.exposeBinding("__wispToCore", (_s, line: string) => core.stdin.write(line + "\n"));
  await ctx.addInitScript(bridge);
  const page = await ctx.newPage();
  deliver = (env) =>
    void page
      .evaluate(
        (e) => (window as unknown as { __wispDeliver?: (x: unknown) => void }).__wispDeliver?.(e),
        env,
      )
      .catch(() => {});
  try {
    await page.goto(`http://127.0.0.1:${PORT}/#/home`);
    await page.getByText("Engine running").waitFor();
    if (shots) mkdirSync(shots, { recursive: true });
    if (shots) await page.screenshot({ path: join(shots, "1-home.png") });
    await page.getByRole("textbox", { name: "Message" }).fill("What is 2 + 2?");
    await page.keyboard.press("Enter");
    await page.waitForURL(/#\/conversations\/[0-9a-f-]+$/, { timeout: 10_000 });
    await page.getByText("What is 2 + 2?").first().waitFor({ state: "visible" });
    // Wait for the answer to finish streaming (the Copy action appears).
    await page.getByRole("button", { name: "Copy" }).first().waitFor({ state: "attached", timeout: 120_000 });
    const box = await page.getByText("What is 2 + 2?").first().boundingBox();
    if (!box || box.height === 0 || box.y > 800)
      throw new Error(`question not visible on screen: ${JSON.stringify(box)}`);
    if (shots) await page.screenshot({ path: join(shots, "2-conversation.png") });
    console.log("✓ chat smoke passed");
  } finally {
    await browser.close();
    core.stdin.end();
    server.kill();
  }
}

main().catch((e) => {
  console.error("✗ chat smoke failed:", e);
  process.exit(1);
});
