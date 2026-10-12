/**
 * Shared end-to-end harness: the built React app in Chromium, wired to a real `wisp-core` over the
 * same stdio protocol the macOS host uses. Host-only methods (Keychain, screen, input) aren't
 * available here, exactly as in a browser.
 */
import { spawn, execFileSync, type ChildProcessWithoutNullStreams } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { chromium, type Browser, type Page } from "playwright";

export const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
export const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

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

type Envelope = {
  kind: string;
  id?: string;
  event?: string;
  method?: string;
  result?: unknown;
  error?: { message: string };
  payload?: unknown;
};

export type App = {
  page: Page;
  /** Call the engine directly (to check what the UI saved). */
  call: <T = unknown>(method: string, params?: unknown) => Promise<T>;
  onEnvelope: (f: (e: Envelope) => void) => void;
  close: () => Promise<void>;
};

export async function startApp({
  port,
  viewport = { width: 1200, height: 800 },
}: {
  port: number;
  viewport?: { width: number; height: number };
}): Promise<App> {
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
      String(port),
      "--strictPort",
      "--host",
      "127.0.0.1",
    ],
    { cwd: root, stdio: "ignore" },
  );
  const core: ChildProcessWithoutNullStreams = spawn(
    join(root, "target/debug/wisp-core"),
    ["--data-dir", mkdtempSync(join(tmpdir(), "wisp-e2e-"))],
    { stdio: ["pipe", "pipe", "ignore"] } as never,
  ) as ChildProcessWithoutNullStreams;
  const listeners: ((e: Envelope) => void)[] = [];
  const waiting = new Map<string, (e: Envelope) => void>();
  let deliver: (e: Envelope) => void = () => {};
  const ready = new Promise<void>((resolve) =>
    createInterface({ input: core.stdout }).on("line", (line) => {
      const env = JSON.parse(line) as Envelope;
      if (env.kind === "event" && env.event === "core.ready") resolve();
      if (env.kind === "response" && env.id && waiting.has(env.id)) {
        waiting.get(env.id)?.(env);
        waiting.delete(env.id);
        return;
      }
      for (const l of listeners) l(env);
      deliver(env);
    }),
  );
  for (let i = 0; i < 100; i++) {
    try {
      await fetch(`http://127.0.0.1:${port}/`);
      break;
    } catch {
      await sleep(100);
    }
  }
  await ready;
  const browser: Browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport, colorScheme: "dark" });
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
  page.on("console", (m) => {
    if (m.type() === "error" || m.type() === "warning") console.log("console:", m.text());
  });
  let n = 0;
  return {
    page,
    onEnvelope: (f) => listeners.push(f),
    call: <T>(method: string, params?: unknown) =>
      new Promise<T>((resolve, reject) => {
        const id = `test:${++n}`;
        waiting.set(id, (e) => (e.error ? reject(new Error(e.error.message)) : resolve(e.result as T)));
        core.stdin.write(
          JSON.stringify({ v: 1, kind: "request", id, method, params: params ?? null }) + "\n",
        );
      }),
    close: async () => {
      await browser.close();
      core.stdin.end();
      server.kill();
    },
  };
}

/** Walk through first-run setup the way a person would, skipping each step. */
export async function completeOnboarding(page: Page, port: number) {
  await page.goto(`http://127.0.0.1:${port}/#/home`);
  const welcome = page.getByRole("button", { name: "Set Up My Assistant →" });
  await welcome.waitFor({ timeout: 15_000 });
  await welcome.click();
  for (let i = 0; i < 7; i++) {
    const b = page.getByRole("button", { name: /Skip for now|Got it|Later|Continue/ }).first();
    if (!(await b.isVisible().catch(() => false))) break;
    await b.click();
  }
  await page.getByRole("button", { name: "Start using Wisp" }).click();
  await page.getByText("Engine running").waitFor({ timeout: 10_000 });
}
