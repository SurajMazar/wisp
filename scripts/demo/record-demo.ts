/**
 * Records a product demo of Wisp's UI running against the real Rust engine.
 *
 *   pnpm demo            → build/demo/wisp-demo.mp4 (+ .gif)
 *
 * How it works: the built React app is served by Vite preview and opened in Chromium via
 * Playwright. A bridge stands in for the Swift host's WKWebView channel: it forwards IPC
 * envelopes to a real `wisp-core` process (Rust + SQLite) over stdio — exactly the protocol the
 * macOS host speaks. Nothing in the UI is mocked. `host.*` methods (implemented in Swift) are not
 * available here, so native-only features are described in captions instead of faked.
 *
 * ffmpeg then trims, fades and encodes the Playwright recording to MP4 and GIF.
 */
import { spawn, execFileSync, type ChildProcessWithoutNullStreams } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { chromium, type Page } from "playwright";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const outDir = join(root, "build/demo");
const rawDir = join(outDir, "raw");
const W = 1440;
const H = 900;
const PORT = 4179;

// ─── engine ────────────────────────────────────────────────────────────────

const dataDir = mkdtempSync(join(tmpdir(), "wisp-demo-"));
let core: ChildProcessWithoutNullStreams | null = null;
let deliver: (env: unknown) => void = () => {};

function startCore(): Promise<void> {
  const bin = join(root, "target/debug/wisp-core");
  core = spawn(bin, ["--data-dir", dataDir], { stdio: ["pipe", "pipe", "pipe"] });
  return new Promise((resolve) => {
    createInterface({ input: core!.stdout }).on("line", (line) => {
      const env = JSON.parse(line) as { kind: string; event?: string };
      if (env.kind === "event" && env.event === "core.ready") resolve();
      deliver(env);
    });
  });
}

function stopCore(): Promise<void> {
  return new Promise((resolve) => {
    if (!core) return resolve();
    core.once("exit", () => resolve());
    core.stdin.end();
    core = null;
  });
}

// ─── page helpers (presentation layer only — never touches app state) ──────

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Bridge injected into every frame: emulates the WKWebView message handler. */
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

/** Visible pointer (headless recordings have none). */
const cursorScript = `(() => {
  if (window.top !== window) return;
  addEventListener("DOMContentLoaded", () => {
    const c = document.createElement("div");
    c.id = "demo-cursor";
    c.style.cssText = "position:fixed;left:0;top:0;width:18px;height:18px;margin:-3px 0 0 -3px;z-index:2147483647;pointer-events:none;transition:transform 80ms ease-out";
    c.innerHTML = '<svg width="18" height="18" viewBox="0 0 18 18"><path d="M2 1.5 L2 15 L5.6 11.6 L8.1 17 L10.4 16 L7.9 10.8 L12.8 10.8 Z" fill="#111" stroke="#fff" stroke-width="1.2" stroke-linejoin="round"/></svg>';
    document.body.appendChild(c);
    addEventListener("mousemove", (e) => { c.style.left = e.clientX + "px"; c.style.top = e.clientY + "px"; }, true);
    addEventListener("mousedown", () => (c.style.transform = "scale(0.82)"), true);
    addEventListener("mouseup", () => (c.style.transform = ""), true);
  });
})();`;

async function overlay(page: Page, html: string, css: string, ms: number) {
  await page.evaluate(
    ({ html, css }) => {
      document.getElementById("demo-overlay")?.remove();
      const el = document.createElement("div");
      el.id = "demo-overlay";
      el.style.cssText =
        css + ";opacity:0;transition:opacity 450ms ease;pointer-events:none;z-index:2147483646";
      el.innerHTML = html;
      document.body.appendChild(el);
      requestAnimationFrame(() => (el.style.opacity = "1"));
    },
    { html, css },
  );
  if (ms > 0) {
    await sleep(ms);
    await clearOverlay(page);
  }
}

async function clearOverlay(page: Page) {
  await page.evaluate(() => {
    const el = document.getElementById("demo-overlay");
    if (!el) return;
    el.style.opacity = "0";
    setTimeout(() => el.remove(), 500);
  });
  await sleep(500);
}

const mark = `<svg width="72" height="72" viewBox="0 0 24 24"><defs><radialGradient id="g" cx="45%" cy="40%" r="60%"><stop offset="0%" stop-color="#fff"/><stop offset="45%" stop-color="#a99cf8"/><stop offset="100%" stop-color="#5ec8f2" stop-opacity="0"/></radialGradient></defs><circle cx="12" cy="12" r="11" fill="url(#g)" opacity=".55"/><circle cx="11" cy="11" r="4.2" fill="url(#g)"/></svg>`;

async function card(page: Page, title: string, subtitle: string, ms: number) {
  await page.evaluate(() => document.getElementById("demo-cursor")?.style.setProperty("opacity", "0"));
  await overlay(
    page,
    `<div style="display:flex;flex-direction:column;align-items:center;gap:18px;color:#ecebe6;font-family:-apple-system,BlinkMacSystemFont,sans-serif">
       ${mark}
       <div style="font-size:52px;font-weight:650;letter-spacing:-0.02em">${title}</div>
       <div style="font-size:21px;color:#a3a29b;max-width:880px;text-align:center;line-height:1.45">${subtitle}</div>
     </div>`,
    "position:fixed;inset:0;display:flex;align-items:center;justify-content:center;background:radial-gradient(ellipse at 50% 40%,#25213a 0%,#111110 70%)",
    ms,
  );
  await page.evaluate(() => document.getElementById("demo-cursor")?.style.setProperty("opacity", "1"));
}

async function caption(page: Page, text: string, ms = 0) {
  await overlay(
    page,
    `<div style="padding:12px 22px;border-radius:12px;background:rgba(108,92,231,.94);color:#fff;font:500 19px -apple-system,BlinkMacSystemFont,sans-serif;box-shadow:0 10px 40px rgba(0,0,0,.25);backdrop-filter:blur(12px);max-width:1000px;text-align:center">${text}</div>`,
    "position:fixed;left:0;right:0;bottom:44px;display:flex;justify-content:center",
    ms,
  );
}

/** Smoothly move the visible pointer to an element and click it. */
async function clickRole(
  page: Page,
  role: "radio" | "link" | "button" | "switch",
  name: string,
  pause = 650,
) {
  const target = page.getByRole(role, { name, exact: true }).first();
  const box = await target.boundingBox();
  if (!box) throw new Error(`no element ${role} "${name}"`);
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2, { steps: 22 });
  await sleep(180);
  await target.click();
  await sleep(pause);
}

async function hover(page: Page, selector: string, steps = 22) {
  const box = await page.locator(selector).first().boundingBox();
  if (box) await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2, { steps });
}

async function scrollTo(page: Page, heading: string) {
  await page.getByRole("heading", { name: heading, exact: true }).scrollIntoViewIfNeeded();
  await page.evaluate((h) => {
    const el = [...document.querySelectorAll("h3")].find((x) => x.textContent === h);
    el?.scrollIntoView({ behavior: "smooth", block: "start" });
  }, heading);
  await sleep(700);
}

// ─── run ───────────────────────────────────────────────────────────────────

async function main() {
  if (!existsSync(join(root, "target/debug/wisp-core")))
    execFileSync("cargo", ["build", "-p", "wisp-core"], { cwd: root, stdio: "inherit" });
  execFileSync("pnpm", ["--filter", "@wisp/web", "build"], { cwd: root, stdio: "inherit" });
  rmSync(outDir, { recursive: true, force: true });
  mkdirSync(rawDir, { recursive: true });

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
    {
      cwd: root,
      stdio: "ignore",
    },
  );
  for (let i = 0; i < 100; i++) {
    try {
      await fetch(`http://127.0.0.1:${PORT}/`);
      break;
    } catch {
      await sleep(100);
    }
  }

  await startCore();
  const browser = await chromium.launch();
  const context = await browser.newContext({
    viewport: { width: W, height: H },
    colorScheme: "light",
    recordVideo: { dir: rawDir, size: { width: W, height: H } },
  });
  await context.exposeBinding("__wispToCore", (_src, line: string) => core?.stdin.write(line + "\n"));
  await context.addInitScript(bridgeScript);
  await context.addInitScript(cursorScript);
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
  await page.getByText("Engine running").waitFor();
  await page.mouse.move(W / 2, H / 2);

  // 1 — title
  await card(
    page,
    "Wisp",
    "A local-first AI layer for macOS.<br>Point at anything. Ask. Wisp understands — privately, on your Mac.",
    3600,
  );

  // 2 — home
  await caption(page, "The interface is connected to Wisp's on-device engine (Rust + SQLite)");
  await hover(page, "nav >> text=Engine running", 30);
  await sleep(1600);
  await hover(page, "nav >> text=Local only", 14);
  await sleep(1400);
  await caption(page, "<b>Local Only</b> is the default — nothing leaves your Mac unless you allow it");
  await sleep(2400);

  // 3 — honest unbuilt sections
  await caption(page, "Features still in development say so — nothing is faked");
  await clickRole(page, "link", "Agents", 900);
  await clickRole(page, "link", "Memory", 900);
  await clickRole(page, "link", "Voice", 1200);

  // 4 — settings + theming
  await clickRole(page, "link", "Settings", 900);
  await caption(page, "Every change travels React → typed IPC → Rust → SQLite");
  await scrollTo(page, "Appearance");
  await clickRole(page, "radio", "Dark", 1100);
  await clickRole(page, "radio", "Cyan", 700);
  await clickRole(page, "radio", "Green", 700);
  await clickRole(page, "radio", "Orange", 700);
  await clickRole(page, "radio", "Wisp", 900);
  await caption(page, "A dark palette designed on its own — not an inverted light theme");
  await clickRole(page, "radio", "Light", 1300);
  await clickRole(page, "radio", "Dark", 1300);

  // 5 — privacy
  await scrollTo(page, "Privacy");
  await caption(page, "Choose where AI runs — Wisp explains exactly what each mode means");
  await clickRole(page, "radio", "Local Preferred", 2000);
  await clickRole(page, "radio", "Cloud Allowed", 2000);
  await clickRole(page, "radio", "Local Only", 1600);

  // 6 — invocation
  await scrollTo(page, "Invocation");
  await caption(page, "Summon Wisp from any app with ⌥⌘ — hold it, or tap to toggle");
  await hover(page, "kbd");
  await sleep(1500);
  await clickRole(page, "radio", "Toggle", 1500);
  await clickRole(page, "radio", "Hold", 1200);
  await caption(
    page,
    "Side mouse buttons work too. Where macOS has no API (trackpad gestures), Wisp says so",
  );
  await hover(page, "text=Trackpad gesture");
  await sleep(2600);

  // 7 — persistence
  await scrollTo(page, "Appearance");
  await clickRole(page, "radio", "Purple", 900);
  await caption(page, "Restarting the engine…");
  await sleep(900);
  await stopCore();
  await startCore();
  await page.reload();
  await page.getByText("Engine running").waitFor();
  await sleep(300);
  await scrollTo(page, "Appearance");
  await caption(page, "…dark theme and purple accent come back from the local database");
  await hover(page, '[aria-label="Purple"]');
  await sleep(2600);

  // 8 — assistant panel (content of the native floating panel)
  await clickRole(page, "link", "Home", 700);
  await page.evaluate((port) => {
    const wrap = document.createElement("div");
    wrap.id = "demo-panel";
    wrap.style.cssText =
      "position:fixed;left:50%;top:110px;width:440px;height:300px;margin-left:-220px;border-radius:14px;overflow:hidden;" +
      "background:rgba(32,32,36,.82);backdrop-filter:blur(24px) saturate(160%);box-shadow:0 24px 70px rgba(0,0,0,.35),0 0 0 1px rgba(255,255,255,.08);" +
      "opacity:0;transform:translateY(-8px) scale(.98);transition:all 380ms cubic-bezier(.22,1,.36,1);z-index:2147483000";
    wrap.innerHTML = `<iframe src="http://127.0.0.1:${port}/#/assistant" style="border:0;width:100%;height:100%;background:transparent" allowtransparency="true"></iframe>`;
    document.body.appendChild(wrap);
    setTimeout(() => {
      wrap.style.opacity = "1";
      wrap.style.transform = "none";
    }, 400);
  }, PORT);
  await caption(page, "Hold ⌥⌘ in any app: the assistant floats over your work without stealing focus");
  await sleep(3800);
  await caption(
    page,
    "Next: an assistant cursor that follows your pointer — draw around anything to ask about it",
  );
  await sleep(3400);
  await page.evaluate(() => document.getElementById("demo-panel")?.remove());

  // 9 — outro
  await card(
    page,
    "Wisp",
    "Phase 2 of 21 · foundation, engine and global invocation<br><span style='font-size:17px;color:#8a8a84'>github.com/SurajMazar/wisp</span>",
    4000,
  );

  await context.close();
  await browser.close();
  await stopCore();
  server.kill();
  rmSync(dataDir, { recursive: true, force: true });

  // ─── ffmpeg ──────────────────────────────────────────────────────────────
  const raw = readdirSync(rawDir).find((f) => f.endsWith(".webm"));
  if (!raw) throw new Error("no recording produced");
  const input = join(rawDir, raw);
  const duration = Number(
    execFileSync("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", input], {
      encoding: "utf8",
    }).trim(),
  );
  const trimStart = 0.6; // initial page load frames
  const length = duration - trimStart;
  const mp4 = join(outDir, "wisp-demo.mp4");
  const gif = join(outDir, "wisp-demo.gif");

  execFileSync(
    "ffmpeg",
    [
      "-y",
      "-v",
      "error",
      "-ss",
      String(trimStart),
      "-i",
      input,
      "-vf",
      `fps=30,fade=t=in:st=0:d=0.6,fade=t=out:st=${(length - 0.8).toFixed(2)}:d=0.8,format=yuv420p`,
      "-c:v",
      "libx264",
      "-preset",
      "slow",
      "-crf",
      "18",
      "-movflags",
      "+faststart",
      mp4,
    ],
    { stdio: "inherit" },
  );
  execFileSync(
    "ffmpeg",
    [
      "-y",
      "-v",
      "error",
      "-i",
      mp4,
      "-vf",
      "fps=12,scale=900:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=160[p];[b][p]paletteuse=dither=sierra2_4a",
      gif,
    ],
    { stdio: "inherit" },
  );
  console.log(`\n✓ ${mp4}\n✓ ${gif}  (${length.toFixed(1)} s)`);
}

main().catch(async (e) => {
  console.error(e);
  await stopCore();
  process.exit(1);
});
